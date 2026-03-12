use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::{Method, Request, Response, StatusCode};

use crate::models::subscription::ChangeNotificationCollection;

type HandlerResult = std::result::Result<Response<Full<Bytes>>, hyper::Error>;

pub async fn handle_request(req: Request<Incoming>) -> HandlerResult {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => Ok(ok_response("OK")),

        (&Method::POST, "/") | (&Method::POST, "/webhook") => {
            // Check for validation token in query string
            if let Some(query) = req.uri().query() {
                let params: Vec<(String, String)> =
                    url::form_urlencoded::parse(query.as_bytes())
                        .into_owned()
                        .collect();

                if let Some((_, token)) = params.iter().find(|(k, _)| k == "validationToken") {
                    return Ok(plain_response(StatusCode::OK, token));
                }
            }

            // Parse notification body
            let body_bytes = req.into_body().collect().await?.to_bytes();
            match serde_json::from_slice::<ChangeNotificationCollection>(&body_bytes) {
                Ok(collection) => {
                    for notification in &collection.value {
                        let json = serde_json::to_string(notification).unwrap_or_default();
                        println!("{json}");
                    }
                    Ok(ok_response(""))
                }
                Err(e) => {
                    tracing::warn!("Failed to parse notification: {e}");
                    Ok(plain_response(
                        StatusCode::BAD_REQUEST,
                        &format!("Invalid notification body: {e}"),
                    ))
                }
            }
        }

        _ => Ok(plain_response(StatusCode::NOT_FOUND, "Not Found")),
    }
}

fn ok_response(body: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/plain")
        .body(Full::new(Bytes::from(body.to_string())))
        .unwrap()
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

    #[test]
    fn health_check_returns_ok() {
        let resp = ok_response("OK");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn validation_token_echo() {
        let resp = plain_response(StatusCode::OK, "my-token-123");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn not_found_response() {
        let resp = plain_response(StatusCode::NOT_FOUND, "Not Found");
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
