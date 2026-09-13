use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    NotFound(String),
    Internal(String),
    #[allow(dead_code)]
    Timeout,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::BadRequest(msg) => write!(f, "{}", msg),
            AppError::NotFound(msg) => write!(f, "{}", msg),
            AppError::Internal(msg) => write!(f, "{}", msg),
            AppError::Timeout => write!(f, "Operation timed out"),
        }
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            AppError::BadRequest(ref m) => (StatusCode::BAD_REQUEST, m.clone()),
            AppError::NotFound(ref m) => (StatusCode::NOT_FOUND, m.clone()),
            AppError::Internal(ref m) => (StatusCode::INTERNAL_SERVER_ERROR, m.clone()),
            AppError::Timeout => (StatusCode::GATEWAY_TIMEOUT, "Operation timed out".to_string()),
        };

        // FastAPI standard format is {"detail": "..."}
        let body = Json(json!({ "detail": msg }));
        (status, body).into_response()
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Internal(format!("JSON error: {}", err))
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::Internal(format!("Network error: {}", err))
    }
}
