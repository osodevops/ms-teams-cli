use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::token::TokenInfo;
use crate::error::{Result, TeamsError};

const DISABLE_KEYRING_ENV: &str = "TEAMS_CLI_DISABLE_KEYRING";
const TOKEN_STORE_ENV: &str = "TEAMS_CLI_TOKEN_STORE";

/// Largest secret a single OS credential entry will hold, when the platform
/// imposes a limit small enough to matter.
///
/// Windows Credential Manager caps a credential blob at
/// `CRED_MAX_CREDENTIAL_BLOB_SIZE` (2560 bytes). A Microsoft Graph token
/// bundle is routinely twice that, so on Windows the serialized token is split
/// across several entries. macOS Keychain and Linux Secret Service accept
/// secrets far larger than any token bundle, so they keep a single entry; on
/// macOS that also keeps the number of items the user must grant access to at
/// one.
#[cfg(windows)]
const CHUNK_BYTES: Option<usize> = Some(2560);
#[cfg(not(windows))]
const CHUNK_BYTES: Option<usize> = None;

fn entry_key(profile: &str) -> String {
    format!("{profile}:token")
}

fn chunk_key(profile: &str, index: usize) -> String {
    format!("{profile}:token:{index}")
}

/// Contents of the primary entry when the token is stored in chunks. The
/// chunks themselves live in `<profile>:token:<index>` entries.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChunkHeader {
    chunks: usize,
}

