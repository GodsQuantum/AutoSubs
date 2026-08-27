use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde::Serialize;

#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct ErrorEnvelope<'a> { error: ErrorBody<'a> }
#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: &'a Option<serde_json::Value>,
}

impl AppError {
    pub fn bad_request(message: impl Into<String>) -> Self { Self { status: StatusCode::BAD_REQUEST, code: "bad_request", message: message.into(), details: None } }
    pub fn not_found(message: impl Into<String>) -> Self { Self { status: StatusCode::NOT_FOUND, code: "not_found", message: message.into(), details: None } }
    pub fn conflict(message: impl Into<String>) -> Self { Self { status: StatusCode::CONFLICT, code: "conflict", message: message.into(), details: None } }
    pub fn forbidden(message: impl Into<String>) -> Self { Self { status: StatusCode::FORBIDDEN, code: "forbidden", message: message.into(), details: None } }
    pub fn internal(error: impl std::fmt::Display) -> Self {
        tracing::error!(error = %error, "internal request error");
        Self { status: StatusCode::INTERNAL_SERVER_ERROR, code: "internal_error", message: "Internal server error".into(), details: None }
    }
    pub fn with_details(mut self, details: serde_json::Value) -> Self { self.details = Some(details); self }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = ErrorEnvelope { error: ErrorBody { code: self.code, message: &self.message, details: &self.details } };
        (self.status, Json(body)).into_response()
    }
}

impl From<anyhow::Error> for AppError { fn from(value: anyhow::Error) -> Self { Self::internal(value) } }
impl From<std::io::Error> for AppError { fn from(value: std::io::Error) -> Self { Self::internal(value) } }
