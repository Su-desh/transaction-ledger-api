use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use rust_decimal::Decimal;
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    ValidationError(String),
    InsufficientFunds {
        balance: Decimal,
        requested: Decimal,
    },
    DuplicateReference(String),
    MethodNotAllowed,
    Internal(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::NotFound(msg) => write!(f, "{}", msg),
            AppError::ValidationError(msg) => write!(f, "{}", msg),
            AppError::InsufficientFunds { balance, requested } => write!(
                f,
                "Insufficient balance for debit. Current balance: {}, requested amount: {}",
                balance, requested
            ),
            AppError::DuplicateReference(msg) => write!(f, "{}", msg),
            AppError::MethodNotAllowed => write!(
                f,
                "Method not allowed. Ledger transactions are immutable and cannot be modified or deleted."
            ),
            AppError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            AppError::ValidationError(_) => (StatusCode::BAD_REQUEST, "VALIDATION_ERROR"),
            AppError::InsufficientFunds { .. } => {
                (StatusCode::UNPROCESSABLE_ENTITY, "INSUFFICIENT_FUNDS")
            }
            AppError::DuplicateReference(_) => (StatusCode::CONFLICT, "DUPLICATE_REFERENCE_ID"),
            AppError::MethodNotAllowed => (StatusCode::METHOD_NOT_ALLOWED, "METHOD_NOT_ALLOWED"),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_SERVER_ERROR"),
        };

        let body = Json(json!({
            "success": false,
            "error": {
                "code": code,
                "message": self.to_string(),
            }
        }));

        (status, body).into_response()
    }
}
