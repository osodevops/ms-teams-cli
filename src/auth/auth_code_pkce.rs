use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Response, StatusCode, Uri};
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::mpsc;

use super::token::MsTokenResponse;
use crate::config::DEFAULT_DELEGATED_SCOPES;
use crate::error::{Result, TeamsError};

/// How long the browser login waits for the redirect back to the CLI.
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(10 * 60);

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

/// What the redirect back to the loopback server carried.
enum Callback {
    Code(String),
    Error(String),
}

/// Serve the redirect URI on `listeners` until a request to `path` carries
/// `state` and either an authorization code or an error, and return the code.
///
/// Every connection on every listener is served, and anything else is
/// answered and ignored: a browser's speculative connection, a favicon
/// request, or a callback with another state cannot end the wait. An error
/// from the identity platform fails the login with its description, and so
/// does `timeout` passing without a callback.
async fn receive_callback(
    listeners: Vec<tokio::net::TcpListener>,
    path: String,
    state: String,
    timeout: Duration,
) -> Result<String> {
    use hyper::service::service_fn;
    use hyper_util::rt::TokioIo;

    let (tx, mut rx) = mpsc::channel::<Callback>(1);
    let mut accept_loops = tokio::task::JoinSet::new();
    for listener in listeners {
        let (tx, path, state) = (tx.clone(), path.clone(), state.clone());
        accept_loops.spawn(async move {
            loop {
                let stream = match listener.accept().await {
                    Ok((stream, _)) => stream,
                    Err(e) => {
                        tracing::debug!("Login callback accept failed: {e}");
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        continue;
                    }
                };
                let (tx, path, state) = (tx.clone(), path.clone(), state.clone());
                tokio::spawn(async move {
                    let service = service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
                        let reply = answer_callback(req.uri(), &path, &state, &tx);
                        async move { Ok::<_, std::convert::Infallible>(reply) }
                    });
                    let _ = hyper::server::conn::http1::Builder::new()
                        .serve_connection(TokioIo::new(stream), service)
                        .await;
                });
            }
        });
    }
    drop(tx);

    let received = tokio::time::timeout(timeout, rx.recv()).await;
    accept_loops.abort_all();
    match received {
        Ok(Some(Callback::Code(code))) => Ok(code),
        Ok(Some(Callback::Error(message))) => Err(TeamsError::AuthError(message)),
        Ok(None) => Err(TeamsError::AuthError(
            "The login callback server stopped before the browser returned".into(),
        )),
        Err(_) => {
            let secs = timeout.as_secs();
            let waited = if secs >= 60 {
                format!("{} minutes", secs / 60)
            } else {
                format!("{secs} seconds")
            };
            Err(TeamsError::AuthError(format!(
                "No sign-in came back from the browser within {waited}. Run \
                 `teams auth login` again, or use `teams auth login --device-code`."
            )))
        }
    }
}

/// The loopback server's reply to one request. A request to the callback
/// path with this login's state sends its code or error on `tx`.
fn answer_callback(
    uri: &Uri,
    path: &str,
    state: &str,
    tx: &mpsc::Sender<Callback>,
) -> Response<Full<Bytes>> {
    if uri.path() != path {
        return callback_page(StatusCode::NOT_FOUND, "Not found.");
    }
    let params: HashMap<String, String> =
        url::form_urlencoded::parse(uri.query().unwrap_or("").as_bytes())
            .into_owned()
            .collect();
    if params.get("state").map(String::as_str) != Some(state) {
        return callback_page(
            StatusCode::BAD_REQUEST,
            "This sign-in response is not for the login in progress.",
        );
    }
    if let Some(code) = params.get("code") {
        let _ = tx.try_send(Callback::Code(code.clone()));
        return callback_page(
            StatusCode::OK,
            "Authentication successful! You can close this tab.",
        );
    }
    if let Some(error) = params.get("error") {
        // Entra's descriptions span lines (trace and correlation IDs).
        let message = match params.get("error_description") {
            Some(description) => format!(
                "Sign-in failed: {error}: {}",
                description.split_whitespace().collect::<Vec<_>>().join(" ")
            ),
            None => format!("Sign-in failed: {error}"),
        };
        let page = format!("{message}\n\nReturn to the terminal.");
        let _ = tx.try_send(Callback::Error(message));
        return callback_page(StatusCode::OK, &page);
    }
    callback_page(StatusCode::BAD_REQUEST, "Missing code or state parameter.")
}

