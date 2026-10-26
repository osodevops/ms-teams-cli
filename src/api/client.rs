use std::time::Duration;

use chrono::{DateTime, NaiveDateTime, Utc};
use reqwest::{Client, Method, StatusCode};
use serde::de::DeserializeOwned;

use crate::auth::token::TokenInfo;
use crate::config::NetworkConfig;
use crate::error::{Result, TeamsError, UNCONFIRMED_HINT};
use crate::models::common::{Page, PageResponse};

/// Options for paginated API requests.
#[derive(Debug, Clone)]
pub struct PaginationOpts {
    pub page_size: u64,
    pub all_pages: bool,
}

/// Microsoft Graph API client with retry and rate-limit handling.
#[derive(Debug, Clone)]
pub struct GraphClient {
    pub http: Client,
    pub token: TokenInfo,
    pub network: NetworkConfig,
}

impl GraphClient {
    pub fn new(token: TokenInfo, network: &NetworkConfig) -> Result<Self> {
        let timeout = Duration::from_secs(network.timeout);
        // A separate connect timeout makes a stalled connect a connect error, which
        // proves the request was never sent; the overall timeout alone cannot say.
        let http = Client::builder()
            .connect_timeout(timeout.min(CONNECT_TIMEOUT))
            .timeout(timeout)
            .build()
            .map_err(TeamsError::NetworkError)?;

        Ok(Self {
            http,
            token,
            network: network.clone(),
        })
    }

    /// Graph answers a request whose token lacks the necessary permission with 403 and, in the
    /// case observed for the presence writes, an empty `message`, so the body says nothing about
    /// what is missing and the failure is indistinguishable from a genuine authorization refusal.
    /// The permissions the token actually carries are the one piece of evidence the CLI already
    /// holds, and listing them turns "Forbidden" into something to check the operation against.
    fn permission_denied_message(&self, body: &str) -> String {
        let mut message = if body.trim().is_empty() {
            "Microsoft Graph returned 403 with no message".to_string()
        } else {
            body.to_string()
        };

        let Some(claims) = self.token.unverified_claims() else {
            return message;
        };

        // The remedy differs by token type and naming the wrong one sends the reader in a
        // direction that cannot work: an application role is granted on the app registration and
        // consented by an administrator, so no amount of interactive login will add one.
        let hint = match claims.auth_type() {
            "delegated" => claims
                .scp
                .clone()
                .filter(|scp| !scp.trim().is_empty())
                .map(|scp| {
                    format!(
                    "this token carries the delegated scopes: {scp}. If the one this operation \
                     needs is not in that list, run `teams auth doctor` to see what the profile \
                     resolves to, then `teams auth login` to consent to it — or, where the tenant \
                     does not allow user consent, `teams auth consent-url` for an administrator \
                     to grant."
                )
                }),
            "app-only" => claims
                .roles
                .clone()
                .filter(|roles| !roles.is_empty())
                .map(|roles| {
                    format!(
                        "this token carries the application roles: {}. If the one this operation \
                     needs is not in that list, add it to the app registration and have an \
                     administrator grant consent; logging in again cannot add an application \
                     role.",
                        roles.join(" ")
                    )
                }),
            _ => None,
        };

        if let Some(hint) = hint {
            message.push_str("\nHint: ");
            message.push_str(&hint);
        }

        message
    }

    fn auth_error_message(&self, body: &str) -> String {
        let mut message = if body.trim().is_empty() {
            "Authentication failed (401)".to_string()
        } else {
            format!("Authentication failed (401): {body}")
        };

        if body.contains("Invalid audience") || body.contains("InvalidAuthenticationToken") {
            let audience = self
                .token
                .unverified_claims()
                .and_then(|claims| claims.audience())
                .unwrap_or_else(|| "unknown or opaque token".to_string());

            message.push_str(&format!(
                "\nHint: Microsoft Graph rejected this token audience. This CLI requires a Microsoft Graph access token; observed audience: {audience}. Run `teams auth login`, `teams auth login --device-code`, or provide a Graph token via `TEAMS_CLI_ACCESS_TOKEN`. Teams client tokens such as `~/.config/fossteams/token-teams.jwt` are not supported."
            ));
        }

        message
    }

    fn error_for_status(&self, status: StatusCode, body: String) -> TeamsError {
        match status {
            StatusCode::UNAUTHORIZED => TeamsError::AuthError(self.auth_error_message(&body)),
            StatusCode::FORBIDDEN => {
                TeamsError::PermissionDenied(self.permission_denied_message(&body))
            }
            StatusCode::NOT_FOUND => TeamsError::NotFound(body),
            _ => TeamsError::ApiError {
                status: status.as_u16(),
                message: body,
            },
        }
    }

    /// GET request with retry logic.
    pub async fn get<T: DeserializeOwned>(&self, url: &str, query: &[(&str, &str)]) -> Result<T> {
        self.request_with_retry(PostKind::NotPost, |this| {
            this.http
                .get(url)
                .header("Authorization", this.token.bearer_header())
                .query(query)
        })
        .await
    }

    /// GET request that returns raw JSON value.
    #[allow(dead_code)]
    pub async fn get_json(&self, url: &str, query: &[(&str, &str)]) -> Result<serde_json::Value> {
        self.get(url, query).await
    }

    /// GET with pagination support. Appends `$top` and optionally follows all pages.
    /// A single page keeps Graph's `@odata.nextLink`.
    pub async fn get_paged<T: DeserializeOwned>(
        &self,
        url: &str,
        query: &[(&str, &str)],
        pagination: &PaginationOpts,
    ) -> Result<Page<T>> {
        let top_str = pagination.page_size.to_string();
        let mut full_query: Vec<(&str, &str)> = query.to_vec();
        full_query.push(("$top", &top_str));

        self.get_paged_without_top(url, &full_query, pagination)
            .await
    }

    /// GET with pagination support for endpoints that reject `$top`.
    pub async fn get_paged_without_top<T: DeserializeOwned>(
        &self,
        url: &str,
        query: &[(&str, &str)],
        pagination: &PaginationOpts,
    ) -> Result<Page<T>> {
        if pagination.all_pages {
            Ok(Page::complete(self.get_all_pages(url, query).await?))
        } else {
            let resp: PageResponse<T> = self.get(url, query).await?;
            Ok(resp.into())
        }
    }

