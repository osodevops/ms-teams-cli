use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use bytes::Bytes;
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::oneshot;

use super::token::MsTokenResponse;
use crate::config::DEFAULT_DELEGATED_SCOPES;
use crate::error::{Result, TeamsError};

fn random_urlsafe_bytes(len: usize) -> Result<String> {
    let mut bytes = vec![0u8; len];
    getrandom::fill(&mut bytes)
        .map_err(|e| TeamsError::AuthError(format!("Failed to generate random bytes: {e}")))?;
    Ok(URL_SAFE_NO_PAD.encode(&bytes))
}

fn generate_pkce() -> Result<(String, String)> {
    let verifier = random_urlsafe_bytes(32)?;
    let challenge = s256_challenge(&verifier);

    Ok((verifier, challenge))
}

/// The S256 code challenge for a verifier (RFC 7636 section 4.2).
fn s256_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hasher.finalize())
}

/// Build the authorization URL for the browser login.
///
/// `prompt=select_account` forces the account picker. Without it the identity
/// platform silently reuses an existing browser session, which makes it
/// impossible to sign a second account into another profile.
fn build_authorize_url(
    client_id: &str,
    tenant_id: &str,
    scopes: &str,
    state: &str,
    challenge: &str,
    redirect_uri: &str,
) -> String {
    format!(
        "https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/authorize?\
         client_id={client_id}\
         &response_type=code\
         &redirect_uri={}\
         &scope={}\
         &state={state}\
         &code_challenge={challenge}\
         &code_challenge_method=S256\
         &prompt=select_account",
        urlencoding::encode(redirect_uri),
        urlencoding::encode(scopes),
    )
}

/// The loopback socket addresses the redirect URI names. `localhost` can
/// resolve to ::1 or 127.0.0.1 depending on the browser, so both are
/// returned and the login listens on each. A URI without a port, or with
/// port 0 or 80, leaves the port to the OS (the url crate reports port 80 as
/// none), so those addresses carry port 0.
pub(crate) fn loopback_addrs(redirect_uri: &str) -> Result<Vec<SocketAddr>> {
    let url = url::Url::parse(redirect_uri).map_err(|e| {
        TeamsError::InvalidInput(format!("Invalid redirect URI '{redirect_uri}': {e}"))
    })?;
    let reject = || {
        TeamsError::InvalidInput(format!(
            "Redirect URI '{redirect_uri}' must use http and point at localhost or a loopback address"
        ))
    };
    // Only `localhost` and IP literals are resolved, so a typo never becomes a DNS lookup.
    let host_ok = match url.host() {
        Some(url::Host::Domain(name)) => name == "localhost",
        Some(_) => true,
        None => false,
    };
    if url.scheme() != "http" || !host_ok {
        return Err(reject());
    }
    let port = url.port().unwrap_or(0);
    let addrs: Vec<SocketAddr> = url
        .socket_addrs(|| None)
        .map_err(|e| {
            TeamsError::InvalidInput(format!("Invalid redirect URI '{redirect_uri}': {e}"))
        })?
        .into_iter()
        .filter(|addr| addr.ip().is_loopback())
        .map(|mut addr| {
            addr.set_port(port);
            addr
        })
        .collect();
    if addrs.is_empty() {
        return Err(reject());
    }
    Ok(addrs)
}

/// Listen on each address. A machine without IPv6 cannot bind `[::1]`, so one
/// bound address is enough, but a port held by another process is an error:
/// the browser could deliver the authorization code to that process. Port 0
/// asks the OS for a free port, which the other addresses then share, so the
/// redirect URI names one port whichever address the browser connects to.
/// Returns the listeners and that port.
async fn bind_loopback(addrs: Vec<SocketAddr>) -> Result<(Vec<tokio::net::TcpListener>, u16)> {
    let mut listeners = Vec::new();
    let mut port = 0;
    let mut skipped = None;
    for mut addr in addrs {
        if addr.port() == 0 {
            addr.set_port(port);
        }
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                port = listener
                    .local_addr()
                    .map_err(|e| {
                        TeamsError::AuthError(format!(
                            "Failed to bind loopback server on {addr}: {e}"
                        ))
                    })?
                    .port();
                listeners.push(listener);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                return Err(TeamsError::AuthError(format!(
                    "Failed to bind loopback server on {addr}: {e}"
                )))
            }
            Err(e) => {
                tracing::debug!("Not listening on {addr}: {e}");
                skipped = Some((addr, e));
            }
        }
    }
    if listeners.is_empty() {
        let (addr, e) = skipped
            .ok_or_else(|| TeamsError::AuthError("No loopback address to listen on".into()))?;
        return Err(TeamsError::AuthError(format!(
            "Failed to bind loopback server on {addr}: {e}"
        )));
    }
    Ok((listeners, port))
}

