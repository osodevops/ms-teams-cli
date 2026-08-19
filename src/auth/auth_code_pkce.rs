use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use bytes::Bytes;
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::oneshot;

use super::token::MsTokenResponse;
use crate::config::{DEFAULT_DELEGATED_SCOPES, DEFAULT_REDIRECT_URI};
use crate::error::{Result, TeamsError};

fn random_urlsafe_bytes(len: usize) -> Result<String> {
    let mut bytes = vec![0u8; len];
    getrandom::fill(&mut bytes)
        .map_err(|e| TeamsError::AuthError(format!("Failed to generate random bytes: {e}")))?;
    Ok(URL_SAFE_NO_PAD.encode(&bytes))
}

fn generate_pkce() -> Result<(String, String)> {
    let verifier = random_urlsafe_bytes(32)?;

    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

    Ok((verifier, challenge))
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
        urlencoding::encode(DEFAULT_REDIRECT_URI),
        urlencoding::encode(scopes),
    )
}

/// Authenticate using authorization code flow with PKCE.
/// Opens a browser and starts a loopback HTTP server to receive the callback.
pub async fn authenticate(
    client_id: &str,
    tenant_id: &str,
    scopes: Option<&str>,
) -> Result<MsTokenResponse> {
    let scopes = scopes.unwrap_or(DEFAULT_DELEGATED_SCOPES);
    let (verifier, challenge) = generate_pkce()?;

    let state = random_urlsafe_bytes(16)?;

    let auth_url = build_authorize_url(client_id, tenant_id, scopes, &state, &challenge);

    // Start loopback server
    let (tx, rx) = oneshot::channel::<String>();
    let expected_state = state.clone();

    let addr: SocketAddr = "127.0.0.1:8400".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| TeamsError::AuthError(format!("Failed to bind loopback server: {e}")))?;

    let server_handle = tokio::spawn(async move {
        use http_body_util::Full;
        use hyper::body::Incoming;
        use hyper::service::service_fn;
        use hyper::{Request, Response};
        use hyper_util::rt::TokioIo;

        let tx = Arc::new(Mutex::new(Some(tx)));

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
    });

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

    server_handle.abort();

    // Exchange code for token
    let token_url = format!("https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token");

    let http = Client::new();
    let resp = http
        .post(&token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("code", &code),
            ("redirect_uri", DEFAULT_REDIRECT_URI),
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

    resp.json::<MsTokenResponse>()
        .await
        .map_err(|e| TeamsError::AuthError(format!("Failed to parse token response: {e}")))
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
        );

        assert!(
            url.starts_with("https://login.microsoftonline.com/tenant-1/oauth2/v2.0/authorize?")
        );
        assert!(url.contains("client_id=client-1"));
        assert!(url.contains("&response_type=code"));
        assert!(url.contains("&scope=User.Read%20offline_access"));
        assert!(url.contains("&state=st4te"));
        assert!(url.contains("&code_challenge=ch4llenge"));
        assert!(url.contains("&code_challenge_method=S256"));
    }

    #[test]
    fn authorize_url_forces_account_picker() {
        let url = build_authorize_url("client-1", "tenant-1", "User.Read", "s", "c");

        assert!(url.contains("&prompt=select_account"));
    }
}
