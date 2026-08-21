use serde::{Deserialize, Serialize};

use super::token::TokenInfo;
use crate::error::{Result, TeamsError};

const SERVICE_NAME: &str = "teams-cli";
const DISABLE_KEYRING_ENV: &str = "TEAMS_CLI_DISABLE_KEYRING";

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
        ::keyring::Entry::new(SERVICE_NAME, key)
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

pub fn store_token(profile: &str, token: &TokenInfo) -> Result<()> {
    if disabled() {
        return Err(TeamsError::KeyringError("Keyring is disabled".into()));
    }
    store_token_in(&OsKeyring, profile, token, CHUNK_BYTES)
}

pub fn get_token(profile: &str) -> Result<TokenInfo> {
    if disabled() {
        return Err(TeamsError::KeyringError("Keyring is disabled".into()));
    }
    get_token_from(&OsKeyring, profile)
}

pub fn delete_token(profile: &str) -> Result<()> {
    if disabled() {
        return Ok(());
    }
    delete_token_from(&OsKeyring, profile, CHUNK_BYTES.is_some())
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
    let entry = match ::keyring::Entry::new(SERVICE_NAME, "profile-index") {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    match entry.get_password() {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => vec![],
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
    let entry = ::keyring::Entry::new(SERVICE_NAME, "profile-index")
        .map_err(|e| TeamsError::KeyringError(format!("Failed to create keyring entry: {e}")))?;
    entry
        .set_password(&json)
        .map_err(|e| TeamsError::KeyringError(format!("Failed to store index: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use super::*;

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