fn disabled() -> bool {
    std::env::var(DISABLE_KEYRING_ENV).is_ok_and(|value| {
        matches!(
            value.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

/// Where tokens live. The OS keyring by default; files under the config
/// directory when `TEAMS_CLI_TOKEN_STORE=file`, for a process that cannot
/// answer a keychain dialog or has no keyring at all — a daemon, a server, a
/// container. On macOS the keychain grants access to a code signature, so an
/// unattended process running a freshly built binary is asked once per
/// profile per build, and a build signed with a local identity is still asked
/// because the item's partition list names build hashes (measured
/// 2026-09-09); Linux without a Secret Service cannot store a token at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoreKind {
    Keyring,
    File,
}

fn store_kind() -> Result<StoreKind> {
    parse_store_kind(std::env::var(TOKEN_STORE_ENV).ok().as_deref())
}

fn parse_store_kind(value: Option<&str>) -> Result<StoreKind> {
    let Some(value) = value else {
        return Ok(StoreKind::Keyring);
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "keyring" | "keychain" => Ok(StoreKind::Keyring),
        "file" => Ok(StoreKind::File),
        _ => Err(TeamsError::InvalidInput(format!(
            "{TOKEN_STORE_ENV}={value} is not a token store; use `keyring` (default) or `file`"
        ))),
    }
}

/// Refuse a mistyped `TEAMS_CLI_TOKEN_STORE` before any command runs. Token
/// reads report every store error as "not signed in", so without this a typo
/// would send the caller to log in again instead of naming the variable.
pub fn check_token_store() -> Result<()> {
    store_kind().map(|_| ())
}

fn store() -> Result<Box<dyn SecretStore>> {
    Ok(match store_kind()? {
        StoreKind::Keyring => Box::new(OsKeyring),
        StoreKind::File => Box::new(FileStore::new(crate::config::config_dir()?.join("tokens"))),
    })
}

/// A file store has no size limit, so it never chunks; the keyring chunks
/// where the platform makes it necessary.
fn chunk_bytes_for(kind: StoreKind) -> Option<usize> {
    match kind {
        StoreKind::Keyring => CHUNK_BYTES,
        StoreKind::File => None,
    }
}

/// The minimal credential-store surface the token logic needs. Implemented by
/// the OS keyring in production and by an in-memory map in tests, because the
/// `keyring` crate's mock store does not persist across entries.
trait SecretStore {
    /// Returns `Ok(None)` when no entry exists under `key`.
    fn get_password(&self, key: &str) -> Result<Option<String>>;
    fn set_password(&self, key: &str, value: &str) -> Result<()>;
    /// Returns `Ok(None)` when no entry exists under `key`.
    fn get_secret(&self, key: &str) -> Result<Option<Vec<u8>>>;
    fn set_secret(&self, key: &str, value: &[u8]) -> Result<()>;
    /// Returns whether an entry existed under `key` before the call.
    fn delete(&self, key: &str) -> Result<bool>;
}

struct OsKeyring;

impl OsKeyring {
    fn entry(key: &str) -> Result<::keyring::Entry> {
        ::keyring::Entry::new(crate::config::NAMESPACE, key)
            .map_err(|e| TeamsError::KeyringError(format!("Failed to create keyring entry: {e}")))
    }
}

fn absent_as_none<T>(result: ::keyring::Result<T>, what: &str) -> Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(::keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(TeamsError::KeyringError(format!("Failed to {what}: {e}"))),
    }
}

impl SecretStore for OsKeyring {
    fn get_password(&self, key: &str) -> Result<Option<String>> {
        absent_as_none(Self::entry(key)?.get_password(), "retrieve token")
    }

    fn set_password(&self, key: &str, value: &str) -> Result<()> {
        // Update in place rather than delete-and-recreate: recreating the item
        // discards its access control list on macOS, so every silent token
        // refresh would revoke a previously granted "Always Allow".
        Self::entry(key)?
            .set_password(value)
            .map_err(|e| TeamsError::KeyringError(format!("Failed to store token: {e}")))
    }

    fn get_secret(&self, key: &str) -> Result<Option<Vec<u8>>> {
        absent_as_none(Self::entry(key)?.get_secret(), "retrieve token")
    }

    fn set_secret(&self, key: &str, value: &[u8]) -> Result<()> {
        Self::entry(key)?
            .set_secret(value)
            .map_err(|e| TeamsError::KeyringError(format!("Failed to store token: {e}")))
    }

    fn delete(&self, key: &str) -> Result<bool> {
        Ok(absent_as_none(Self::entry(key)?.delete_credential(), "delete token")?.is_some())
    }
}

/// One file per entry under `dir`, `<key>` with `:` as `.` (`default:token` is
/// `default.token`), the directory `0700` and each file `0600` on Unix. Writes
/// go to a sibling temporary file and rename over, so a reader never sees a
/// partial token.
struct FileStore {
    dir: PathBuf,
}

impl FileStore {
    fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn path(&self, key: &str) -> PathBuf {
        self.dir.join(key.replace([':', '/', '\\'], "."))
    }

    fn ensure_dir(&self) -> Result<()> {
        std::fs::create_dir_all(&self.dir).map_err(|e| {
            TeamsError::KeyringError(format!(
                "Failed to create token directory {}: {e}",
                self.dir.display()
            ))
        })?;
        restrict(&self.dir, 0o700)
    }
}

#[cfg(unix)]
fn restrict(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).map_err(|e| {
        TeamsError::KeyringError(format!(
            "Failed to set permissions on {}: {e}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn restrict(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

/// Create `path` and write `value`, never following a symlink and never
/// widening past `0600`: `create_new` fails rather than opening an existing
/// file or a planted symlink, and the mode is set at creation so the bytes are
/// never briefly readable at the umask default.
fn write_private(path: &Path, value: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| TeamsError::KeyringError(format!("Failed to store token: {e}")))?;
    // Flushed to disk before the caller renames it into place, so a crash
    // cannot leave an empty or truncated token where the old one was.
    file.write_all(value)
        .and_then(|()| file.sync_all())
        .map_err(|e| TeamsError::KeyringError(format!("Failed to store token: {e}")))
}

/// Make the rename itself durable. Best-effort: the token is already in
/// place, and Windows cannot open a directory to sync it.
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(dir) = std::fs::File::open(dir) {
        let _ = dir.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// The store writes every file `0600`, so a token readable by the group or by
/// others was copied or loosened by hand. Say so on stderr rather than refuse:
/// a refused read would surface as "not signed in", which hides the cause.
#[cfg(unix)]
fn warn_if_exposed(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(path) {
        let mode = meta.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            tracing::warn!(
                "{} is readable by other users (mode {mode:o}); run `chmod 600` on it",
                path.display()
            );
        }
    }
}

#[cfg(not(unix))]
fn warn_if_exposed(_path: &Path) {}

fn absent_file_as_none<T>(result: std::io::Result<T>, what: &str) -> Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(TeamsError::KeyringError(format!("Failed to {what}: {e}"))),
    }
}

impl SecretStore for FileStore {
    fn get_password(&self, key: &str) -> Result<Option<String>> {
        let path = self.path(key);
        warn_if_exposed(&path);
        absent_file_as_none(std::fs::read_to_string(path), "retrieve token")
    }

    fn set_password(&self, key: &str, value: &str) -> Result<()> {
        self.set_secret(key, value.as_bytes())
    }

    fn get_secret(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let path = self.path(key);
        warn_if_exposed(&path);
        absent_file_as_none(std::fs::read(path), "retrieve token")
    }

    fn set_secret(&self, key: &str, value: &[u8]) -> Result<()> {
        self.ensure_dir()?;
        let path = self.path(key);
        // A temp name unique to this write (pid + nanos) so concurrent writers
        // never share one, written 0600 and renamed over so a reader never sees
        // a partial or wider-than-0600 token.
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("token");
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp = self
            .dir
            .join(format!(".{name}.{}.{nonce}.tmp", std::process::id()));
        write_private(&tmp, value)?;
        if let Err(e) = std::fs::rename(&tmp, &path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(TeamsError::KeyringError(format!(
                "Failed to store token: {e}"
            )));
        }
        sync_dir(&self.dir);
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<bool> {
        Ok(absent_file_as_none(std::fs::remove_file(self.path(key)), "delete token")?.is_some())
    }
}

pub fn store_token(profile: &str, token: &TokenInfo) -> Result<()> {
    if disabled() {
        return Err(TeamsError::KeyringError("Keyring is disabled".into()));
    }
    store_token_in(&*store()?, profile, token, chunk_bytes_for(store_kind()?))
}

pub fn get_token(profile: &str) -> Result<TokenInfo> {
    if disabled() {
        return Err(TeamsError::KeyringError("Keyring is disabled".into()));
    }
    get_token_from(&*store()?, profile)
}

pub fn delete_token(profile: &str) -> Result<()> {
    if disabled() {
        return Ok(());
    }
    delete_token_from(
        &*store()?,
        profile,
        chunk_bytes_for(store_kind()?).is_some(),
    )
}

fn store_token_in(
    store: &dyn SecretStore,
    profile: &str,
    token: &TokenInfo,
    chunk_bytes: Option<usize>,
) -> Result<()> {
    let json = serde_json::to_string(token)
        .map_err(|e| TeamsError::KeyringError(format!("Failed to serialize token: {e}")))?;
    match chunk_bytes {
        None => store.set_password(&entry_key(profile), &json),
        Some(size) => store_chunked(store, profile, json.as_bytes(), size),
    }
}

/// Writes the new chunks, publishes the header, then removes chunks left over
/// from a previous, larger token.
///
/// Credential Manager offers no transactions, so a write interrupted part-way
/// (or two processes refreshing at once) can leave the header describing a
/// mixture of old and new chunks. That state either fails to parse or yields
/// a spliced token that Microsoft Graph rejects; in both cases the next
/// refresh or login rewrites every entry consistently. The scheme is
/// self-healing rather than atomic.
fn store_chunked(store: &dyn SecretStore, profile: &str, bytes: &[u8], size: usize) -> Result<()> {
    let previous = stored_chunk_count(store, profile)?;
    let chunks: Vec<&[u8]> = bytes.chunks(size).collect();
    for (index, chunk) in chunks.iter().enumerate() {
        store.set_secret(&chunk_key(profile, index), chunk)?;
    }
    let header = serde_json::to_string(&ChunkHeader {
        chunks: chunks.len(),
    })
    .map_err(|e| TeamsError::KeyringError(format!("Failed to serialize token: {e}")))?;
    store.set_password(&entry_key(profile), &header)?;
    delete_chunks(store, profile, chunks.len(), previous)
}

/// The chunk count recorded in the primary entry, or zero when there is no
/// entry or it holds a legacy single-entry token.
fn stored_chunk_count(store: &dyn SecretStore, profile: &str) -> Result<usize> {
    Ok(store
        .get_password(&entry_key(profile))?
        .and_then(|primary| serde_json::from_str::<ChunkHeader>(&primary).ok())
        .map_or(0, |header| header.chunks))
}

fn get_token_from(store: &dyn SecretStore, profile: &str) -> Result<TokenInfo> {
    let primary = store.get_password(&entry_key(profile))?.ok_or_else(|| {
        TeamsError::KeyringError(format!(
            "Failed to retrieve token: no stored token for profile `{profile}`"
        ))
    })?;
    let json = match serde_json::from_str::<ChunkHeader>(&primary) {
        Ok(header) => read_chunks(store, profile, header.chunks)?,
        Err(_) => primary,
    };
    serde_json::from_str(&json)
        .map_err(|e| TeamsError::KeyringError(format!("Failed to parse stored token: {e}")))
}

fn read_chunks(store: &dyn SecretStore, profile: &str, count: usize) -> Result<String> {
    let mut bytes = Vec::new();
    for index in 0..count {
        let chunk = store
            .get_secret(&chunk_key(profile, index))?
            .ok_or_else(|| {
                TeamsError::KeyringError(format!(
                    "Stored token is incomplete (chunk {index} of {count} is missing). \
                 Run `teams auth login` again."
                ))
            })?;
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes)
        .map_err(|e| TeamsError::KeyringError(format!("Stored token is not valid UTF-8: {e}")))
}

fn delete_token_from(store: &dyn SecretStore, profile: &str, chunked: bool) -> Result<()> {
    if chunked {
        // Remove the chunks before the header, so that a sweep cut short by an
        // error can still learn how many chunks to expect when it is retried.
        let recorded = stored_chunk_count(store, profile)?;
        delete_chunks(store, profile, 0, recorded)?;
    }
    store.delete(&entry_key(profile))?;
    Ok(())
}

/// Deletes chunk entries from `start` upwards. Every index below `recorded`
/// (the count a header claimed) is attempted whether or not it exists, so a
/// gap left by an earlier partial failure cannot hide later chunks; beyond
/// that the sweep continues until the first index that does not exist, which
/// also catches chunks no header ever described.
fn delete_chunks(
    store: &dyn SecretStore,
    profile: &str,
    start: usize,
    recorded: usize,
) -> Result<()> {
    let mut index = start;
    loop {
        let existed = store.delete(&chunk_key(profile, index))?;
        if !existed && index >= recorded {
            return Ok(());
        }
        index += 1;
    }
}

pub fn list_profiles() -> Vec<String> {
    if disabled() {
        return vec![];
    }

    // Keyring doesn't support enumeration natively.
    // We maintain a separate index entry.
    match store().and_then(|s| s.get_password("profile-index")) {
        Ok(Some(json)) => serde_json::from_str(&json).unwrap_or_default(),
        _ => vec![],
    }
}

pub fn add_profile_to_index(profile: &str) -> Result<()> {
    if disabled() {
        return Ok(());
    }

    let mut profiles = list_profiles();
    if !profiles.contains(&profile.to_string()) {
        profiles.push(profile.to_string());
    }
    write_profile_index(&profiles)
}

pub fn remove_profile_from_index(profile: &str) -> Result<()> {
    if disabled() {
        return Ok(());
    }

    let mut profiles = list_profiles();
    profiles.retain(|p| p != profile);
    write_profile_index(&profiles)
}

fn write_profile_index(profiles: &[String]) -> Result<()> {
    let json = serde_json::to_string(profiles)
        .map_err(|e| TeamsError::KeyringError(format!("Failed to serialize index: {e}")))?;
    store()?
        .set_password("profile-index", &json)
        .map_err(|e| TeamsError::KeyringError(format!("Failed to store index: {e}")))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use super::*;

    /// Keychain entries are named after the build's storage namespace, so by
    /// default a debug build does not reach a release build's items.
    /// Building an entry does not touch the keychain.
    #[cfg(target_os = "macos")]
    #[test]
    fn keychain_entries_use_the_storage_namespace_as_their_service() {
        let entry = OsKeyring::entry("default:token").unwrap();
        let credential = entry
            .get_credential()
            .downcast_ref::<::keyring::macos::MacCredential>()
            .expect("the macOS keyring builds MacCredential entries");
        assert_eq!(credential.service, crate::config::NAMESPACE);
        assert_eq!(credential.account, "default:token");
    }

    #[derive(Default)]
    struct MemoryStore {
        entries: Mutex<BTreeMap<String, Vec<u8>>>,
    }

    impl MemoryStore {
        fn keys(&self) -> Vec<String> {
            self.entries.lock().unwrap().keys().cloned().collect()
        }

        fn remove(&self, key: &str) {
            self.entries.lock().unwrap().remove(key);
        }
    }

    impl SecretStore for MemoryStore {
        fn get_password(&self, key: &str) -> Result<Option<String>> {
            Ok(self
                .get_secret(key)?
                .map(|bytes| String::from_utf8(bytes).unwrap()))
        }

        fn set_password(&self, key: &str, value: &str) -> Result<()> {
            self.set_secret(key, value.as_bytes())
        }

        fn get_secret(&self, key: &str) -> Result<Option<Vec<u8>>> {
            Ok(self.entries.lock().unwrap().get(key).cloned())
        }

        fn set_secret(&self, key: &str, value: &[u8]) -> Result<()> {
            self.entries
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_vec());
            Ok(())
        }

        fn delete(&self, key: &str) -> Result<bool> {
            Ok(self.entries.lock().unwrap().remove(key).is_some())
        }
    }

    /// The chunking arithmetic is exercised against an in-memory store on every platform, which
    /// proves the splitting and the sweep but not that Credential Manager accepts what it is
    /// handed. Only a real store shows that, and Windows is the only platform whose limit makes
    /// it matter, so this runs there alone — CI has a Credential Manager, and a developer on
    /// macOS keeps their keychain untouched.
    #[cfg(windows)]
    #[test]
    fn a_token_too_large_for_one_credential_round_trips_through_credential_manager() {
        let profile = format!("teams-cli-test-{}", uuid::Uuid::new_v4());
        let original = token(&"a".repeat(6000));
        let json = serde_json::to_string(&original).unwrap();
        assert!(
            json.len() > 2560,
            "token must exceed one blob to be the case at hand"
        );

        // what issue #67 hit: the whole bundle in a single entry
        let single = OsKeyring.set_password(&entry_key(&profile), &json);

        let stored = store_token_in(&OsKeyring, &profile, &original, CHUNK_BYTES);
        let loaded = stored.and_then(|()| get_token_from(&OsKeyring, &profile));
        let swept = delete_token_from(&OsKeyring, &profile, CHUNK_BYTES.is_some());
        let after = get_token_from(&OsKeyring, &profile);

        assert!(
            single.is_err(),
            "Credential Manager accepted an oversized blob; the premise of chunking is gone"
        );
        let loaded = loaded.expect("chunked store and load");
        assert_eq!(loaded.access_token, original.access_token);
        assert_eq!(loaded.refresh_token, original.refresh_token);
        assert_eq!(loaded.scope, original.scope);
        swept.expect("logout sweep");
        assert!(after.is_err(), "logout left the token readable");
    }

    fn token(access_token: &str) -> TokenInfo {
        TokenInfo {
            access_token: access_token.to_string(),
            expires_at: None,
            token_type: "Bearer".to_string(),
            scope: Some("User.Read".to_string()),
            refresh_token: Some("refresh-0.ARwA".to_string()),
            profile: "work".to_string(),
        }
    }

    fn serialized_len(token: &TokenInfo) -> usize {
        serde_json::to_string(token).unwrap().len()
    }

    #[test]
    fn file_store_round_trips_under_the_directory_with_private_permissions() {
        let dir = std::env::temp_dir().join(format!("teams-cli-tokens-{}", uuid::Uuid::new_v4()));
        let store = FileStore::new(dir.clone());
        let original = token(&"a".repeat(5000));

        store_token_in(&store, "work", &original, chunk_bytes_for(StoreKind::File)).unwrap();

        let path = dir.join("work.token");
        assert!(path.is_file(), "{}", path.display());
        let leftover_tmp = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().ends_with(".tmp"));
        assert!(!leftover_tmp, "temporary file left behind");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let loaded = get_token_from(&store, "work").unwrap();
        assert_eq!(loaded.access_token, original.access_token);
        assert_eq!(loaded.refresh_token, original.refresh_token);

        store.set_password("profile-index", "[\"work\"]").unwrap();
        assert_eq!(
            store.get_password("profile-index").unwrap().as_deref(),
            Some("[\"work\"]")
        );

        delete_token_from(&store, "work", false).unwrap();
        assert!(get_token_from(&store, "work").is_err());
        assert!(
            !store.delete("work:token").unwrap(),
            "a second delete finds nothing"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn store_kind_is_keyring_unless_file_is_asked_for() {
        for value in [
            None,
            Some(""),
            Some("keyring"),
            Some("Keychain"),
            Some(" KEYRING "),
        ] {
            assert_eq!(
                parse_store_kind(value).unwrap(),
                StoreKind::Keyring,
                "{value:?}"
            );
        }
        for value in ["file", "FILE", " file\n"] {
            assert_eq!(
                parse_store_kind(Some(value)).unwrap(),
                StoreKind::File,
                "{value:?}"
            );
        }
        let err = parse_store_kind(Some("flie")).unwrap_err();
        assert!(matches!(err, TeamsError::InvalidInput(_)), "{err:?}");
        assert_eq!(err.exit_code(), 2);
        assert!(
            err.to_string().contains("TEAMS_CLI_TOKEN_STORE=flie"),
            "{err}"
        );
    }

    #[test]
    fn single_entry_round_trip_when_chunking_is_off() {
        let store = MemoryStore::default();
        let original = token(&"a".repeat(5000));

        store_token_in(&store, "work", &original, None).unwrap();

        assert_eq!(store.keys(), vec!["work:token"]);
        let loaded = get_token_from(&store, "work").unwrap();
        assert_eq!(loaded.access_token, original.access_token);
        assert_eq!(loaded.refresh_token, original.refresh_token);
    }

    #[test]
    fn chunked_round_trip_splits_across_entries() {
        let store = MemoryStore::default();
        let original = token(&"a".repeat(200));
        let expected_chunks = serialized_len(&original).div_ceil(64);
        assert!(expected_chunks > 2);

        store_token_in(&store, "work", &original, Some(64)).unwrap();

        let mut expected_keys: Vec<String> = (0..expected_chunks)
            .map(|i| format!("work:token:{i}"))
            .collect();
        expected_keys.push("work:token".to_string());
        expected_keys.sort();
        assert_eq!(store.keys(), expected_keys);
        assert_eq!(
            store.get_password("work:token").unwrap().unwrap(),
            format!("{{\"chunks\":{expected_chunks}}}")
        );

        let loaded = get_token_from(&store, "work").unwrap();
        assert_eq!(loaded.access_token, original.access_token);
        assert_eq!(loaded.refresh_token, original.refresh_token);
        assert_eq!(loaded.scope, original.scope);
    }

    #[test]
    fn chunk_size_exactly_dividing_the_payload_adds_no_empty_chunk() {
        let store = MemoryStore::default();
        let original = token("x");
        let size = serialized_len(&original);

        store_token_in(&store, "work", &original, Some(size)).unwrap();

        assert_eq!(
            store.get_password("work:token").unwrap().unwrap(),
            "{\"chunks\":1}"
        );
        assert_eq!(store.keys(), vec!["work:token", "work:token:0"]);
        assert_eq!(get_token_from(&store, "work").unwrap().access_token, "x");
    }

    #[test]
    fn rewriting_a_smaller_token_removes_stale_chunks() {
        let store = MemoryStore::default();
        store_token_in(&store, "work", &token(&"a".repeat(500)), Some(64)).unwrap();
        let before = store.keys().len();

        store_token_in(&store, "work", &token("short"), Some(64)).unwrap();

        let after = store.keys();
        assert!(after.len() < before);
        let expected_chunks = serialized_len(&token("short")).div_ceil(64);
        assert_eq!(after.len(), expected_chunks + 1);
        assert_eq!(
            get_token_from(&store, "work").unwrap().access_token,
            "short"
        );
    }

    #[test]
    fn legacy_single_entry_is_readable() {
        let store = MemoryStore::default();
        let original = token("legacy");
        store
            .set_password("work:token", &serde_json::to_string(&original).unwrap())
            .unwrap();

        assert_eq!(
            get_token_from(&store, "work").unwrap().access_token,
            "legacy"
        );
    }

    #[test]
    fn missing_chunk_reports_incomplete_token() {
        let store = MemoryStore::default();
        store_token_in(&store, "work", &token(&"a".repeat(300)), Some(64)).unwrap();
        store.remove("work:token:1");

        let err = get_token_from(&store, "work").unwrap_err();
        let message = err.to_string();
        assert!(message.contains("incomplete"), "{message}");
        assert!(message.contains("teams auth login"), "{message}");
    }

    #[test]
    fn missing_primary_entry_is_an_error() {
        let store = MemoryStore::default();
        assert!(get_token_from(&store, "work").is_err());
    }

    #[test]
    fn delete_removes_header_and_every_chunk() {
        let store = MemoryStore::default();
        store_token_in(&store, "work", &token(&"a".repeat(300)), Some(64)).unwrap();
        store_token_in(&store, "other", &token("keep"), Some(64)).unwrap();

        delete_token_from(&store, "work", true).unwrap();

        assert!(store
            .keys()
            .iter()
            .all(|key| key.starts_with("other:token")));
        assert_eq!(
            get_token_from(&store, "other").unwrap().access_token,
            "keep"
        );
    }

    #[test]
    fn delete_sweeps_chunks_even_without_a_header() {
        let store = MemoryStore::default();
        store_token_in(&store, "work", &token(&"a".repeat(300)), Some(64)).unwrap();
        store.remove("work:token");

        delete_token_from(&store, "work", true).unwrap();

        assert!(store.keys().is_empty());
    }

    #[test]
    fn delete_without_chunking_touches_only_the_primary_entry() {
        let store = MemoryStore::default();
        store_token_in(&store, "work", &token("single"), None).unwrap();
        store.set_secret("work:token:0", b"unrelated").unwrap();

        delete_token_from(&store, "work", false).unwrap();

        assert_eq!(store.keys(), vec!["work:token:0"]);
    }

    #[test]
    fn delete_attempts_every_recorded_chunk_despite_a_gap() {
        let store = MemoryStore::default();
        store_token_in(&store, "work", &token(&"a".repeat(300)), Some(64)).unwrap();
        assert!(store.keys().len() > 3);
        store.remove("work:token:0");

        delete_token_from(&store, "work", true).unwrap();

        assert!(store.keys().is_empty());
    }

    #[test]
    fn delete_continues_past_a_gap_at_the_last_recorded_index() {
        let store = MemoryStore::default();
        store.set_password("work:token", "{\"chunks\":3}").unwrap();
        for index in [0, 1, 3, 4, 5] {
            store
                .set_secret(&format!("work:token:{index}"), b"x")
                .unwrap();
        }

        delete_token_from(&store, "work", true).unwrap();

        assert!(store.keys().is_empty(), "{:?}", store.keys());
    }

    #[test]
    fn shrinking_rewrite_removes_recorded_chunks_despite_a_gap() {
        let store = MemoryStore::default();
        store_token_in(&store, "work", &token(&"a".repeat(500)), Some(64)).unwrap();
        let small_chunks = serialized_len(&token("short")).div_ceil(64);
        store.remove(&format!("work:token:{small_chunks}"));

        store_token_in(&store, "work", &token("short"), Some(64)).unwrap();

        assert_eq!(store.keys().len(), small_chunks + 1);
    }

    #[test]
    fn multi_byte_characters_survive_splitting_at_arbitrary_byte_offsets() {
        let store = MemoryStore::default();
        let mut original = token("tok");
        original.scope = Some("Ünïcödé ✓ 日本語".to_string());
        original.profile = "ñ".to_string();

        for size in 1..=5 {
            store_token_in(&store, "work", &original, Some(size)).unwrap();
            let loaded = get_token_from(&store, "work").unwrap();
            assert_eq!(loaded.scope, original.scope, "chunk size {size}");
            assert_eq!(loaded.profile, original.profile, "chunk size {size}");
        }
    }
}
