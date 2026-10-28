use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Credit,
    Debit,
}

impl TransactionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransactionType::Credit => "credit",
            TransactionType::Debit => "debit",
        }
    }

    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "credit" => Some(TransactionType::Credit),
            "debit" => Some(TransactionType::Debit),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: Uuid,
    pub name: String,
    pub balance: Decimal,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: Uuid,
    pub account_id: Uuid,
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
    pub amount: Decimal,
    pub description: String,
    pub reference_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAccountRequest {
    pub name: String,
    pub balance: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTransactionRequest {
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
    pub amount: Decimal,
    pub reference_id: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TransferRequest {
    pub source_account_id: Uuid,
    pub destination_account_id: Uuid,
    pub amount: Decimal,
    pub reference_id: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TransferResponse {
    pub transfer_id: Uuid,
    pub source_account_id: Uuid,
    pub destination_account_id: Uuid,
    pub amount: Decimal,
    pub reference_id: String,
    pub debit_transaction: Transaction,
    pub credit_transaction: Transaction,
    pub source_balance: Decimal,
    pub destination_balance: Decimal,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct TransactionQueryParams {
    pub page: Option<usize>,
    pub limit: Option<usize>,
    #[serde(rename = "type")]
    pub transaction_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PaginationMeta {
    pub current_page: usize,
    pub page_size: usize,
    pub total_records: usize,
    pub total_pages: usize,
}

#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub pagination: PaginationMeta,
}

#[derive(Debug, Serialize)]
pub struct BalanceResponse {
    pub account_id: Uuid,
    pub name: String,
    pub balance: Decimal,
}

#[derive(Debug, Serialize)]
pub struct IntegrityResponse {
    #[serde(rename = "isConsistent")]
    pub is_consistent: bool,
    #[serde(rename = "accountBalance")]
    pub account_balance: Decimal,
    #[serde(rename = "ledgerBalance")]
    pub ledger_balance: Decimal,
}
