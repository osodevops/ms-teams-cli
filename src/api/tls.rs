//! The certificate roots every HTTP client trusts on top of the operating system's.
//!
//! Server certificates are checked by the operating system's own verifier (rustls-platform-verifier,
//! through reqwest's `rustls` feature): the macOS keychain, the Windows certificate store, the
//! system CA bundle on Linux. Two sets of roots are merged on top of it:
//!
//! - `SSL_CERT_FILE` / `SSL_CERT_DIR`, when set. The macOS and Windows verifiers never read them, so
//!   the root of a proxy that inspects TLS given there would otherwise be ignored. On Linux the
//!   verifier also reads them itself, in place of the system bundle, as OpenSSL does.
//! - On Linux, Mozilla's roots, so a static binary still verifies where the system has no CA bundle
//!   (a `scratch` container): the Linux verifier refuses to build a client without any root.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::CertificateDer;

/// The roots to merge into a client's verifier.
///
/// `SSL_CERT_FILE` / `SSL_CERT_DIR` are read once per process, so a run that builds a second client
/// (a token refresh, then Graph) neither reads them nor warns about them twice. The error is the
/// reason the bundle cannot be used.
pub fn extra_roots() -> std::result::Result<Vec<reqwest::Certificate>, String> {
    static ENV_ROOTS: OnceLock<std::result::Result<Vec<CertificateDer<'static>>, String>> =
        OnceLock::new();
    let env_roots = ENV_ROOTS
        .get_or_init(|| {
            let file = std::env::var_os("SSL_CERT_FILE").map(PathBuf::from);
            let dirs: Vec<PathBuf> = std::env::var_os("SSL_CERT_DIR")
                .map(|dirs| {
                    std::env::split_paths(&dirs)
                        .filter(|dir| !dir.as_os_str().is_empty())
                        .collect()
                })
                .unwrap_or_default();
            load_pem_roots(file.as_deref(), &dirs)
        })
        .clone()?;

    #[cfg(target_os = "linux")]
    let built_in = webpki_root_certs::TLS_SERVER_ROOT_CERTS;
    #[cfg(not(target_os = "linux"))]
    let built_in: &[CertificateDer<'static>] = &[];

    env_roots
        .iter()
        .chain(built_in)
        .map(|der| reqwest::Certificate::from_der(der).map_err(|e| e.to_string()))
        .collect()
}

/// Read the PEM certificates in `file` and in the files directly inside `dirs`, keeping those that
/// can serve as trust anchors, each once.
///
/// It is as forgiving as the operating systems' own loaders: a path that cannot be read and a
/// certificate that does not parse are skipped with a warning, because one stale entry in a bundle
/// should not stop every request. A bundle in which certificates were found but none is usable is
/// an error: trusting nothing from it would fail every request through the proxy it was meant for,
/// with a message about the server's certificate rather than about the bundle.
fn load_pem_roots(
    file: Option<&Path>,
    dirs: &[PathBuf],
) -> std::result::Result<Vec<CertificateDer<'static>>, String> {
    let mut paths: Vec<PathBuf> = file.map(Path::to_path_buf).into_iter().collect();
    for dir in dirs {
        match std::fs::read_dir(dir) {
            Ok(entries) => {
                let mut files: Vec<PathBuf> = entries
                    .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                    .filter(|path| path.is_file())
                    .collect();
                files.sort();
                paths.extend(files);
            }
            Err(e) => tracing::warn!("SSL_CERT_DIR: cannot read {}: {e}", dir.display()),
        }
    }

    let mut seen = HashSet::new();
    let mut usable = Vec::new();
    let mut unusable = 0usize;
    for path in &paths {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::warn!("cannot read the certificates in {}: {e}", path.display());
                continue;
            }
        };
        for item in CertificateDer::pem_slice_iter(&bytes) {
            match item {
                Ok(der) if rustls::RootCertStore::empty().add(der.clone()).is_ok() => {
                    if seen.insert(der.as_ref().to_vec()) {
                        usable.push(der);
                    }
                }
                _ => unusable += 1,
            }
        }
    }

    if usable.is_empty() && unusable > 0 {
        return Err(format!(
            "SSL_CERT_FILE / SSL_CERT_DIR name {unusable} certificate(s) and none can be used; \
             they have to be PEM-encoded X.509 certificates"
        ));
    }
    if unusable > 0 {
        tracing::warn!(
            "skipped {unusable} certificate(s) in SSL_CERT_FILE / SSL_CERT_DIR that do not parse"
        );
    }
    Ok(usable)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root_pem(name: &str) -> String {
        let key = rcgen::KeyPair::generate().unwrap();
        let mut params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, name);
        params.self_signed(&key).unwrap().pem()
    }

    const NOT_A_CERTIFICATE: &str =
        "-----BEGIN CERTIFICATE-----\nbm90IGEgY2VydGlmaWNhdGU=\n-----END CERTIFICATE-----\n";

    /// A bundle holds several roots; every one of them is trusted, not only the first.
    #[test]
    fn every_certificate_in_a_bundle_is_loaded() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join("bundle.pem");
        std::fs::write(&bundle, root_pem("one") + &root_pem("two")).unwrap();

        assert_eq!(load_pem_roots(Some(&bundle), &[]).unwrap().len(), 2);
    }

    /// One stale entry must not stop every request: it is skipped and the rest are trusted.
    #[test]
    fn a_certificate_that_does_not_parse_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join("bundle.pem");
        std::fs::write(&bundle, root_pem("good") + NOT_A_CERTIFICATE).unwrap();

        assert_eq!(load_pem_roots(Some(&bundle), &[]).unwrap().len(), 1);
    }

    /// A bundle with certificates but nothing usable would silently trust nothing from it.
    #[test]
    fn a_bundle_with_nothing_usable_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join("bundle.pem");
        std::fs::write(&bundle, NOT_A_CERTIFICATE).unwrap();

        let err = load_pem_roots(Some(&bundle), &[]).unwrap_err();
        assert!(err.contains("SSL_CERT_FILE"), "{err}");
    }

    /// A path that does not exist is skipped, as the system loaders skip it, rather than failing.
    #[test]
    fn a_missing_file_or_directory_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.pem");

        assert!(
            load_pem_roots(Some(&missing), &[dir.path().join("missing-dir")])
                .unwrap()
                .is_empty()
        );
    }

    /// An OpenSSL-style directory holds each root under its own name and often again under a
    /// hash link; every file is read and each root kept once. Files without a certificate are
    /// ignored.
    #[test]
    fn a_directory_is_read_file_by_file_and_duplicates_are_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let root = root_pem("dir");
        std::fs::write(dir.path().join("root.pem"), &root).unwrap();
        std::fs::write(dir.path().join("abcd1234.0"), &root).unwrap();
        std::fs::write(dir.path().join("other.pem"), root_pem("other")).unwrap();
        std::fs::write(dir.path().join("README"), "not PEM").unwrap();

        assert_eq!(
            load_pem_roots(None, &[dir.path().to_path_buf()])
                .unwrap()
                .len(),
            2
        );
    }

    /// A static binary must still verify where the system has no CA bundle, which the Linux
    /// verifier refuses to do without a root of its own.
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_merges_mozillas_roots() {
        assert!(extra_roots().unwrap().len() >= webpki_root_certs::TLS_SERVER_ROOT_CERTS.len());
    }
}
