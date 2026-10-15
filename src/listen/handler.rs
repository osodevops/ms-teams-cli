use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use http_body_util::{BodyExt, Full, LengthLimitError, Limited};
use hyper::body::{Body, Bytes};
use hyper::{Method, Request, Response, StatusCode};
use sha2::{Digest, Sha256};

use super::queue::{Push, Queue};
use crate::models::subscription::ChangeNotificationCollection;

/// Largest notification body accepted.
pub const MAX_BODY: usize = 1024 * 1024;
/// Time allowed to receive a request body.
pub const BODY_TIMEOUT: Duration = Duration::from_secs(10);

type HandlerResult = std::result::Result<Response<Full<Bytes>>, Infallible>;

pub struct State {
    /// SHA-256 of the expected clientState, when one is set.
    client_state: Option<[u8; 32]>,
    queue: Queue,
}

impl State {
    pub fn new(client_state: Option<&str>, queue: Queue) -> Self {
        Self {
            client_state: client_state.map(digest),
            queue,
        }
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    fn accepts(&self, client_state: Option<&str>) -> bool {
        match (&self.client_state, client_state) {
            (None, _) => true,
            (Some(want), Some(got)) => constant_time_eq(want, &digest(got)),
            (Some(_), None) => false,
        }
    }
}

fn digest(s: &str) -> [u8; 32] {
    Sha256::digest(s.as_bytes()).into()
}

/// Compare digests without an early exit, so timing does not reveal a prefix.
fn constant_time_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let diff = a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y));
    std::hint::black_box(diff) == 0
}

pub async fn handle_request<B>(req: Request<B>, state: Arc<State>) -> HandlerResult
where
    B: Body<Data = Bytes>,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => Ok(plain_response(StatusCode::OK, "OK")),

        (&Method::POST, "/") | (&Method::POST, "/webhook") => {
            // Validation is answered before the body is read or the output
            // queue is consulted, so it works whatever stdout is doing.
            if let Some(query) = req.uri().query() {
                if let Some((_, token)) = url::form_urlencoded::parse(query.as_bytes())
                    .find(|(k, _)| k == "validationToken")
                {
                    return Ok(plain_response(StatusCode::OK, &token));
                }
            }

            if req.body().size_hint().lower() > MAX_BODY as u64 {
                return Ok(too_large());
            }
            let body = Limited::new(req.into_body(), MAX_BODY);
            let body_bytes = match tokio::time::timeout(BODY_TIMEOUT, body.collect()).await {
                Ok(Ok(collected)) => collected.to_bytes(),
                Ok(Err(e)) if e.downcast_ref::<LengthLimitError>().is_some() => {
                    return Ok(too_large());
                }
                Ok(Err(e)) => {
                    return Ok(plain_response(
                        StatusCode::BAD_REQUEST,
                        &format!("Failed to read body: {e}"),
                    ));
                }
                Err(_) => {
                    return Ok(plain_response(
                        StatusCode::REQUEST_TIMEOUT,
                        "Timed out reading body",
                    ))
                }
            };

            let collection =
                match serde_json::from_slice::<ChangeNotificationCollection>(&body_bytes) {
                    Ok(collection) => collection,
                    Err(e) => {
                        tracing::warn!("Failed to parse notification: {e}");
                        return Ok(plain_response(
                            StatusCode::BAD_REQUEST,
                            &format!("Invalid notification body: {e}"),
                        ));
                    }
                };

            let mut lines = Vec::with_capacity(collection.value.len());
            for notification in &collection.value {
                if !state.accepts(notification.client_state.as_deref()) {
                    tracing::warn!(
                        subscription_id = ?notification.subscription_id,
                        "Dropped notification: clientState missing or wrong"
                    );
                    continue;
                }
                lines.push(notification.to_output().to_string());
            }

            // Rejected items still get 202: an error only makes Graph resend
            // them. A batch is queued whole or not at all, so a retry after
            // 503 does not duplicate lines.
            if lines.is_empty() {
                return Ok(plain_response(StatusCode::ACCEPTED, ""));
            }
            match state.queue.try_push(lines) {
                Push::Queued => Ok(plain_response(StatusCode::ACCEPTED, "")),
                Push::Full => {
                    tracing::warn!("Output queue full; answering 503");
                    Ok(plain_response(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "Output queue full",
                    ))
                }
                Push::Closed => Ok(plain_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Shutting down",
                )),
            }
        }

        _ => Ok(plain_response(StatusCode::NOT_FOUND, "Not Found")),
    }
}

