use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{BalanceResponse, CreateAccountRequest};
use crate::service::LedgerService;

pub async fn create_account(
    State(service): State<Arc<LedgerService>>,
    Json(payload): Json<CreateAccountRequest>,
) -> Result<impl IntoResponse, AppError> {
    if payload.balance.is_some() {
        return Err(AppError::ValidationError(
            "Initial balance cannot be provided by the client. It is always set to 0 by the system.".to_string(),
        ));
    }

    let account = service.create_account(payload.name)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "success": true,
            "data": account
        })),
    ))
}

pub async fn get_account_by_id(
    State(service): State<Arc<LedgerService>>,
    Path(account_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let account = service.get_account(account_id)?;
    Ok(Json(json!({
        "success": true,
        "data": account
    })))
}

pub async fn get_account_balance(
    State(service): State<Arc<LedgerService>>,
    Path(account_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let account = service.get_account(account_id)?;
    Ok(Json(json!({
        "success": true,
        "data": BalanceResponse {
            account_id: account.id,
            name: account.name,
            balance: account.balance,
        }
    })))
}

pub async fn get_all_accounts(
    State(service): State<Arc<LedgerService>>,
) -> Result<impl IntoResponse, AppError> {
    let accounts = service.get_all_accounts()?;
    Ok(Json(json!({
        "success": true,
        "data": accounts
    })))
}

pub async fn verify_integrity(
    State(service): State<Arc<LedgerService>>,
    Path(account_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let integrity = service.verify_integrity(account_id)?;
    Ok(Json(json!({
        "success": true,
        "data": integrity
    })))
}
