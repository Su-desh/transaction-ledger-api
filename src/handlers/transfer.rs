use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use std::sync::Arc;

use crate::error::AppError;
use crate::models::TransferRequest;
use crate::service::LedgerService;

pub async fn transfer_funds(
    State(service): State<Arc<LedgerService>>,
    Json(payload): Json<TransferRequest>,
) -> Result<impl IntoResponse, AppError> {
    let result = service.transfer_funds(payload)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "success": true,
            "data": result
        })),
    ))
}