fn too_large() -> Response<Full<Bytes>> {
    plain_response(StatusCode::PAYLOAD_TOO_LARGE, "Body exceeds 1 MiB")
}

fn plain_response(status: StatusCode, body: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("Content-Type", "text/plain")
        .body(Full::new(Bytes::from(body.to_string())))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::body::Frame;
    use std::sync::mpsc as std_mpsc;

    fn state(client_state: Option<&str>) -> (Arc<State>, std_mpsc::Receiver<String>) {
        let (tx, rx) = std_mpsc::channel();
        let queue = Queue::new(1024 * 1024, move |text| {
            let _ = tx.send(text.to_string());
            Ok(())
        });
        (Arc::new(State::new(client_state, queue)), rx)
    }

    fn post(uri: &str, body: &str) -> Request<Full<Bytes>> {
        Request::post(uri)
            .body(Full::new(Bytes::from(body.to_string())))
            .unwrap()
    }

    async fn body_text(resp: Response<Full<Bytes>>) -> String {
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn health_check_returns_ok() {
        let (state, _) = state(None);
        let req = Request::get("/").body(Full::new(Bytes::new())).unwrap();
        let resp = handle_request(req, state).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn validation_token_is_echoed_decoded_as_text() {
        let (state, _) = state(Some("s"));
        let resp = handle_request(post("/webhook?validationToken=a%20b%2Bc", ""), state)
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.headers()["Content-Type"], "text/plain");
        assert_eq!(body_text(resp).await, "a b+c");
    }

    #[tokio::test]
    async fn not_found_response() {
        let (state, _) = state(None);
        let resp = handle_request(post("/other", ""), state).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn notifications_with_wrong_client_state_are_dropped() {
        let (state, rx) = state(Some("right"));
        let body = r#"{"value":[
            {"subscriptionId":"forged","clientState":"wrong"},
            {"subscriptionId":"missing"},
            {"subscriptionId":"ok","clientState":"right","changeType":"created"}
        ]}"#;
        let resp = handle_request(post("/", body), state).await.unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
        let out = rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let lines: Vec<serde_json::Value> = out
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["subscriptionId"], "ok");
        assert_eq!(lines[0]["kind"], "change");
        assert!(lines[0].get("clientState").is_none());
    }

    #[tokio::test]
    async fn invalid_json_is_bad_request() {
        let (state, _) = state(None);
        let resp = handle_request(post("/", "not json"), state).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    /// A body of `n` 64 KiB frames with no size hint, like a chunked request.
    struct Chunks(usize);

    impl Body for Chunks {
        type Data = Bytes;
        type Error = Infallible;

        fn poll_frame(
            mut self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Result<Frame<Bytes>, Infallible>>> {
            if self.0 == 0 {
                return std::task::Poll::Ready(None);
            }
            self.0 -= 1;
            let frame = Frame::data(Bytes::from(vec![b' '; 64 * 1024]));
            std::task::Poll::Ready(Some(Ok(frame)))
        }
    }

    #[tokio::test]
    async fn oversized_body_without_length_is_rejected() {
        let (state, _) = state(None);
        let req = Request::post("/").body(Chunks(20)).unwrap();
        let resp = handle_request(req, state).await.unwrap();
        assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn shutting_down_answers_503_but_still_validates() {
        let (state, rx) = state(None);
        state.queue().close();
        let resp = handle_request(post("/", r#"{"value":[{"id":"1"}]}"#), state.clone())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(rx.try_recv().is_err());
        let resp = handle_request(post("/?validationToken=t", ""), state)
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn full_queue_answers_503() {
        // Smaller than one line, so the push is refused.
        let queue = Queue::new(4, |_| Ok(()));
        let state = Arc::new(State::new(None, queue));
        let resp = handle_request(post("/", r#"{"value":[{"id":"1"}]}"#), state)
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn client_state_comparison() {
        let (state, _) = state(Some("secret"));
        assert!(state.accepts(Some("secret")));
        assert!(!state.accepts(Some("secreT")));
        assert!(!state.accepts(Some("")));
        assert!(!state.accepts(None));
        let (open, _) = self::state(None);
        assert!(open.accepts(None));
    }
}
