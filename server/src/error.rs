use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    DatabaseError(sqlx::Error),
    #[allow(dead_code)]
    NotFound(String),
    Unauthorized,
    BadRequest(String),
    Conflict(String),
    Forbidden(String),
    PayloadTooLarge(String),
    TooManyRequests(String),
    /// The account must set a new password before it can do anything else.
    PasswordChangeRequired,
    /// This can't be undone, and the session is too old to vouch for the person.
    PasswordRequired,
    /// The push would take the account past its storage quota.
    StorageQuotaExceeded,
    /// Too many attempts: the client should wait this long before trying again.
    RateLimited { retry_after_secs: u64 },
    InternalError(String),
    InternalServerError,
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::DatabaseError(e) => write!(f, "Database error: {}", e),
            AppError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AppError::Unauthorized => write!(f, "Unauthorized"),
            AppError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            AppError::Conflict(msg) => write!(f, "Conflict: {}", msg),
            AppError::Forbidden(msg) => write!(f, "Forbidden: {}", msg),
            AppError::PayloadTooLarge(msg) => write!(f, "Payload too large: {}", msg),
            AppError::TooManyRequests(msg) => write!(f, "Too many requests: {}", msg),
            AppError::PasswordChangeRequired => write!(f, "Password change required"),
            AppError::PasswordRequired => write!(f, "Password required"),
            AppError::StorageQuotaExceeded => write!(f, "Storage quota exceeded"),
            AppError::RateLimited { retry_after_secs } => {
                write!(f, "Rate limited for {} seconds", retry_after_secs)
            }
            AppError::InternalError(msg) => write!(f, "Internal error: {}", msg),
            AppError::InternalServerError => write!(f, "Internal server error"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::DatabaseError(err)
    }
}

impl From<crate::services::AdminError> for AppError {
    fn from(err: crate::services::AdminError) -> Self {
        use crate::services::AdminError;

        match err {
            AdminError::UserNotFound(_) => AppError::NotFound("User not found".to_string()),
            AdminError::DeviceNotFound(_) => AppError::NotFound("Device not found".to_string()),
            AdminError::LastAdmin | AdminError::WeakPassword(_) | AdminError::AmbiguousId(_) => {
                AppError::BadRequest(err.to_string())
            }
            AdminError::AlreadyInState(_) => AppError::Conflict(err.to_string()),
            AdminError::Database(e) => AppError::DatabaseError(e),
            AdminError::PasswordHash(e) => AppError::InternalError(e),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if matches!(self, AppError::PasswordChangeRequired) {
            let body = Json(json!({
                "error": "You must change your password before continuing",
                "code": "password_change_required"
            }));
            return (StatusCode::FORBIDDEN, body).into_response();
        }

        if matches!(self, AppError::PasswordRequired) {
            let body = Json(json!({
                "error": "Enter your password to do this",
                "code": "password_required"
            }));
            return (StatusCode::FORBIDDEN, body).into_response();
        }

        if matches!(self, AppError::StorageQuotaExceeded) {
            let body = Json(json!({
                "error": "This account has reached its storage quota",
                "code": "storage_quota_exceeded"
            }));
            return (StatusCode::INSUFFICIENT_STORAGE, body).into_response();
        }

        if let AppError::RateLimited { retry_after_secs } = self {
            let body = Json(json!({
                "error": format!("Too many attempts. Try again in {} seconds.", retry_after_secs),
                "code": "rate_limited",
                "retryAfter": retry_after_secs
            }));
            let headers = [(axum::http::header::RETRY_AFTER, retry_after_secs.to_string())];
            return (StatusCode::TOO_MANY_REQUESTS, headers, body).into_response();
        }

        let (status, error_message) = match self {
            AppError::DatabaseError(e) => {
                tracing::error!("Database error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string())
            }
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            AppError::PayloadTooLarge(msg) => (StatusCode::PAYLOAD_TOO_LARGE, msg),
            AppError::TooManyRequests(msg) => (StatusCode::TOO_MANY_REQUESTS, msg),
            AppError::PasswordChangeRequired
            | AppError::PasswordRequired
            | AppError::StorageQuotaExceeded
            | AppError::RateLimited { .. } => {
                unreachable!("handled above")
            }
            AppError::InternalError(msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
            AppError::InternalServerError => {
                tracing::error!("Internal server error");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
        };

        let body = Json(json!({
            "error": error_message
        }));

        (status, body).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