/// The redirect URI with the listener's port in it, when the URI left the
/// port to the OS. A URI that names a port is sent exactly as given.
fn redirect_uri_with_port(redirect_uri: &str, port: u16) -> Result<String> {
    let mut url = url::Url::parse(redirect_uri).map_err(|e| {
        TeamsError::InvalidInput(format!("Invalid redirect URI '{redirect_uri}': {e}"))
    })?;
    if url.port().unwrap_or(0) != 0 {
        return Ok(redirect_uri.to_string());
    }
    url.set_port(Some(port))
        .map_err(|()| TeamsError::AuthError("Cannot set the redirect URI port".into()))?;
    Ok(url.to_string())
}

/// Authenticate using authorization code flow with PKCE.
/// Opens a browser and starts a loopback HTTP server to receive the callback.
pub async fn authenticate(
    client_id: &str,
    tenant_id: &str,
    scopes: Option<&str>,
    redirect_uri: &str,
) -> Result<MsTokenResponse> {
    let addrs = loopback_addrs(redirect_uri)?;
    let scopes = scopes.unwrap_or(DEFAULT_DELEGATED_SCOPES);
    let (verifier, challenge) = generate_pkce()?;

    let state = random_urlsafe_bytes(16)?;

    // Start loopback server
    let (tx, rx) = oneshot::channel::<String>();

    let (listeners, port) = bind_loopback(addrs).await?;
    let redirect_uri = redirect_uri_with_port(redirect_uri, port)?;
    let auth_url = build_authorize_url(
        client_id,
        tenant_id,
        scopes,
        &state,
        &challenge,
        &redirect_uri,
    );

    let tx = Arc::new(Mutex::new(Some(tx)));
    let mut server_handles = Vec::new();
    for listener in listeners {
        let expected_state = state.clone();
        let tx = Arc::clone(&tx);
        server_handles.push(tokio::spawn(async move {
            use http_body_util::Full;
            use hyper::body::Incoming;
            use hyper::service::service_fn;
            use hyper::{Request, Response};
            use hyper_util::rt::TokioIo;

            if let Ok((stream, _)) = listener.accept().await {
                let io = TokioIo::new(stream);
                let expected = expected_state.clone();
                let tx = Arc::clone(&tx);

                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(
                        io,
                        service_fn(move |req: Request<Incoming>| {
                            let expected = expected.clone();
                            let tx = Arc::clone(&tx);
                            async move {
                                let query = req.uri().query().unwrap_or("");
                                let params: Vec<(String, String)> =
                                    url::form_urlencoded::parse(query.as_bytes())
                                        .into_owned()
                                        .collect();

                                let code = params
                                    .iter()
                                    .find(|(k, _)| k == "code")
                                    .map(|(_, v)| v.clone());
                                let state = params
                                    .iter()
                                    .find(|(k, _)| k == "state")
                                    .map(|(_, v)| v.clone());

                                if let (Some(code), Some(state)) = (code, state) {
                                    if state == expected {
                                        if let Some(tx) = tx.lock().unwrap().take() {
                                            let _ = tx.send(code);
                                        }
                                        Ok::<_, hyper::Error>(Response::new(Full::new(Bytes::from(
                                        "Authentication successful! You can close this tab.",
                                    ))))
                                    } else {
                                        Ok(Response::new(Full::new(Bytes::from(
                                            "State mismatch error.",
                                        ))))
                                    }
                                } else {
                                    Ok(Response::new(Full::new(Bytes::from(
                                        "Missing code or state parameter.",
                                    ))))
                                }
                            }
                        }),
                    )
                    .await
                    .ok();
            }
        }));
    }
    // Only the server tasks hold the sender now, so if every accepted
    // connection ends without an authorization code, waiting fails instead
    // of hanging.
    drop(tx);

    // Open browser
    eprintln!("Opening browser for authentication...");
    if webbrowser::open(&auth_url).is_err() {
        eprintln!("Could not open browser. Please visit this URL manually:");
        eprintln!("{auth_url}");
    }

    // Wait for callback
    let code = rx
        .await
        .map_err(|_| TeamsError::AuthError("Failed to receive auth code from callback".into()))?;

    for handle in server_handles {
        handle.abort();
    }

    // Exchange code for token
    let token_url = format!("https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token");

    let http = Client::new();
    let resp = http
        .post(&token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("code", &code),
            ("redirect_uri", redirect_uri.as_str()),
            ("code_verifier", &verifier),
            ("scope", scopes),
        ])
        .send()
        .await
        .map_err(TeamsError::NetworkError)?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(TeamsError::AuthError(format!(
            "Token exchange failed: {body}"
        )));
    }

    resp.json::<MsTokenResponse>().await.map_err(|e| {
        TeamsError::AuthError(format!(
            "Failed to parse token response: {}",
            crate::error::describe_with_causes(&e)
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorize_url_contains_expected_parameters() {
        let url = build_authorize_url(
            "client-1",
            "tenant-1",
            "User.Read offline_access",
            "st4te",
            "ch4llenge",
            "http://localhost:8400",
        );

        assert!(
            url.starts_with("https://login.microsoftonline.com/tenant-1/oauth2/v2.0/authorize?")
        );
        assert!(url.contains("client_id=client-1"));
        assert!(url.contains("&response_type=code"));
        assert!(url.contains("&redirect_uri=http%3A%2F%2Flocalhost%3A8400"));
        assert!(url.contains("&scope=User.Read%20offline_access"));
        assert!(url.contains("&state=st4te"));
        assert!(url.contains("&code_challenge=ch4llenge"));
        assert!(url.contains("&code_challenge_method=S256"));
    }

    #[test]
    fn s256_challenge_matches_rfc7636_example() {
        // RFC 7636 Appendix B.
        assert_eq!(
            s256_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn generated_verifier_is_43_unreserved_chars() {
        let (verifier, challenge) = generate_pkce().unwrap();

        assert_eq!(verifier.len(), 43);
        assert!(verifier
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert_eq!(challenge, s256_challenge(&verifier));
    }

    #[test]
    fn authorize_url_forces_account_picker() {
        let url = build_authorize_url(
            "client-1",
            "tenant-1",
            "User.Read",
            "s",
            "c",
            "http://localhost:8400",
        );

        assert!(url.contains("&prompt=select_account"));
    }

    #[test]
    fn loopback_addrs_follow_the_redirect_uri() {
        let addrs = loopback_addrs("http://localhost:8400/callback").unwrap();
        assert!(!addrs.is_empty());
        assert!(addrs
            .iter()
            .all(|a| a.ip().is_loopback() && a.port() == 8400));

        assert_eq!(
            loopback_addrs("http://127.0.0.1:9400").unwrap(),
            vec!["127.0.0.1:9400".parse::<SocketAddr>().unwrap()]
        );
        assert_eq!(
            loopback_addrs("http://[::1]:8400").unwrap(),
            vec!["[::1]:8400".parse::<SocketAddr>().unwrap()]
        );
        for uri in [
            "http://localhost",
            "http://localhost:80/callback",
            "http://localhost:0",
        ] {
            assert!(
                loopback_addrs(uri).unwrap().iter().all(|a| a.port() == 0),
                "{uri}"
            );
        }
    }

    #[test]
    fn loopback_addrs_reject_other_uris() {
        for uri in [
            "https://localhost:8400",
            "http://10.0.0.1:8400",
            "localhost:8400",
            "not a url",
            "http://example.invalid:8400",
        ] {
            assert_eq!(loopback_addrs(uri).unwrap_err().exit_code(), 2, "{uri}");
        }
    }

    /// A busy port is an error, because another process would get the
    /// authorization code; an address this machine does not have, as `[::1]`
    /// on a machine without IPv6, is skipped. 192.0.2.1 (TEST-NET-1) is never
    /// a local address.
    #[tokio::test]
    async fn bind_loopback_skips_an_unavailable_address_but_not_a_busy_port() {
        let held = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let busy = held.local_addr().unwrap();
        let err = bind_loopback(vec![busy]).await.unwrap_err();
        assert_eq!(err.exit_code(), 3, "{err}");
        assert!(err.to_string().contains(&busy.to_string()), "{err}");

        let unavailable: SocketAddr = "192.0.2.1:0".parse().unwrap();
        let free: SocketAddr = "127.0.0.1:0".parse().unwrap();
        assert_eq!(
            bind_loopback(vec![unavailable, free])
                .await
                .unwrap()
                .0
                .len(),
            1
        );
        assert!(bind_loopback(vec![unavailable]).await.is_err());
    }

    /// Port 0 gets a free port from the OS, and every other address gets the
    /// same one, so the redirect URI names one port. [::1] may be skipped on a
    /// machine without IPv6, which is why only the bound ones are checked.
    #[tokio::test]
    async fn bind_loopback_shares_one_free_port() {
        let addrs: Vec<SocketAddr> =
            vec!["127.0.0.1:0".parse().unwrap(), "[::1]:0".parse().unwrap()];
        let (listeners, port) = bind_loopback(addrs).await.unwrap();
        assert_ne!(port, 0);
        assert!(!listeners.is_empty());
        assert!(listeners
            .iter()
            .all(|l| l.local_addr().unwrap().port() == port));
    }

    #[test]
    fn redirect_uri_gets_the_listener_port_only_when_it_names_none() {
        for (uri, want) in [
            ("http://localhost", "http://localhost:54321/"),
            (
                "http://localhost/callback",
                "http://localhost:54321/callback",
            ),
            (
                "http://localhost:8400/callback",
                "http://localhost:8400/callback",
            ),
            ("http://localhost:0", "http://localhost:54321/"),
        ] {
            assert_eq!(redirect_uri_with_port(uri, 54321).unwrap(), want, "{uri}");
        }
    }
}
