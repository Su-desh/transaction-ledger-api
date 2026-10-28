use chrono::Utc;
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

use crate::db::DbPool;
use crate::error::AppError;
use crate::models::{
    Account, CreateTransactionRequest, IntegrityResponse, PaginatedResponse, PaginationMeta,
    Transaction, TransactionQueryParams, TransactionType, TransferRequest, TransferResponse,
};

pub struct LedgerService {
    pool: DbPool,
}

impl LedgerService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_account(&self, name: String) -> Result<Account, AppError> {
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            return Err(AppError::ValidationError("Account name must not be empty".to_string()));
        }
        if trimmed_name.len() > 255 {
            return Err(AppError::ValidationError("Account name must not exceed 255 characters".to_string()));
        }

        let account = Account {
            id: Uuid::new_v4(),
            name: trimmed_name.to_string(),
            balance: Decimal::ZERO,
            created_at: Utc::now(),
        };

        let conn = self.pool.lock().unwrap();
        conn.execute(
            "INSERT INTO accounts (id, name, balance, created_at) VALUES (?1, ?2, ?3, ?4)",
            (
                account.id.to_string(),
                &account.name,
                account.balance.to_string(),
                account.created_at.to_rfc3339(),
            ),
        )
        .map_err(|e| AppError::Internal(format!("Failed to insert account: {}", e)))?;

        Ok(account)
    }

    pub fn get_account(&self, id: Uuid) -> Result<Account, AppError> {
        let conn = self.pool.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, name, balance, created_at FROM accounts WHERE id = ?1")
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let row = stmt
            .query_row([id.to_string()], |row| {
                let id_str: String = row.get(0)?;
                let name: String = row.get(1)?;
                let balance_str: String = row.get(2)?;
                let created_str: String = row.get(3)?;
                Ok((id_str, name, balance_str, created_str))
            })
            .optional()
            .map_err(|e| AppError::Internal(e.to_string()))?;

        match row {
            Some((id_str, name, balance_str, created_str)) => {
                let id = Uuid::parse_str(&id_str).map_err(|_| AppError::Internal("Corrupted account UUID".to_string()))?;
                let balance = Decimal::from_str(&balance_str).map_err(|_| AppError::Internal("Corrupted balance decimal".to_string()))?;
                let created_at = chrono::DateTime::parse_from_rfc3339(&created_str)
                    .map_err(|_| AppError::Internal("Corrupted timestamp".to_string()))?
                    .with_timezone(&Utc);

                Ok(Account { id, name, balance, created_at })
            }
            None => Err(AppError::NotFound(format!("Account with ID '{}' not found", id))),
        }
    }

    pub fn get_all_accounts(&self) -> Result<Vec<Account>, AppError> {
        let conn = self.pool.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, name, balance, created_at FROM accounts ORDER BY created_at DESC")
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                let id_str: String = row.get(0)?;
                let name: String = row.get(1)?;
                let balance_str: String = row.get(2)?;
                let created_str: String = row.get(3)?;
                Ok((id_str, name, balance_str, created_str))
            })
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let mut accounts = Vec::new();
        for r in rows {
            let (id_str, name, balance_str, created_str) = r.map_err(|e| AppError::Internal(e.to_string()))?;
            let id = Uuid::parse_str(&id_str).map_err(|_| AppError::Internal("Corrupted account UUID".to_string()))?;
            let balance = Decimal::from_str(&balance_str).map_err(|_| AppError::Internal("Corrupted balance decimal".to_string()))?;
            let created_at = chrono::DateTime::parse_from_rfc3339(&created_str)
                .map_err(|_| AppError::Internal("Corrupted timestamp".to_string()))?
                .with_timezone(&Utc);

            accounts.push(Account { id, name, balance, created_at });
        }

        Ok(accounts)
    }

    pub fn add_transaction(
        &self,
        account_id: Uuid,
        req: CreateTransactionRequest,
    ) -> Result<(Transaction, Decimal), AppError> {
        // Validation
        if req.amount <= Decimal::ZERO {
            return Err(AppError::ValidationError("Amount must be greater than 0".to_string()));
        }
        if req.amount.scale() > 2 {
            return Err(AppError::ValidationError("Amount cannot have more than 2 decimal places".to_string()));
        }
        let trimmed_ref = req.reference_id.trim();
        if trimmed_ref.is_empty() {
            return Err(AppError::ValidationError("reference_id is required".to_string()));
        }

        let mut conn = self.pool.lock().unwrap();

        // 1. Check account exists
        let account_exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ?1)",
                [account_id.to_string()],
                |row| row.get(0),
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;

        if !account_exists {
            return Err(AppError::NotFound(format!("Account with ID '{}' not found", account_id)));
        }

        // 2. Check duplicate reference_id for this account
        let is_duplicate: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM transactions WHERE account_id = ?1 AND reference_id = ?2)",
                (account_id.to_string(), trimmed_ref),
                |row| row.get(0),
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;

        if is_duplicate {
            return Err(AppError::DuplicateReference(format!(
                "Transaction with reference_id '{}' already exists for account '{}'",
                trimmed_ref, account_id
            )));
        }

        // 3. Execute atomic transaction
        let tx = conn
            .transaction()
            .map_err(|e| AppError::Internal(format!("Failed to start transaction: {}", e)))?;

        // Re-read current balance inside transaction
        let current_balance_str: String = tx
            .query_row(
                "SELECT balance FROM accounts WHERE id = ?1",
                [account_id.to_string()],
                |row| row.get(0),
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let current_balance = Decimal::from_str(&current_balance_str)
            .map_err(|_| AppError::Internal("Corrupted balance decimal".to_string()))?;

        let new_balance = match req.transaction_type {
            TransactionType::Credit => current_balance + req.amount,
            TransactionType::Debit => {
                if current_balance < req.amount {
                    return Err(AppError::InsufficientFunds {
                        balance: current_balance,
                        requested: req.amount,
                    });
                }
                current_balance - req.amount
            }
        };

        let new_tx = Transaction {
            id: Uuid::new_v4(),
            account_id,
            transaction_type: req.transaction_type,
            amount: req.amount,
            description: req.description.unwrap_or_default().trim().to_string(),
            reference_id: trimmed_ref.to_string(),
            created_at: Utc::now(),
        };

        // Persist transaction record
        tx.execute(
            r#"
            INSERT INTO transactions (id, account_id, type, amount, description, reference_id, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            (
                new_tx.id.to_string(),
                new_tx.account_id.to_string(),
                new_tx.transaction_type.as_str(),
                new_tx.amount.to_string(),
                &new_tx.description,
                &new_tx.reference_id,
                new_tx.created_at.to_rfc3339(),
            ),
        )
        .map_err(|e| AppError::Internal(format!("Failed to record transaction: {}", e)))?;

        // Update account balance
        tx.execute(
            "UPDATE accounts SET balance = ?1 WHERE id = ?2",
            (new_balance.to_string(), account_id.to_string()),
        )
        .map_err(|e| AppError::Internal(format!("Failed to update account balance: {}", e)))?;

        tx.commit()
            .map_err(|e| AppError::Internal(format!("Commit failed: {}", e)))?;

        Ok((new_tx, new_balance))
    }

    pub fn get_transactions(
        &self,
        account_id: Uuid,
        query: TransactionQueryParams,
    ) -> Result<PaginatedResponse<Transaction>, AppError> {
        let conn = self.pool.lock().unwrap();

        // Check account exists
        let account_exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ?1)",
                [account_id.to_string()],
                |row| row.get(0),
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;

        if !account_exists {
            return Err(AppError::NotFound(format!("Account with ID '{}' not found", account_id)));
        }

        let page = query.page.unwrap_or(1);
        if page < 1 {
            return Err(AppError::ValidationError("Page must be greater than or equal to 1".to_string()));
        }
        let limit = query.limit.unwrap_or(10);
        if limit < 1 || limit > 100 {
            return Err(AppError::ValidationError("Limit must be between 1 and 100".to_string()));
        }

        let tx_type = match query.transaction_type.as_deref() {
            Some(s) => match TransactionType::from_str_opt(s) {
                Some(t) => Some(t),
                None => return Err(AppError::ValidationError("Invalid transaction type filter".to_string())),
            },
            None => None,
        };

        let offset = (page - 1) * limit;

        // Count total
        let total: usize = match tx_type {
            Some(t) => conn.query_row(
                "SELECT COUNT(*) FROM transactions WHERE account_id = ?1 AND type = ?2",
                (account_id.to_string(), t.as_str()),
                |row| row.get(0),
            ),
            None => conn.query_row(
                "SELECT COUNT(*) FROM transactions WHERE account_id = ?1",
                [account_id.to_string()],
                |row| row.get(0),
            ),
        }
        .map_err(|e| AppError::Internal(e.to_string()))?;

        // Query transactions
        let mut transactions = Vec::new();
        let query_str = match tx_type {
            Some(_) => "SELECT id, account_id, type, amount, description, reference_id, created_at FROM transactions WHERE account_id = ?1 AND type = ?2 ORDER BY created_at DESC, id DESC LIMIT ?3 OFFSET ?4",
            None => "SELECT id, account_id, type, amount, description, reference_id, created_at FROM transactions WHERE account_id = ?1 ORDER BY created_at DESC, id DESC LIMIT ?2 OFFSET ?3",
        };

        let mut stmt = conn.prepare(query_str).map_err(|e| AppError::Internal(e.to_string()))?;

        let rows = match tx_type {
            Some(t) => stmt.query_map(
                (account_id.to_string(), t.as_str(), limit as i64, offset as i64),
                Self::map_transaction_row,
            ),
            None => stmt.query_map(
                (account_id.to_string(), limit as i64, offset as i64),
                Self::map_transaction_row,
            ),
        }
        .map_err(|e| AppError::Internal(e.to_string()))?;

        for r in rows {
            transactions.push(r.map_err(|e| AppError::Internal(e.to_string()))?);
        }

        let total_pages = if total == 0 { 1 } else { (total + limit - 1) / limit };

        Ok(PaginatedResponse {
            data: transactions,
            pagination: PaginationMeta {
                current_page: page,
                page_size: limit,
                total_records: total,
                total_pages,
            },
        })
    }

    fn map_transaction_row(row: &rusqlite::Row) -> rusqlite::Result<Transaction> {
        let id_str: String = row.get(0)?;
        let account_id_str: String = row.get(1)?;
        let type_str: String = row.get(2)?;
        let amount_str: String = row.get(3)?;
        let description: String = row.get(4)?;
        let reference_id: String = row.get(5)?;
        let created_str: String = row.get(6)?;

        let id = Uuid::parse_str(&id_str).map_err(|_| rusqlite::Error::InvalidQuery)?;
        let account_id = Uuid::parse_str(&account_id_str).map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction_type = TransactionType::from_str_opt(&type_str).ok_or(rusqlite::Error::InvalidQuery)?;
        let amount = Decimal::from_str(&amount_str).map_err(|_| rusqlite::Error::InvalidQuery)?;
        let created_at = chrono::DateTime::parse_from_rfc3339(&created_str)
            .map_err(|_| rusqlite::Error::InvalidQuery)?
            .with_timezone(&Utc);

        Ok(Transaction {
            id,
            account_id,
            transaction_type,
            amount,
            description,
            reference_id,
            created_at,
        })
    }

    pub fn transfer_funds(&self, req: TransferRequest) -> Result<TransferResponse, AppError> {
        let TransferRequest {
            source_account_id,
            destination_account_id,
            amount,
            reference_id,
            description,
        } = req;

        if source_account_id == destination_account_id {
            return Err(AppError::ValidationError("Source and destination accounts must be different".to_string()));
        }
        if amount <= Decimal::ZERO {
            return Err(AppError::ValidationError("Transfer amount must be greater than 0".to_string()));
        }
        if amount.scale() > 2 {
            return Err(AppError::ValidationError("Amount cannot have more than 2 decimal places".to_string()));
        }
        let trimmed_ref = reference_id.trim();
        if trimmed_ref.is_empty() {
            return Err(AppError::ValidationError("reference_id is required".to_string()));
        }

        let debit_ref = format!("{}_DR", trimmed_ref);
        let credit_ref = format!("{}_CR", trimmed_ref);

        let mut conn = self.pool.lock().unwrap();

        // 1. Verify existence of both accounts
        let src_exists: bool = conn
            .query_row("SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ?1)", [source_account_id.to_string()], |r| r.get(0))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        if !src_exists {
            return Err(AppError::NotFound(format!("Source account with ID '{}' not found", source_account_id)));
        }

        let dst_exists: bool = conn
            .query_row("SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ?1)", [destination_account_id.to_string()], |r| r.get(0))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        if !dst_exists {
            return Err(AppError::NotFound(format!("Destination account with ID '{}' not found", destination_account_id)));
        }

        // 2. Check duplicate references
        let src_dup: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM transactions WHERE account_id = ?1 AND reference_id = ?2)",
                (source_account_id.to_string(), &debit_ref),
                |r| r.get(0),
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;
        if src_dup {
            return Err(AppError::DuplicateReference(format!(
                "Transfer transaction with reference_id '{}' already processed for source account",
                trimmed_ref
            )));
        }

        // 3. Atomic execution
        let tx = conn
            .transaction()
            .map_err(|e| AppError::Internal(format!("Failed to start transaction: {}", e)))?;

        // Deadlock-free ordered lock acquisition
        let mut ordered_ids = [source_account_id, destination_account_id];
        ordered_ids.sort();
        for id in &ordered_ids {
            let _: String = tx
                .query_row("SELECT balance FROM accounts WHERE id = ?1", [id.to_string()], |r| r.get(0))
                .map_err(|e| AppError::Internal(e.to_string()))?;
        }

        let src_bal_str: String = tx
            .query_row("SELECT balance FROM accounts WHERE id = ?1", [source_account_id.to_string()], |r| r.get(0))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let src_balance = Decimal::from_str(&src_bal_str).map_err(|_| AppError::Internal("Corrupted balance".to_string()))?;

        if src_balance < amount {
            return Err(AppError::InsufficientFunds {
                balance: src_balance,
                requested: amount,
            });
        }

        let dst_bal_str: String = tx
            .query_row("SELECT balance FROM accounts WHERE id = ?1", [destination_account_id.to_string()], |r| r.get(0))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let dst_balance = Decimal::from_str(&dst_bal_str).map_err(|_| AppError::Internal("Corrupted balance".to_string()))?;

        let new_src_balance = src_balance - amount;
        let new_dst_balance = dst_balance + amount;
        let transfer_id = Uuid::new_v4();
        let timestamp = Utc::now();
        let desc = description.unwrap_or_else(|| "Funds Transfer".to_string());

        // 1. Debit Transaction on Source
        let debit_tx = Transaction {
            id: Uuid::new_v4(),
            account_id: source_account_id,
            transaction_type: TransactionType::Debit,
            amount,
            description: format!("{} (Transfer to {})", desc, destination_account_id),
            reference_id: debit_ref,
            created_at: timestamp,
        };
        tx.execute(
            "INSERT INTO transactions (id, account_id, type, amount, description, reference_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                debit_tx.id.to_string(),
                debit_tx.account_id.to_string(),
                debit_tx.transaction_type.as_str(),
                debit_tx.amount.to_string(),
                &debit_tx.description,
                &debit_tx.reference_id,
                debit_tx.created_at.to_rfc3339(),
            ),
        ).map_err(|e| AppError::Internal(e.to_string()))?;

        // 2. Credit Transaction on Destination
        let credit_tx = Transaction {
            id: Uuid::new_v4(),
            account_id: destination_account_id,
            transaction_type: TransactionType::Credit,
            amount,
            description: format!("{} (Transfer from {})", desc, source_account_id),
            reference_id: credit_ref,
            created_at: timestamp,
        };
        tx.execute(
            "INSERT INTO transactions (id, account_id, type, amount, description, reference_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                credit_tx.id.to_string(),
                credit_tx.account_id.to_string(),
                credit_tx.transaction_type.as_str(),
                credit_tx.amount.to_string(),
                &credit_tx.description,
                &credit_tx.reference_id,
                credit_tx.created_at.to_rfc3339(),
            ),
        ).map_err(|e| AppError::Internal(e.to_string()))?;

        // 3. Update balances
        tx.execute("UPDATE accounts SET balance = ?1 WHERE id = ?2", (new_src_balance.to_string(), source_account_id.to_string()))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        tx.execute("UPDATE accounts SET balance = ?1 WHERE id = ?2", (new_dst_balance.to_string(), destination_account_id.to_string()))
            .map_err(|e| AppError::Internal(e.to_string()))?;

        tx.commit().map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(TransferResponse {
            transfer_id,
            source_account_id,
            destination_account_id,
            amount,
            reference_id: trimmed_ref.to_string(),
            debit_transaction: debit_tx,
            credit_transaction: credit_tx,
            source_balance: new_src_balance,
            destination_balance: new_dst_balance,
            created_at: timestamp,
        })
    }

    pub fn verify_integrity(&self, account_id: Uuid) -> Result<IntegrityResponse, AppError> {
        let account = self.get_account(account_id)?;
        let conn = self.pool.lock().unwrap();

        let mut stmt = conn
            .prepare("SELECT type, amount FROM transactions WHERE account_id = ?1")
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let rows = stmt
            .query_map([account_id.to_string()], |row| {
                let type_str: String = row.get(0)?;
                let amount_str: String = row.get(1)?;
                Ok((type_str, amount_str))
            })
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let mut ledger_sum = Decimal::ZERO;
        for r in rows {
            let (t_str, a_str) = r.map_err(|e| AppError::Internal(e.to_string()))?;
            let amount = Decimal::from_str(&a_str).map_err(|_| AppError::Internal("Bad amount decimal".to_string()))?;
            match t_str.as_str() {
                "credit" => ledger_sum += amount,
                "debit" => ledger_sum -= amount,
                _ => {}
            }
        }

        Ok(IntegrityResponse {
            is_consistent: account.balance == ledger_sum,
            account_balance: account.balance,
            ledger_balance: ledger_sum,
        })
    }
}