fn callback_page(status: StatusCode, text: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("Content-Type", "text/plain; charset=utf-8")
        .header("Connection", "close")
        .body(Full::new(Bytes::from(text.to_string())))
        .expect("static response parts are valid")
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

    let (listeners, port) = bind_loopback(addrs).await?;
    let redirect_uri = redirect_uri_with_port(redirect_uri, port)?;
    let callback_path = url::Url::parse(&redirect_uri)
        .map_err(|e| {
            TeamsError::InvalidInput(format!("Invalid redirect URI '{redirect_uri}': {e}"))
        })?
        .path()
        .to_string();
    let auth_url = build_authorize_url(
        client_id,
        tenant_id,
        scopes,
        &state,
        &challenge,
        &redirect_uri,
    );

    // The listeners are bound, so a fast redirect waits in their backlog
    // until the callback server below accepts it.
    eprintln!("Opening browser for authentication...");
    if webbrowser::open(&auth_url).is_err() {
        eprintln!("Could not open browser. Please visit this URL manually:");
        eprintln!("{auth_url}");
    }

    let code = receive_callback(listeners, callback_path, state, CALLBACK_TIMEOUT).await?;

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

    /// One HTTP/1.1 GET to `addr`; returns the status and body.
    async fn get(addr: SocketAddr, target: &str) -> (u16, String) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        let request =
            format!("GET {target} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.unwrap();
        let text = String::from_utf8_lossy(&buf).into_owned();
        let status = text.split(' ').nth(1).unwrap().parse().unwrap();
        let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
        (status, body)
    }

    /// A callback server for path `/callback` and state `st4te`.
    async fn callback_server(
        timeout: Duration,
    ) -> (SocketAddr, tokio::task::JoinHandle<Result<String>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let login = tokio::spawn(receive_callback(
            vec![listener],
            "/callback".into(),
            "st4te".into(),
            timeout,
        ));
        (addr, login)
    }

    #[tokio::test]
    async fn callback_waits_past_stray_connections_and_other_requests() {
        let (addr, login) = callback_server(Duration::from_secs(30)).await;

        // A speculative connection that never sends a request.
        let _idle = tokio::net::TcpStream::connect(addr).await.unwrap();
        assert_eq!(get(addr, "/favicon.ico").await.0, 404);
        assert_eq!(get(addr, "/callback?code=forged&state=other").await.0, 400);
        assert_eq!(get(addr, "/callback?state=st4te").await.0, 400);
        assert!(!login.is_finished());

        let (status, body) = get(addr, "/callback?code=c0de&state=st4te").await;
        assert_eq!(status, 200);
        assert!(body.contains("successful"), "{body}");
        assert_eq!(login.await.unwrap().unwrap(), "c0de");
    }

    #[tokio::test]
    async fn an_error_redirect_fails_the_login_with_its_description() {
        let (addr, login) = callback_server(Duration::from_secs(30)).await;

        // Another login's error is not this one's.
        let other = "/callback?error=access_denied&state=other";
        assert_eq!(get(addr, other).await.0, 400);
        assert!(!login.is_finished());

        let target = "/callback?error=access_denied&error_description=\
                      AADSTS65004%3A+User+declined+to+consent.%0D%0ATrace+ID%3A+abc&state=st4te";
        let (status, body) = get(addr, target).await;
        assert_eq!(status, 200);
        assert!(body.contains("access_denied"), "{body}");

        let err = login.await.unwrap().unwrap_err();
        assert_eq!(err.exit_code(), 3);
        let message = err.to_string();
        assert!(
            message.contains("access_denied: AADSTS65004: User declined to consent. Trace ID: abc"),
            "{message}"
        );
    }

    #[tokio::test]
    async fn waiting_for_the_callback_times_out() {
        let (_addr, login) = callback_server(Duration::from_millis(200)).await;
        let err = login.await.unwrap().unwrap_err();
        assert_eq!(err.exit_code(), 3);
        assert!(err.to_string().contains("teams auth login"), "{err}");
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
