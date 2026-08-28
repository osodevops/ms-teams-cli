use std::process;

#[derive(Debug, thiserror::Error)]
pub enum TeamsError {
    #[error("Authentication failed: {0}")]
    AuthError(String),

    #[error("Token expired")]
    TokenExpired,

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Rate limited, retry after {retry_after}s")]
    RateLimited { retry_after: u64 },

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("API error ({status}): {message}")]
    ApiError { status: u16, message: String },

    #[error("Server error ({status}): {message}")]
    ServerError { status: u16, message: String },

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Config error: {0}")]
    ConfigError(String),

    #[error("Keyring error: {0}")]
    KeyringError(String),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

impl TeamsError {
    /// Exit codes per PRD §8.3
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Other(_) => 1,
            Self::InvalidInput(_) => 2,
            Self::AuthError(_) | Self::TokenExpired => 3,
            Self::PermissionDenied(_) => 4,
            Self::NotFound(_) => 5,
            Self::RateLimited { .. } => 6,
            Self::NetworkError(_) => 7,
            Self::ServerError { .. } => 8,
            Self::ConfigError(_) | Self::KeyringError(_) => 10,
            Self::ApiError { status, .. } => match *status {
                401 => 3,
                403 => 4,
                404 => 5,
                429 => 6,
                s if s >= 500 => 8,
                _ => 1,
            },
        }
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            Self::AuthError(_) => "AUTH_FAILED",
            Self::TokenExpired => "AUTH_TOKEN_EXPIRED",
            Self::PermissionDenied(_) => "PERMISSION_DENIED",
            Self::NotFound(_) => "NOT_FOUND",
            Self::RateLimited { .. } => "RATE_LIMITED",
            Self::NetworkError(_) => "NETWORK_ERROR",
            Self::ApiError { .. } => "API_ERROR",
            Self::ServerError { .. } => "SERVER_ERROR",
            Self::InvalidInput(_) => "INVALID_INPUT",
            Self::ConfigError(_) => "CONFIG_ERROR",
            Self::KeyringError(_) => "KEYRING_ERROR",
            Self::Other(_) => "UNKNOWN",
        }
    }

    #[allow(dead_code)]
    pub fn exit(self) -> ! {
        let code = self.exit_code();
        tracing::error!("{}", self);
        process::exit(code);
    }
}

/// Render an error together with everything that caused it.
///
/// `reqwest::Error` displays a failed deserialization as the bare phrase "error decoding response
/// body", which names neither the offending value nor where in the body it sat; serde's own
/// message, which names both, is one level down the source chain and `Display` never reaches it.
/// A response that Graph considers successful but this crate cannot parse is otherwise
/// indistinguishable from any other parse failure, and diagnosing one means capturing the body by
/// hand.
pub fn describe_with_causes(err: &dyn std::error::Error) -> String {
    let mut message = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        let text = cause.to_string();
        if !text.is_empty() && !message.ends_with(&text) {
            message.push_str(": ");
            message.push_str(&text);
        }
        source = cause.source();
    }
    message
}

pub type Result<T> = std::result::Result<T, TeamsError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_with_causes_flattens_the_whole_chain() {
        #[derive(Debug)]
        struct Layer(&'static str, Option<Box<Layer>>);
        impl std::fmt::Display for Layer {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.0)
            }
        }
        impl std::error::Error for Layer {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                self.1
                    .as_deref()
                    .map(|l| l as &(dyn std::error::Error + 'static))
            }
        }

        let err = Layer(
            "error decoding response body",
            Some(Box::new(Layer(
                "invalid type: map, expected a string at line 1 column 66",
                None,
            ))),
        );
        assert_eq!(
            describe_with_causes(&err),
            "error decoding response body: invalid type: map, expected a string at line 1 column 66"
        );
    }

    #[test]
    fn describe_with_causes_does_not_repeat_a_cause_the_outer_error_already_quotes() {
        #[derive(Debug)]
        struct Wrapper(std::io::Error);
        impl std::fmt::Display for Wrapper {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "reading config: {}", self.0)
            }
        }
        impl std::error::Error for Wrapper {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&self.0)
            }
        }

        let err = Wrapper(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no such file",
        ));
        assert_eq!(describe_with_causes(&err), "reading config: no such file");
    }

    #[test]
    fn exit_codes_match_prd() {
        assert_eq!(TeamsError::InvalidInput("x".into()).exit_code(), 2);
        assert_eq!(TeamsError::AuthError("x".into()).exit_code(), 3);
        assert_eq!(TeamsError::TokenExpired.exit_code(), 3);
        assert_eq!(TeamsError::PermissionDenied("x".into()).exit_code(), 4);
        assert_eq!(TeamsError::NotFound("x".into()).exit_code(), 5);
        assert_eq!(TeamsError::RateLimited { retry_after: 60 }.exit_code(), 6);
        assert_eq!(
            TeamsError::ServerError {
                status: 500,
                message: "x".into()
            }
            .exit_code(),
            8
        );
        assert_eq!(TeamsError::ConfigError("x".into()).exit_code(), 10);
        assert_eq!(TeamsError::KeyringError("x".into()).exit_code(), 10);
    }

    #[test]
    fn error_codes_are_correct() {
        assert_eq!(
            TeamsError::AuthError("x".into()).error_code(),
            "AUTH_FAILED"
        );
        assert_eq!(TeamsError::TokenExpired.error_code(), "AUTH_TOKEN_EXPIRED");
        assert_eq!(TeamsError::NotFound("x".into()).error_code(), "NOT_FOUND");
        assert_eq!(
            TeamsError::RateLimited { retry_after: 10 }.error_code(),
            "RATE_LIMITED"
        );
    }

    #[test]
    fn api_error_exit_codes_map_status() {
        assert_eq!(
            TeamsError::ApiError {
                status: 401,
                message: "x".into()
            }
            .exit_code(),
            3
        );
        assert_eq!(
            TeamsError::ApiError {
                status: 403,
                message: "x".into()
            }
            .exit_code(),
            4
        );
        assert_eq!(
            TeamsError::ApiError {
                status: 404,
                message: "x".into()
            }
            .exit_code(),
            5
        );
        assert_eq!(
            TeamsError::ApiError {
                status: 429,
                message: "x".into()
            }
            .exit_code(),
            6
        );
        assert_eq!(
            TeamsError::ApiError {
                status: 503,
                message: "x".into()
            }
            .exit_code(),
            8
        );
    }
}
