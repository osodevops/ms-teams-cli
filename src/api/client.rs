use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;

use crate::auth::token::TokenInfo;
use crate::config::NetworkConfig;
use crate::error::{Result, TeamsError};
use crate::models::common::PageResponse;

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
        let http = Client::builder()
            .timeout(Duration::from_secs(network.timeout))
            .build()
            .map_err(TeamsError::NetworkError)?;

        Ok(Self {
            http,
            token,
            network: network.clone(),
        })
    }

    /// GET request with retry logic.
    pub async fn get<T: DeserializeOwned>(&self, url: &str, query: &[(&str, &str)]) -> Result<T> {
        self.request_with_retry(|this| {
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
    pub async fn get_paged<T: DeserializeOwned>(
        &self,
        url: &str,
        query: &[(&str, &str)],
        pagination: &PaginationOpts,
    ) -> Result<Vec<T>> {
        let top_str = pagination.page_size.to_string();
        let mut full_query: Vec<(&str, &str)> = query.to_vec();
        full_query.push(("$top", &top_str));

        if pagination.all_pages {
            self.get_all_pages(url, &full_query).await
        } else {
            let resp: PageResponse<T> = self.get(url, &full_query).await?;
            Ok(resp.value)
        }
    }

    /// POST request with JSON body.
    pub async fn post<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<T> {
        self.request_with_retry(|this| {
            this.http
                .post(url)
                .header("Authorization", this.token.bearer_header())
                .json(body)
        })
        .await
    }

    /// PATCH request with JSON body.
    pub async fn patch<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<T> {
        self.request_with_retry(|this| {
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

    /// POST request returning the Location header (for 202 Accepted async operations).
    pub async fn post_for_location<B: serde::Serialize>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<Option<String>> {
        let max_retries = self.network.max_retries;
        let backoff_base = self.network.retry_backoff_base;

        for attempt in 0..=max_retries {
            let req = self
                .http
                .post(url)
                .header("Authorization", self.token.bearer_header())
                .json(body);

            let resp = match req.send().await {
                Ok(r) => r,
                Err(e) => {
                    if attempt < max_retries {
                        let delay = backoff_base.pow(attempt);
                        tracing::warn!(
                            "Request failed (attempt {}/{}), retrying in {delay}s: {e}",
                            attempt + 1,
                            max_retries + 1
                        );
                        tokio::time::sleep(Duration::from_secs(delay)).await;
                        continue;
                    }
                    return Err(TeamsError::NetworkError(e));
                }
            };

            let status = resp.status();

            if let Some(err) = self.check_retryable_error(&resp, status, attempt, max_retries, backoff_base).await? {
                if err {
                    continue;
                }
            }

            if !status.is_success() && status != StatusCode::ACCEPTED {
                let body = resp.text().await.unwrap_or_default();
                return Err(TeamsError::ApiError {
                    status: status.as_u16(),
                    message: body,
                });
            }

            let location = resp
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            return Ok(location);
        }

        Err(TeamsError::ApiError {
            status: 0,
            message: "Max retries exceeded".to_string(),
        })
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
        self.request_with_retry(move |this| {
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
        let max_retries = self.network.max_retries;
        let backoff_base = self.network.retry_backoff_base;

        for attempt in 0..=max_retries {
            let req = self
                .http
                .get(url)
                .header("Authorization", self.token.bearer_header());

            let resp = match req.send().await {
                Ok(r) => r,
                Err(e) => {
                    if attempt < max_retries {
                        let delay = backoff_base.pow(attempt);
                        tracing::warn!(
                            "Request failed (attempt {}/{}), retrying in {delay}s: {e}",
                            attempt + 1,
                            max_retries + 1
                        );
                        tokio::time::sleep(Duration::from_secs(delay)).await;
                        continue;
                    }
                    return Err(TeamsError::NetworkError(e));
                }
            };

            let status = resp.status();

            if let Some(err) = self
                .check_retryable_error(&resp, status, attempt, max_retries, backoff_base)
                .await?
            {
                if err {
                    continue;
                }
            }

            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(TeamsError::ApiError {
                    status: status.as_u16(),
                    message: body,
                });
            }

            let bytes = resp.bytes().await.map_err(TeamsError::NetworkError)?;
            return Ok(bytes.to_vec());
        }

        Err(TeamsError::ApiError {
            status: 0,
            message: "Max retries exceeded".to_string(),
        })
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

    /// Check for retryable error conditions. Returns:
    /// - Ok(Some(true)) → should continue (retry)
    /// - Ok(Some(false)) → impossible (reserved)
    /// - Ok(None) → no error, proceed with response
    /// - Err → non-retryable error
    async fn check_retryable_error(
        &self,
        resp: &reqwest::Response,
        status: StatusCode,
        attempt: u32,
        max_retries: u32,
        backoff_base: u64,
    ) -> Result<Option<bool>> {
        if status == StatusCode::TOO_MANY_REQUESTS {
            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(60);

            if attempt < max_retries {
                tracing::warn!("Rate limited, waiting {retry_after}s");
                tokio::time::sleep(Duration::from_secs(retry_after)).await;
                return Ok(Some(true));
            }
            return Err(TeamsError::RateLimited { retry_after });
        }

        if status == StatusCode::UNAUTHORIZED {
            return Err(TeamsError::AuthError(
                "Authentication failed (401)".to_string(),
            ));
        }
        if status == StatusCode::FORBIDDEN {
            return Err(TeamsError::PermissionDenied(
                "Forbidden (403)".to_string(),
            ));
        }
        if status == StatusCode::NOT_FOUND {
            return Err(TeamsError::NotFound("Not found (404)".to_string()));
        }

        if status.is_server_error() {
            if attempt < max_retries {
                let delay = backoff_base.pow(attempt);
                tracing::warn!(
                    "Server error {status} (attempt {}/{}), retrying in {delay}s",
                    attempt + 1,
                    max_retries + 1
                );
                tokio::time::sleep(Duration::from_secs(delay)).await;
                return Ok(Some(true));
            }
            return Err(TeamsError::ServerError {
                status: status.as_u16(),
                message: format!("Server error {status}"),
            });
        }

        Ok(None)
    }

    /// Internal: execute a request with retry + rate-limit handling.
    async fn request_with_retry<T, F>(&self, build_request: F) -> Result<T>
    where
        T: DeserializeOwned,
        F: Fn(&Self) -> reqwest::RequestBuilder,
    {
        let max_retries = self.network.max_retries;
        let backoff_base = self.network.retry_backoff_base;

        for attempt in 0..=max_retries {
            let req = build_request(self);

            let resp = match req.send().await {
                Ok(r) => r,
                Err(e) => {
                    if attempt < max_retries {
                        let delay = backoff_base.pow(attempt);
                        tracing::warn!(
                            "Request failed (attempt {}/{}), retrying in {delay}s: {e}",
                            attempt + 1,
                            max_retries + 1
                        );
                        tokio::time::sleep(Duration::from_secs(delay)).await;
                        continue;
                    }
                    return Err(TeamsError::NetworkError(e));
                }
            };

            let status = resp.status();

            // Rate limiting (429)
            if status == StatusCode::TOO_MANY_REQUESTS {
                let retry_after = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(60);

                if attempt < max_retries {
                    tracing::warn!("Rate limited, waiting {retry_after}s");
                    tokio::time::sleep(Duration::from_secs(retry_after)).await;
                    continue;
                }
                return Err(TeamsError::RateLimited { retry_after });
            }

            // Auth errors
            if status == StatusCode::UNAUTHORIZED {
                let body = resp.text().await.unwrap_or_default();
                return Err(TeamsError::AuthError(format!(
                    "Authentication failed (401): {body}"
                )));
            }
            if status == StatusCode::FORBIDDEN {
                let body = resp.text().await.unwrap_or_default();
                return Err(TeamsError::PermissionDenied(body));
            }

            // Not found
            if status == StatusCode::NOT_FOUND {
                let body = resp.text().await.unwrap_or_default();
                return Err(TeamsError::NotFound(body));
            }

            // Server errors (retry)
            if status.is_server_error() {
                let body = resp.text().await.unwrap_or_default();
                if attempt < max_retries {
                    let delay = backoff_base.pow(attempt);
                    tracing::warn!(
                        "Server error {status} (attempt {}/{}), retrying in {delay}s",
                        attempt + 1,
                        max_retries + 1
                    );
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                    continue;
                }
                return Err(TeamsError::ServerError {
                    status: status.as_u16(),
                    message: body,
                });
            }

            // Other client errors
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(TeamsError::ApiError {
                    status: status.as_u16(),
                    message: body,
                });
            }

            // Success
            let body = resp.json::<T>().await.map_err(|e| {
                if e.is_decode() {
                    TeamsError::ApiError {
                        status: 200,
                        message: format!("Failed to parse API response: {e}"),
                    }
                } else {
                    TeamsError::NetworkError(e)
                }
            })?;
            return Ok(body);
        }

        Err(TeamsError::ApiError {
            status: 0,
            message: "Max retries exceeded".to_string(),
        })
    }

    /// Internal: execute a request that returns no content (204).
    async fn request_with_retry_no_content<F>(&self, build_request: F) -> Result<()>
    where
        F: Fn(&Self) -> reqwest::RequestBuilder,
    {
        let max_retries = self.network.max_retries;
        let backoff_base = self.network.retry_backoff_base;

        for attempt in 0..=max_retries {
            let req = build_request(self);

            let resp = match req.send().await {
                Ok(r) => r,
                Err(e) => {
                    if attempt < max_retries {
                        let delay = backoff_base.pow(attempt);
                        tracing::warn!(
                            "Request failed (attempt {}/{}), retrying in {delay}s: {e}",
                            attempt + 1,
                            max_retries + 1
                        );
                        tokio::time::sleep(Duration::from_secs(delay)).await;
                        continue;
                    }
                    return Err(TeamsError::NetworkError(e));
                }
            };

            let status = resp.status();

            // Rate limiting (429)
            if status == StatusCode::TOO_MANY_REQUESTS {
                let retry_after = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(60);

                if attempt < max_retries {
                    tracing::warn!("Rate limited, waiting {retry_after}s");
                    tokio::time::sleep(Duration::from_secs(retry_after)).await;
                    continue;
                }
                return Err(TeamsError::RateLimited { retry_after });
            }

            if status == StatusCode::UNAUTHORIZED {
                let body = resp.text().await.unwrap_or_default();
                return Err(TeamsError::AuthError(format!(
                    "Authentication failed (401): {body}"
                )));
            }
            if status == StatusCode::FORBIDDEN {
                let body = resp.text().await.unwrap_or_default();
                return Err(TeamsError::PermissionDenied(body));
            }
            if status == StatusCode::NOT_FOUND {
                let body = resp.text().await.unwrap_or_default();
                return Err(TeamsError::NotFound(body));
            }

            if status.is_server_error() {
                let body = resp.text().await.unwrap_or_default();
                if attempt < max_retries {
                    let delay = backoff_base.pow(attempt);
                    tracing::warn!(
                        "Server error {status} (attempt {}/{}), retrying in {delay}s",
                        attempt + 1,
                        max_retries + 1
                    );
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                    continue;
                }
                return Err(TeamsError::ServerError {
                    status: status.as_u16(),
                    message: body,
                });
            }

            if !status.is_success() && status != StatusCode::NO_CONTENT {
                let body = resp.text().await.unwrap_or_default();
                return Err(TeamsError::ApiError {
                    status: status.as_u16(),
                    message: body,
                });
            }

            return Ok(());
        }

        Err(TeamsError::ApiError {
            status: 0,
            message: "Max retries exceeded".to_string(),
        })
    }
}
