use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{CreateTransactionRequest, TransactionQueryParams};
use crate::service::LedgerService;

pub async fn add_transaction(
    State(service): State<Arc<LedgerService>>,
    Path(account_id): Path<Uuid>,
    Json(payload): Json<CreateTransactionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (tx, new_balance) = service.add_transaction(account_id, payload)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "success": true,
            "data": tx,
            "balance": new_balance
        })),
    ))
}

pub async fn get_transaction_history(
    State(service): State<Arc<LedgerService>>,
    Path(account_id): Path<Uuid>,
    Query(query): Query<TransactionQueryParams>,
) -> Result<impl IntoResponse, AppError> {
    let result = service.get_transactions(account_id, query)?;
    Ok(Json(json!({
        "success": true,
        "data": result.data,
        "pagination": result.pagination
    })))
}

pub async fn method_not_allowed() -> Result<axum::response::Response, AppError> {
    Err(AppError::MethodNotAllowed)
}