    /// POST request with JSON body.
    pub async fn post<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<T> {
        self.request_with_retry(PostKind::Write, |this| {
            this.http
                .post(url)
                .header("Authorization", this.token.bearer_header())
                .json(body)
        })
        .await
    }

    /// POST that only reads, such as `/search/query`. Graph performs nothing, so it retries like
    /// a GET.
    pub async fn post_read_only<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<T> {
        self.request_with_retry(PostKind::ReadOnly, |this| {
            this.http
                .post(url)
                .header("Authorization", this.token.bearer_header())
                .json(body)
        })
        .await
    }

    /// PATCH request with JSON body.
    ///
    /// Graph answers some updates (team, channel) with 204 No Content and others
    /// with the resource. An empty body is read back with a GET of the same URL.
    pub async fn patch<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<T> {
        let resp = self
            .send(false, |this| {
                this.http
                    .patch(url)
                    .header("Authorization", this.token.bearer_header())
                    .json(body)
            })
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(self.error_for_status(status, body));
        }

        // The update is applied from here on, so failures are general errors (exit 1)
        // saying so, not the category of what failed, which would suggest it was not.
        // An empty body is read back whatever its framing: no length, or chunked.
        let empty = status == StatusCode::NO_CONTENT;
        let bytes = if empty {
            Default::default()
        } else {
            resp.bytes().await.map_err(|e| {
                TeamsError::Other(anyhow::anyhow!(
                    "Update succeeded, but its response could not be read: {e}"
                ))
            })?
        };
        if empty || bytes.iter().all(u8::is_ascii_whitespace) {
            return self.get(url, &[]).await.map_err(|e| {
                TeamsError::Other(anyhow::anyhow!(
                    "Update succeeded, but reading it back failed: {e}"
                ))
            });
        }

        serde_json::from_slice(&bytes).map_err(|e| TeamsError::ApiError {
            status: status.as_u16(),
            message: format!(
                "Failed to parse API response: {}",
                crate::error::describe_with_causes(&e)
            ),
        })
    }

    /// PATCH request returning no content.
    ///
    /// Some Graph endpoints answer a successful PATCH with an empty body rather
    /// than the updated resource; editing a chat message is one. Decoding that
    /// as the resource type reports a parse failure on what was in fact a
    /// success, so those callers use this and read the resource back instead.
    pub async fn patch_no_content<B: serde::Serialize>(&self, url: &str, body: &B) -> Result<()> {
        self.request_with_retry_no_content(|this| {
            this.http
                .patch(url)
                .header("Authorization", this.token.bearer_header())
                .json(body)
        })
        .await
    }

    /// DELETE request returning no content.
    pub async fn delete(&self, url: &str) -> Result<()> {
        self.request_with_retry_no_content(|this| {
            this.http
                .delete(url)
                .header("Authorization", this.token.bearer_header())
        })
        .await
    }

    /// POST request returning no content (204).
    pub async fn post_no_content<B: serde::Serialize>(&self, url: &str, body: &B) -> Result<()> {
        self.request_with_retry_no_content(|this| {
            this.http
                .post(url)
                .header("Authorization", this.token.bearer_header())
                .json(body)
        })
        .await
    }

    /// POST with no request body, returning no content (204). For Graph
    /// actions documented as taking no body, such as `softDelete`.
    pub async fn post_empty_no_content(&self, url: &str) -> Result<()> {
        self.request_with_retry_no_content(|this| {
            this.http
                .post(url)
                .header("Authorization", this.token.bearer_header())
                .header(reqwest::header::CONTENT_LENGTH, 0)
        })
        .await
    }

    /// POST request returning the Location header (for 202 Accepted async operations).
    pub async fn post_for_location<B: serde::Serialize>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<Option<String>> {
        let resp = self
            .send(false, |this| {
                this.http
                    .post(url)
                    .header("Authorization", this.token.bearer_header())
                    .json(body)
            })
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(self.error_for_status(status, body));
        }

        let location = resp
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        Ok(location)
    }

    /// PUT raw bytes (for file upload).
    pub async fn put_bytes<T: DeserializeOwned>(
        &self,
        url: &str,
        bytes: Vec<u8>,
        content_type: &str,
    ) -> Result<T> {
        let url = url.to_string();
        let content_type = content_type.to_string();
        self.request_with_retry(PostKind::NotPost, move |this| {
            this.http
                .put(&url)
                .header("Authorization", this.token.bearer_header())
                .header("Content-Type", &content_type)
                .body(bytes.clone())
        })
        .await
    }

    /// GET raw bytes (for file download).
    pub async fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        self.get_bytes_with_content_type(url).await.map(|(b, _)| b)
    }

    /// GET raw bytes along with the response's Content-Type header. Hosted
    /// contents (`/$value`) only expose their MIME type via that header.
    pub async fn get_bytes_with_content_type(
        &self,
        url: &str,
    ) -> Result<(Vec<u8>, Option<String>)> {
        let resp = self
            .send(false, |this| {
                this.http
                    .get(url)
                    .header("Authorization", this.token.bearer_header())
            })
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(self.error_for_status(status, body));
        }

        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.split(';').next().unwrap_or(v).trim().to_string());
        let bytes = resp.bytes().await.map_err(TeamsError::NetworkError)?;
        Ok((bytes.to_vec(), content_type))
    }

    /// Follow @odata.nextLink chain, returning all items.
    pub async fn get_all_pages<T: DeserializeOwned>(
        &self,
        url: &str,
        query: &[(&str, &str)],
    ) -> Result<Vec<T>> {
        let pb = crate::output::progress::paging_bar();

        let mut all_items: Vec<T> = Vec::new();
        let first_page: PageResponse<T> = self.get(url, query).await?;
        all_items.extend(first_page.value);
        pb.inc(1);

        let mut next = first_page.next_link;
        while let Some(ref next_url) = next {
            let page: PageResponse<T> = self.get(next_url, &[]).await?;
            all_items.extend(page.value);
            pb.inc(1);
            next = page.next_link;
        }

        pb.finish_and_clear();
        Ok(all_items)
    }

    /// Exponential backoff before retry `attempt + 1`.
    fn backoff(&self, attempt: u32) -> Duration {
        let factor = self.network.retry_backoff_base.saturating_pow(attempt);
        BACKOFF_UNIT.saturating_mul(u32::try_from(factor).unwrap_or(u32::MAX))
    }

    /// Internal: send a request, retrying what is safe to retry, and return the first response
    /// that is neither a 429 nor a retryable 5xx.
    ///
    /// A POST is not idempotent, so unless `read_only` it is replayed only when Graph cannot
    /// have acted on it: on 429, and when no connection was made. Any other failure after sending
    /// it is returned with a hint to check before re-sending. Other methods, PATCH included,
    /// retry as before.
    async fn send<F>(&self, read_only: bool, build_request: F) -> Result<reqwest::Response>
    where
        F: Fn(&Self) -> reqwest::RequestBuilder,
    {
        let max_retries = self.network.max_retries;

        for attempt in 0..=max_retries {
            let req = build_request(self)
                .build()
                .map_err(TeamsError::NetworkError)?;
            let replayable = read_only || req.method() != Method::POST;
            let last = attempt == max_retries;

            let resp = match self.http.execute(req).await {
                Ok(r) => r,
                Err(e) => {
                    // reqwest's `is_connect` is hyper-util's `ErrorKind::Connect`, raised only
                    // while obtaining a connection (DNS, TCP, TLS), before the request is
                    // written. A timeout cannot say whether it was written, so it counts as sent.
                    if !replayable && !e.is_connect() {
                        return Err(TeamsError::UnconfirmedWrite(e));
                    }
                    if last {
                        return Err(TeamsError::NetworkError(e));
                    }
                    let delay = self.backoff(attempt);
                    tracing::warn!(
                        "Request failed (attempt {}/{}), retrying in {delay:?}: {e}",
                        attempt + 1,
                        max_retries + 1
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }
            };

            let status = resp.status();

            // Rate limiting (429): Graph did not perform the request.
            if status == StatusCode::TOO_MANY_REQUESTS {
                let wait = retry_after(&resp).unwrap_or(DEFAULT_RETRY_AFTER);
                if last {
                    return Err(TeamsError::RateLimited {
                        retry_after: wait.as_secs() + u64::from(wait.subsec_nanos() > 0),
                    });
                }
                tracing::warn!("Rate limited, waiting {wait:?}");
                tokio::time::sleep(wait).await;
                continue;
            }

            // Server errors: retry unless the request may have been performed.
            if status.is_server_error() {
                let wait = retry_after(&resp);
                let body = resp.text().await.unwrap_or_default();
                if !replayable {
                    let message = if body.trim().is_empty() {
                        UNCONFIRMED_HINT.to_string()
                    } else {
                        format!("{body}\n{UNCONFIRMED_HINT}")
                    };
                    return Err(TeamsError::ServerError {
                        status: status.as_u16(),
                        message,
                    });
                }
                if last {
                    return Err(TeamsError::ServerError {
                        status: status.as_u16(),
                        message: body,
                    });
                }
                let delay = wait.unwrap_or_else(|| self.backoff(attempt));
                tracing::warn!(
                    "Server error {status} (attempt {}/{}), retrying in {delay:?}",
                    attempt + 1,
                    max_retries + 1
                );
                tokio::time::sleep(delay).await;
                continue;
            }

            return Ok(resp);
        }

        Err(TeamsError::ApiError {
            status: 0,
            message: "Max retries exceeded".to_string(),
        })
    }

    /// Internal: execute a request with retry + rate-limit handling.
    async fn request_with_retry<T, F>(&self, kind: PostKind, build_request: F) -> Result<T>
    where
        T: DeserializeOwned,
        F: Fn(&Self) -> reqwest::RequestBuilder,
    {
        let resp = self.send(kind == PostKind::ReadOnly, build_request).await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(self.error_for_status(status, body));
        }

        decode_json(resp, kind == PostKind::Write).await
    }

    /// Internal: execute a request that returns no content (204).
    async fn request_with_retry_no_content<F>(&self, build_request: F) -> Result<()>
    where
        F: Fn(&Self) -> reqwest::RequestBuilder,
    {
        let resp = self.send(false, build_request).await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(self.error_for_status(status, body));
        }

        Ok(())
    }
}

