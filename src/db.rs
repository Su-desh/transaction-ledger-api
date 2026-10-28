use rusqlite::{Connection, Result};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub type DbPool = Arc<Mutex<Connection>>;

pub fn init_db(db_path: Option<&str>) -> Result<DbPool> {
    let conn = match db_path {
        Some(path) => {
            if path != ":memory:" {
                if let Some(parent) = Path::new(path).parent() {
                    fs::create_dir_all(parent).ok();
                }
            }
            Connection::open(path)?
        }
        None => {
            let data_dir = Path::new("data");
            fs::create_dir_all(data_dir).ok();
            Connection::open("data/ledger.sqlite")?
        }
    };

    conn.pragma_update(None, "foreign_keys", "ON")?;

    // Only enable WAL for file-based DBs
    if db_path != Some(":memory:") {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
    }

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS accounts (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            balance TEXT NOT NULL,
            created_at TEXT NOT NULL,
            CHECK (CAST(balance AS REAL) >= 0.00)
        );

        CREATE TABLE IF NOT EXISTS transactions (
            id TEXT PRIMARY KEY,
            account_id TEXT NOT NULL,
            type TEXT NOT NULL CHECK (type IN ('credit', 'debit')),
            amount TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            reference_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE RESTRICT,
            CHECK (CAST(amount AS REAL) > 0.00)
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_transactions_account_reference
        ON transactions (account_id, reference_id);

        CREATE INDEX IF NOT EXISTS idx_transactions_account_history
        ON transactions (account_id, created_at DESC, id DESC);
        "#,
    )?;

    Ok(Arc::new(Mutex::new(conn)))
}