/// Whether a request is a POST, and if so whether it writes. Decides replay and what an
/// unreadable 2xx body means.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PostKind {
    NotPost,
    ReadOnly,
    Write,
}

/// Decode a 2xx response body. For a write POST (`accepted_write`) the 2xx means Graph
/// performed it, so a body that cannot be read or parsed says so and warns against re-sending.
async fn decode_json<T: DeserializeOwned>(
    resp: reqwest::Response,
    accepted_write: bool,
) -> Result<T> {
    if accepted_write {
        let status = resp.status().as_u16();
        let bytes = resp
            .bytes()
            .await
            .map_err(|source| TeamsError::AcceptedUnread { status, source })?;
        return serde_json::from_slice(&bytes).map_err(|e| TeamsError::ApiError {
            status: 200,
            message: format!(
                "Failed to parse API response: {}\n{}",
                crate::error::describe_with_causes(&e),
                crate::error::accepted_hint(status)
            ),
        });
    }

    resp.json::<T>().await.map_err(|e| {
        if e.is_decode() {
            TeamsError::ApiError {
                status: 200,
                message: format!(
                    "Failed to parse API response: {}",
                    crate::error::describe_with_causes(&e)
                ),
            }
        } else {
            TeamsError::NetworkError(e)
        }
    })
}

/// Longest wait for a connection to Graph.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Longest wait honored from a `Retry-After` header.
const MAX_RETRY_AFTER: Duration = Duration::from_secs(300);

/// Wait on a 429 that names none.
const DEFAULT_RETRY_AFTER: Duration = Duration::from_secs(60);

/// Unit of the exponential backoff, shrunk in tests so retries run fast.
#[cfg(not(test))]
const BACKOFF_UNIT: Duration = Duration::from_secs(1);
#[cfg(test)]
const BACKOFF_UNIT: Duration = Duration::from_millis(1);

fn retry_after(resp: &reqwest::Response) -> Option<Duration> {
    let value = resp.headers().get(reqwest::header::RETRY_AFTER)?;
    parse_retry_after(value.to_str().ok()?, Utc::now())
}

/// Parse a `Retry-After` value, delta-seconds or HTTP-date, clamped to `MAX_RETRY_AFTER`.
/// A date in the past means no wait.
fn parse_retry_after(value: &str, now: DateTime<Utc>) -> Option<Duration> {
    let value = value.trim();
    let wait = match value.parse::<u64>() {
        Ok(secs) => Duration::from_secs(secs),
        Err(_) => {
            // IMF-fixdate, then the obsolete RFC 850 and asctime forms RFC 9110 still accepts.
            let at = DateTime::parse_from_rfc2822(value)
                .map(|d| d.with_timezone(&Utc))
                .or_else(|_| {
                    NaiveDateTime::parse_from_str(value, "%A, %d-%b-%y %H:%M:%S GMT")
                        .or_else(|_| NaiveDateTime::parse_from_str(value, "%a %b %e %H:%M:%S %Y"))
                        .map(|d| d.and_utc())
                })
                .ok()?;
            (at - now).to_std().unwrap_or(Duration::ZERO)
        }
    };
    Some(wait.min(MAX_RETRY_AFTER))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::token::TokenInfo;
    use crate::config::NetworkConfig;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use wiremock::matchers::{method, path, query_param, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// `unverified_claims` decodes the token's own payload, so a hint test needs a token shaped
    /// like a JWT rather than the opaque placeholder the other tests use.
    fn jwt_with(claims: serde_json::Value) -> String {
        use base64::Engine;
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
        format!("header.{payload}.signature")
    }

    fn test_client() -> GraphClient {
        GraphClient {
            http: Client::new(),
            token: TokenInfo {
                access_token: "test-token".into(),
                expires_at: None,
                token_type: "Bearer".into(),
                scope: None,
                refresh_token: None,
                profile: "default".into(),
            },
            network: NetworkConfig {
                timeout: 30,
                max_retries: 0,
                retry_backoff_base: 2,
            },
        }
    }

    /// A response Graph considers successful but this crate cannot deserialize is the one failure
    /// the user cannot investigate from the message alone, so the message has to carry serde's
    /// account of it: which value was wrong, what was expected, and where it sat.
    #[tokio::test]
    async fn a_schema_mismatch_reports_what_serde_objected_to() {
        #[derive(Debug, serde::Deserialize)]
        struct Expiring {
            #[allow(dead_code)]
            expiry_date_time: String,
        }

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me/presence"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "expiry_date_time": { "dateTime": "2026-09-01T08:00:00Z", "timeZone": "UTC" }
            })))
            .mount(&server)
            .await;

        let err = test_client()
            .get::<Expiring>(&format!("{}/me/presence", server.uri()), &[])
            .await
            .unwrap_err();

        let TeamsError::ApiError { status, message } = err else {
            panic!("expected an ApiError");
        };
        assert_eq!(status, 200);
        assert!(
            message.contains("invalid type: map, expected a string"),
            "message lost serde's account of the mismatch: {message}"
        );
    }

    /// Graph answered the presence writes with 403 and an empty `message`, leaving nothing to act
    /// on. The scopes the token carries are held locally, so the CLI can supply the list the user
    /// would otherwise have to go and read.
    #[tokio::test]
    async fn a_forbidden_with_no_message_lists_the_scopes_the_token_carries() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me/presence"))
            .respond_with(
                ResponseTemplate::new(403)
                    .set_body_string(r#"{"error":{"code":"Forbidden","message":""}}"#),
            )
            .mount(&server)
            .await;

        let mut client = test_client();
        client.token.access_token = jwt_with(serde_json::json!({
            "scp": "Presence.Read.All User.Read"
        }));

        let err = client
            .get::<serde_json::Value>(&format!("{}/me/presence", server.uri()), &[])
            .await
            .unwrap_err();

        let TeamsError::PermissionDenied(message) = err else {
            panic!("expected PermissionDenied");
        };
        assert!(message.contains("Presence.Read.All User.Read"), "{message}");
        assert!(message.contains("teams auth login"), "{message}");
        // A tenant that does not allow user consent needs the administrator route named too.
        assert!(message.contains("teams auth consent-url"), "{message}");
    }

    /// An app-only token carries roles rather than scopes, and naming the wrong one would send the
    /// reader looking for a delegated permission that cannot apply.
    #[tokio::test]
    async fn a_forbidden_on_an_app_only_token_lists_its_roles() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me/presence"))
            .respond_with(ResponseTemplate::new(403).set_body_string(""))
            .mount(&server)
            .await;

        let mut client = test_client();
        client.token.access_token = jwt_with(serde_json::json!({
            "roles": ["Presence.Read.All"]
        }));

        let err = client
            .get::<serde_json::Value>(&format!("{}/me/presence", server.uri()), &[])
            .await
            .unwrap_err();

        let TeamsError::PermissionDenied(message) = err else {
            panic!("expected PermissionDenied");
        };
        assert!(
            message.contains("application roles: Presence.Read.All"),
            "{message}"
        );
        assert!(
            message.contains("Microsoft Graph returned 403 with no message"),
            "{message}"
        );
        // The whole point of splitting the remedy: an interactive login cannot add an
        // application role, so reintroducing that advice here has to fail.
        assert!(
            message.contains("cannot add an application role"),
            "{message}"
        );
        assert!(!message.contains("teams auth login"), "{message}");
    }

    /// `post_no_content` reaches its own copy of the 403 handling, which a test driving `get`
    /// would never touch.
    #[tokio::test]
    async fn a_forbidden_on_a_no_content_write_carries_the_same_hint() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/me/presence/setPresence"))
            .respond_with(
                ResponseTemplate::new(403)
                    .set_body_string(r#"{"error":{"code":"Forbidden","message":""}}"#),
            )
            .mount(&server)
            .await;

        let mut client = test_client();
        client.token.access_token = jwt_with(serde_json::json!({
            "scp": "Presence.Read.All User.Read"
        }));

        let err = client
            .post_no_content(
                &format!("{}/me/presence/setPresence", server.uri()),
                &serde_json::json!({}),
            )
            .await
            .unwrap_err();

        let TeamsError::PermissionDenied(message) = err else {
            panic!("expected PermissionDenied");
        };
        assert!(message.contains("Presence.Read.All User.Read"), "{message}");
    }

    /// `error_for_status` is the third copy, and it is reached only by the byte and location
    /// paths — a paginated read routes through `request_with_retry` like any other GET, so a test
    /// driving `get_paged` would leave this copy untouched.
    #[tokio::test]
    async fn a_forbidden_on_a_byte_download_carries_the_same_hint() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/drives/drive-id/items/item-id/content"))
            .respond_with(ResponseTemplate::new(403).set_body_string(""))
            .mount(&server)
            .await;

        let mut client = test_client();
        client.token.access_token = jwt_with(serde_json::json!({
            "scp": "Files.Read.All"
        }));

        let err = client
            .get_bytes(&format!(
                "{}/drives/drive-id/items/item-id/content",
                server.uri()
            ))
            .await
            .unwrap_err();

        let TeamsError::PermissionDenied(message) = err else {
            panic!("expected PermissionDenied");
        };
        assert!(message.contains("Files.Read.All"), "{message}");
    }

    /// An opaque token yields no claims, and a hint invented without them would be a guess.
    #[tokio::test]
    async fn a_forbidden_on_an_opaque_token_adds_no_hint() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me/presence"))
            .respond_with(
                ResponseTemplate::new(403)
                    .set_body_string(r#"{"error":{"code":"Forbidden","message":"Access denied"}}"#),
            )
            .mount(&server)
            .await;

        let err = test_client()
            .get::<serde_json::Value>(&format!("{}/me/presence", server.uri()), &[])
            .await
            .unwrap_err();

        let TeamsError::PermissionDenied(message) = err else {
            panic!("expected PermissionDenied");
        };
        assert!(!message.contains("Hint:"), "{message}");
        assert!(message.contains("Access denied"), "{message}");
    }

    #[tokio::test]
    async fn get_paged_reproduces_graph_channel_top_rejection() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/teams/team-id/channels"))
            .and(query_param("$top", "50"))
            .respond_with(ResponseTemplate::new(400).set_body_string(
                r#"{"error":{"code":"BadRequest","message":"Unsupported query parameter 'top'."}}"#,
            ))
            .mount(&server)
            .await;

        let err = test_client()
            .get_paged::<serde_json::Value>(
                &format!("{}/teams/team-id/channels", server.uri()),
                &[],
                &PaginationOpts {
                    page_size: 50,
                    all_pages: false,
                },
            )
            .await
            .unwrap_err();

        assert!(
            matches!(err, TeamsError::ApiError { status: 400, message } if message.contains("Unsupported query parameter 'top'"))
        );
    }

    #[tokio::test]
    async fn get_paged_without_top_omits_top_from_first_page_request() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/teams/team-id/channels"))
            .and(query_param_is_missing("$top"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [
                    {
                        "id": "channel-id",
                        "displayName": "General",
                        "membershipType": "standard"
                    }
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let channels = test_client()
            .get_paged_without_top::<serde_json::Value>(
                &format!("{}/teams/team-id/channels", server.uri()),
                &[],
                &PaginationOpts {
                    page_size: 50,
                    all_pages: false,
                },
            )
            .await
            .unwrap()
            .items;

        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0]["displayName"], "General");
    }

    #[tokio::test]
    async fn get_paged_without_top_follows_next_link_when_all_pages_enabled() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/teams/team-id/channels"))
            .and(query_param_is_missing("$top"))
            .and(query_param_is_missing("page"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [{ "id": "channel-1" }],
                "@odata.nextLink": format!("{}/teams/team-id/channels?page=2", server.uri())
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/teams/team-id/channels"))
            .and(query_param("page", "2"))
            .and(query_param_is_missing("$top"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [{ "id": "channel-2" }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let channels = test_client()
            .get_paged_without_top::<serde_json::Value>(
                &format!("{}/teams/team-id/channels", server.uri()),
                &[],
                &PaginationOpts {
                    page_size: 50,
                    all_pages: true,
                },
            )
            .await
            .unwrap()
            .items;

        assert_eq!(channels.len(), 2);
        assert_eq!(channels[0]["id"], "channel-1");
        assert_eq!(channels[1]["id"], "channel-2");
    }

    fn retrying_client(max_retries: u32) -> GraphClient {
        let mut client = test_client();
        // The connect timeout mirrors production: on Windows a refused port is
        // retried for about two seconds, and must still end as a connect error.
        client.http = Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_millis(250))
            .timeout(Duration::from_millis(500))
            .build()
            .unwrap();
        client.network.max_retries = max_retries;
        client
    }

    async fn requests_seen(server: &MockServer) -> usize {
        server.received_requests().await.unwrap().len()
    }

    /// A 5xx answer to a POST does not say the write was not performed, so replaying it can post
    /// twice. Every POST path must give up after one request.
    #[tokio::test]
    async fn a_post_answered_with_a_5xx_is_sent_once() {
        for status in [500, 502, 503, 504] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(status))
                .mount(&server)
                .await;
            let client = retrying_client(2);
            let url = format!("{}/chats/c/messages", server.uri());
            let body = serde_json::json!({});

            let errors = [
                client.post::<serde_json::Value, _>(&url, &body).await.err(),
                client.post_no_content(&url, &body).await.err(),
                client.post_empty_no_content(&url).await.err(),
                client.post_for_location(&url, &body).await.err(),
            ];
            assert_eq!(requests_seen(&server).await, 4, "{status} was replayed");
            for err in errors {
                let err = err.expect("a 5xx must fail");
                assert_eq!(err.exit_code(), 8, "{status}: {err}");
                assert!(err.to_string().contains("check before"), "{err}");
            }
        }
    }

    #[tokio::test]
    async fn a_get_answered_with_a_503_is_retried() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        retrying_client(2)
            .get::<serde_json::Value>(&format!("{}/me", server.uri()), &[])
            .await
            .unwrap();
        assert_eq!(requests_seen(&server).await, 2);
    }

    #[tokio::test]
    async fn a_read_only_post_answered_with_a_503_is_retried() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        retrying_client(2)
            .post_read_only::<serde_json::Value, _>(
                &format!("{}/search/query", server.uri()),
                &serde_json::json!({}),
            )
            .await
            .unwrap();
        assert_eq!(requests_seen(&server).await, 2);
    }

    #[tokio::test]
    async fn patch_put_and_delete_answered_with_a_503_are_retried() {
        let server = MockServer::start().await;
        for verb in ["PATCH", "PUT", "DELETE"] {
            Mock::given(method(verb))
                .respond_with(ResponseTemplate::new(503))
                .up_to_n_times(1)
                .mount(&server)
                .await;
            Mock::given(method(verb))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
                .mount(&server)
                .await;
        }
        let client = retrying_client(2);
        let url = format!("{}/x", server.uri());
        let body = serde_json::json!({});

        client
            .patch::<serde_json::Value, _>(&url, &body)
            .await
            .unwrap();
        client
            .put_bytes::<serde_json::Value>(&url, b"x".to_vec(), "text/plain")
            .await
            .unwrap();
        client.delete(&url).await.unwrap();

        let requests = server.received_requests().await.unwrap();
        for verb in ["PATCH", "PUT", "DELETE"] {
            let sent = requests
                .iter()
                .filter(|r| r.method.as_str() == verb)
                .count();
            assert_eq!(sent, 2, "{verb} was not retried");
        }
    }

    /// 429 means Graph did not perform the request, so even a POST is retried, after the wait
    /// Graph asked for.
    #[tokio::test]
    async fn a_throttled_post_waits_for_retry_after_and_is_retried() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "1"))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": "m"})))
            .mount(&server)
            .await;

        let started = std::time::Instant::now();
        let sent: serde_json::Value = retrying_client(1)
            .post(
                &format!("{}/chats/c/messages", server.uri()),
                &serde_json::json!({}),
            )
            .await
            .unwrap();
        assert_eq!(sent["id"], "m");
        assert!(started.elapsed() >= Duration::from_secs(1));
        assert_eq!(requests_seen(&server).await, 2);
    }

    /// The HTTP-date form is honored on a retryable 5xx too. The date has whole seconds, so a
    /// date three seconds ahead is at least two ahead once truncated: longer than the backoff.
    #[tokio::test]
    async fn a_get_503_waits_for_an_http_date_retry_after() {
        let server = MockServer::start().await;
        let started = std::time::Instant::now();
        let at = (chrono::Utc::now() + chrono::Duration::seconds(3))
            .format("%a, %d %b %Y %H:%M:%S GMT")
            .to_string();
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503).insert_header("Retry-After", at.as_str()))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        retrying_client(1)
            .get::<serde_json::Value>(&format!("{}/me", server.uri()), &[])
            .await
            .unwrap();
        assert!(started.elapsed() >= Duration::from_secs(2));
        assert_eq!(requests_seen(&server).await, 2);
    }

    #[tokio::test]
    async fn a_huge_retry_after_is_clamped() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "86400"))
            .mount(&server)
            .await;

        let err = retrying_client(0)
            .post_no_content(&format!("{}/x", server.uri()), &serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(
            matches!(err, TeamsError::RateLimited { retry_after: 300 }),
            "{err:?}"
        );
    }

    #[test]
    fn retry_after_parses_seconds_and_http_dates() {
        let now = DateTime::parse_from_rfc3339("2015-10-21T07:28:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let parse = |v| parse_retry_after(v, now);

        assert_eq!(parse("120"), Some(Duration::from_secs(120)));
        assert_eq!(parse(" 0 "), Some(Duration::ZERO));
        assert_eq!(
            parse("Wed, 21 Oct 2015 07:29:30 GMT"),
            Some(Duration::from_secs(90))
        );
        assert_eq!(
            parse("Wednesday, 21-Oct-15 07:28:10 GMT"),
            Some(Duration::from_secs(10))
        );
        assert_eq!(
            parse("Wed Oct 21 07:28:05 2015"),
            Some(Duration::from_secs(5))
        );
        assert_eq!(parse("Wed, 21 Oct 2015 07:00:00 GMT"), Some(Duration::ZERO));
        assert_eq!(parse("86400"), Some(MAX_RETRY_AFTER));
        assert_eq!(
            parse("Thu, 22 Oct 2015 07:28:00 GMT"),
            Some(MAX_RETRY_AFTER)
        );
        assert_eq!(parse("soon"), None);
        assert_eq!(parse("-5"), None);
    }

    #[derive(Clone, Copy)]
    enum Fault {
        /// Read the request, then reset the connection.
        Reset,
        /// Read the request, send part of the status line and headers, then close.
        PartialHeaders,
        /// Read the request, then never answer.
        Stall,
        /// Read the request, send complete 201 headers and part of the body, then close.
        TruncatedBody,
        /// Read the request, send complete 201 headers and part of the body, then stall.
        StalledBody,
    }

    /// A server that reads each complete request, counts it, then misbehaves as `fault` says.
    /// Graph may have acted on a request by the time any of these happen.
    async fn fault_server(fault: Fault) -> (String, Arc<AtomicUsize>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/chats/c/messages", listener.local_addr().unwrap());
        let seen = Arc::new(AtomicUsize::new(0));
        let counter = seen.clone();
        tokio::spawn(async move {
            let mut stalled = Vec::new();
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                let complete = loop {
                    if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buf[..end]).to_ascii_lowercase();
                        let length = head
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length:"))
                            .map_or(0, |v| v.trim().parse::<usize>().unwrap());
                        if buf.len() >= end + 4 + length {
                            break true;
                        }
                    }
                    match stream.read(&mut chunk).await {
                        Ok(0) | Err(_) => break false,
                        Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    }
                };
                if !complete {
                    continue;
                }
                counter.fetch_add(1, Ordering::SeqCst);
                match fault {
                    Fault::Reset => {
                        #[allow(deprecated)]
                        stream.set_linger(Some(Duration::ZERO)).unwrap();
                    }
                    Fault::PartialHeaders => {
                        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Ty").await;
                    }
                    Fault::Stall => stalled.push(stream),
                    Fault::TruncatedBody | Fault::StalledBody => {
                        let _ = stream
                            .write_all(
                                b"HTTP/1.1 201 Created\r\nContent-Type: application/json\r\n\
                                  Content-Length: 100\r\n\r\n{\"id\":",
                            )
                            .await;
                        if matches!(fault, Fault::StalledBody) {
                            stalled.push(stream);
                        }
                    }
                }
            }
        });
        (url, seen)
    }

    async fn assert_post_sent_once_and_get_retried(fault: Fault) {
        let (url, seen) = fault_server(fault).await;
        let err = retrying_client(2)
            .post::<serde_json::Value, _>(&url, &serde_json::json!({"body": "hi"}))
            .await
            .unwrap_err();
        assert_eq!(seen.load(Ordering::SeqCst), 1, "the POST was replayed");
        assert!(matches!(err, TeamsError::UnconfirmedWrite(_)), "{err:?}");
        assert_eq!(err.exit_code(), 7, "{err}");
        assert_eq!(err.error_code(), "NETWORK_ERROR");
        assert!(err.to_string().contains("check before"), "{err}");

        let (url, seen) = fault_server(fault).await;
        let err = retrying_client(2)
            .get::<serde_json::Value>(&url, &[])
            .await
            .unwrap_err();
        assert_eq!(seen.load(Ordering::SeqCst), 3, "the GET was not retried");
        assert_eq!(err.exit_code(), 7, "{err}");
    }

    #[tokio::test]
    async fn a_reset_after_the_request_is_not_replayed_for_a_post() {
        assert_post_sent_once_and_get_retried(Fault::Reset).await;
    }

    /// 2xx headers mean Graph performed the POST, so a body that cannot be read must say so
    /// rather than leave the caller to re-send. A GET keeps the ordinary error.
    async fn assert_accepted_post_says_do_not_resend(fault: Fault) {
        let (url, seen) = fault_server(fault).await;
        let err = retrying_client(2)
            .post::<serde_json::Value, _>(&url, &serde_json::json!({"body": "hi"}))
            .await
            .unwrap_err();
        assert_eq!(seen.load(Ordering::SeqCst), 1, "the POST was replayed");
        let message = err.to_string();
        assert!(message.contains("accepted the request (201"), "{message}");
        assert!(message.contains("do not re-send"), "{message}");
        assert_eq!(err.exit_code(), 7, "{err}");
        assert_eq!(err.error_code(), "NETWORK_ERROR");

        let (url, seen) = fault_server(fault).await;
        let err = retrying_client(2)
            .get::<serde_json::Value>(&url, &[])
            .await
            .unwrap_err();
        assert_eq!(seen.load(Ordering::SeqCst), 1);
        assert!(
            err.to_string().contains("Failed to parse API response"),
            "{err}"
        );
        assert!(!err.to_string().contains("re-send"), "{err}");
    }

    #[tokio::test]
    async fn a_truncated_body_after_a_201_says_the_post_was_performed() {
        assert_accepted_post_says_do_not_resend(Fault::TruncatedBody).await;
    }

    #[tokio::test]
    async fn a_stalled_body_after_a_201_says_the_post_was_performed() {
        assert_accepted_post_says_do_not_resend(Fault::StalledBody).await;
    }

    /// A 2xx body that parses but does not match is also a performed write.
    #[tokio::test]
    async fn an_unparseable_201_says_the_post_was_performed() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(201).set_body_string("not json"))
            .mount(&server)
            .await;

        let err = retrying_client(2)
            .post::<serde_json::Value, _>(&format!("{}/x", server.uri()), &serde_json::json!({}))
            .await
            .unwrap_err();
        assert_eq!(requests_seen(&server).await, 1);
        let message = err.to_string();
        assert!(
            message.contains("Failed to parse API response"),
            "{message}"
        );
        assert!(message.contains("accepted the request (201"), "{message}");
        assert!(message.contains("do not re-send"), "{message}");
    }

    #[tokio::test]
    async fn a_reset_on_a_read_only_post_is_retried() {
        let (url, seen) = fault_server(Fault::Reset).await;
        let err = retrying_client(2)
            .post_read_only::<serde_json::Value, _>(&url, &serde_json::json!({}))
            .await
            .unwrap_err();
        assert_eq!(
            seen.load(Ordering::SeqCst),
            3,
            "the read-only POST was not retried"
        );
        assert!(matches!(err, TeamsError::NetworkError(_)), "{err:?}");
    }

    #[tokio::test]
    async fn a_partial_response_is_not_replayed_for_a_post() {
        assert_post_sent_once_and_get_retried(Fault::PartialHeaders).await;
    }

    #[tokio::test]
    async fn a_timeout_after_the_request_is_not_replayed_for_a_post() {
        assert_post_sent_once_and_get_retried(Fault::Stall).await;
    }

    /// A refused connection proves the request never left, so a POST may retry it, and the
    /// failure carries no warning that it might have been performed.
    #[tokio::test]
    async fn a_refused_connection_is_retryable_for_a_post() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/x", listener.local_addr().unwrap());
        drop(listener);

        let err = retrying_client(2)
            .post_no_content(&url, &serde_json::json!({}))
            .await
            .unwrap_err();
        let TeamsError::NetworkError(source) = &err else {
            panic!("expected a plain NetworkError, got {err:?}");
        };
        assert!(source.is_connect(), "{source}");
        assert!(!err.to_string().contains("check before"), "{err}");
    }

    /// Graph documents Update team and Update channel as 204 No Content; the
    /// update must succeed and return the resource as read back.
    #[tokio::test]
    async fn patch_with_no_content_reads_the_resource_back() {
        use crate::models::team::{Team, TeamUpdateRequest};

        let server = MockServer::start().await;
        let team = serde_json::json!({ "id": "t1", "displayName": "New", "description": "d" });
        Mock::given(method("PATCH"))
            .and(path("/teams/t1"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/teams/t1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(team.clone()))
            .expect(1)
            .mount(&server)
            .await;

        let req = TeamUpdateRequest {
            display_name: Some("New".into()),
            description: None,
        };
        let updated: Team = test_client()
            .patch(&format!("{}/teams/t1", server.uri()), &req)
            .await
            .unwrap();

        assert_eq!(serde_json::to_value(updated).unwrap(), team);
    }

    /// Update chat answers 200 with the resource; that body is used as is.
    #[tokio::test]
    async fn patch_with_a_body_does_not_read_back() {
        let server = MockServer::start().await;
        let chat = serde_json::json!({ "id": "c1", "topic": "New" });
        Mock::given(method("PATCH"))
            .and(path("/chats/c1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(chat.clone()))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(&server)
            .await;

        let updated: serde_json::Value = test_client()
            .patch(
                &format!("{}/chats/c1", server.uri()),
                &serde_json::json!({ "topic": "New" }),
            )
            .await
            .unwrap();

        assert_eq!(updated, chat);
    }

    /// A 200 with an empty body is treated like 204.
    #[tokio::test]
    async fn patch_with_an_empty_200_reads_the_resource_back() {
        let server = MockServer::start().await;
        let channel = serde_json::json!({ "id": "ch1", "displayName": "New" });
        Mock::given(method("PATCH"))
            .and(path("/teams/t1/channels/ch1"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/teams/t1/channels/ch1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(channel.clone()))
            .expect(1)
            .mount(&server)
            .await;

        let updated: serde_json::Value = test_client()
            .patch(
                &format!("{}/teams/t1/channels/ch1", server.uri()),
                &serde_json::json!({ "displayName": "New" }),
            )
            .await
            .unwrap();

        assert_eq!(updated, channel);
    }

    /// The update is applied even when the read-back fails, so the error says
    /// so and exits 1 rather than with the GET's own category (here 5).
    #[tokio::test]
    async fn patch_read_back_failure_says_the_update_succeeded() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/teams/t1"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/teams/t1"))
            .respond_with(ResponseTemplate::new(404).set_body_string("gone"))
            .mount(&server)
            .await;

        let err = test_client()
            .patch::<serde_json::Value, _>(
                &format!("{}/teams/t1", server.uri()),
                &serde_json::json!({ "displayName": "New" }),
            )
            .await
            .unwrap_err();

        assert_eq!(err.exit_code(), 1);
        let message = err.to_string();
        assert!(
            message.starts_with("Update succeeded, but reading it back failed:")
                && message.contains("gone"),
            "{message}"
        );
    }

    /// A raw HTTP/1.1 server answering each request with `patch_reply` for a
    /// PATCH and a JSON resource for anything else, one request per connection.
    async fn raw_patch_server(patch_reply: &'static [u8]) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/teams/t1", listener.local_addr().unwrap());
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut chunk).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    }
                }
                let reply: &[u8] = if buf.starts_with(b"PATCH") {
                    patch_reply
                } else {
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                      Content-Length: 12\r\nConnection: close\r\n\r\n{\"id\": \"t1\"}"
                };
                let _ = stream.write_all(reply).await;
                let _ = stream.shutdown().await;
            }
        });
        url
    }

    /// An empty 200 has no Content-Length when chunked or delimited by the
    /// connection closing; it must still be read back, not parsed.
    #[tokio::test]
    async fn patch_with_an_empty_body_of_unknown_length_reads_the_resource_back() {
        // The server closes every connection, so the chunked reply says so too.
        // Without `Connection: close` the client pools it and can send the
        // read-back GET down the socket just as the server closes it.
        let replies: [&'static [u8]; 2] = [
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n0\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n",
        ];
        for reply in replies {
            let url = raw_patch_server(reply).await;
            let updated: serde_json::Value = test_client()
                .patch(&url, &serde_json::json!({ "displayName": "New" }))
                .await
                .unwrap();
            assert_eq!(updated, serde_json::json!({ "id": "t1" }));
        }
    }
    /// Mounts a two-page `/me/chats` collection; returns the second page's URL.
    async fn mount_two_pages(server: &MockServer) -> String {
        let next = format!("{}/me/chats?$skiptoken=abc", server.uri());
        Mock::given(method("GET"))
            .and(path("/me/chats"))
            .and(query_param_is_missing("$skiptoken"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [{ "id": "chat-1" }],
                "@odata.nextLink": next
            })))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/me/chats"))
            .and(query_param("$skiptoken", "abc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [{ "id": "chat-2" }]
            })))
            .mount(server)
            .await;
        next
    }

    fn one_page() -> PaginationOpts {
        PaginationOpts {
            page_size: 1,
            all_pages: false,
        }
    }

    #[tokio::test]
    async fn get_paged_keeps_next_link_of_a_truncated_page() {
        let server = MockServer::start().await;
        let next = mount_two_pages(&server).await;

        let page = test_client()
            .get_paged::<serde_json::Value>(&format!("{}/me/chats", server.uri()), &[], &one_page())
            .await
            .unwrap();

        assert_eq!(page.items, vec![serde_json::json!({ "id": "chat-1" })]);
        assert_eq!(page.next_link.as_deref(), Some(next.as_str()));
    }

    #[tokio::test]
    async fn get_paged_reports_no_next_link_on_the_last_page() {
        let server = MockServer::start().await;
        let next = mount_two_pages(&server).await;

        let page = test_client()
            .get_paged::<serde_json::Value>(&next, &[], &one_page())
            .await
            .unwrap();

        assert_eq!(page.items, vec![serde_json::json!({ "id": "chat-2" })]);
        assert!(page.next_link.is_none());
    }

    #[tokio::test]
    async fn get_paged_all_pages_follows_links_and_reports_no_next_link() {
        let server = MockServer::start().await;
        mount_two_pages(&server).await;

        let page = test_client()
            .get_paged::<serde_json::Value>(
                &format!("{}/me/chats", server.uri()),
                &[],
                &PaginationOpts {
                    page_size: 1,
                    all_pages: true,
                },
            )
            .await
            .unwrap();

        assert_eq!(
            page.items,
            vec![
                serde_json::json!({ "id": "chat-1" }),
                serde_json::json!({ "id": "chat-2" })
            ]
        );
        assert!(page.next_link.is_none());
    }
    #[tokio::test]
    async fn review_empty_page_with_opaque_next_link_is_not_terminal() {
        let server = MockServer::start().await;
        let next = format!(
            "{}/me/chats?$skiptoken=opaque%2Btoken%2F%3D&$top=7",
            server.uri()
        );
        Mock::given(method("GET"))
            .and(path("/me/chats"))
            .and(query_param_is_missing("$skiptoken"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [], "@odata.nextLink": next
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/me/chats"))
            .and(query_param("$skiptoken", "opaque+token/="))
            .and(query_param("$top", "7"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [{"id": "last"}]
            })))
            .mount(&server)
            .await;
        let url = format!("{}/me/chats", server.uri());
        let single = test_client()
            .get_paged::<serde_json::Value>(&url, &[], &one_page())
            .await
            .unwrap();
        assert!(single.items.is_empty());
        assert_eq!(single.next_link.as_deref(), Some(next.as_str()));
        let all = test_client()
            .get_paged::<serde_json::Value>(
                &url,
                &[],
                &PaginationOpts {
                    page_size: 1,
                    all_pages: true,
                },
            )
            .await
            .unwrap();
        assert_eq!(all.items, vec![serde_json::json!({"id":"last"})]);
        assert!(all.next_link.is_none());
        let requests = server.received_requests().await.unwrap();
        assert_eq!(
            requests.last().unwrap().url.query(),
            Some("$skiptoken=opaque%2Btoken%2F%3D&$top=7")
        );
    }
}
