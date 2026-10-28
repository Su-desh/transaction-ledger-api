# Rust Transaction Ledger Engine

> **Full Stack Developer Assessment** - High-performance financial transaction ledger backend written in **Rust**, powered by **Axum**, **Tokio (Multi-Threaded Async Runtime)**, **SQLite (ACID / WAL Mode)**, and **Rust Decimal**. Features an interactive dark-mode Web Dashboard served directly from the binary.

---

## Key Highlights & Engineering Guarantees

1. **Zero-Cost Safety & Exact Decimal Precision**:
   - Zero floating-point rounding errors: financial amounts use `rust_decimal::Decimal` (128-bit fixed-point decimal arithmetic).
   - Strict 2-decimal scale validation (`amount.scale() <= 2`).
   - SQLite engine-level check constraints: `CHECK (CAST(balance AS REAL) >= 0.00)` and `CHECK (CAST(amount AS REAL) > 0.00)`.

2. **Strict Invariant Auditing**:
   - Running balance cached on the account for O(1) lookups.
   - Mathematically verified invariant: Balance = Sum of Credits - Sum of Debits.
   - Verified via automated tests and the `GET /api/v1/accounts/:id/integrity` endpoint.

3. **Transaction Immutability**:
   - Transactions are strictly append-only.
   - Any attempt to `PUT`, `PATCH`, or `DELETE` transactions returns `405 Method Not Allowed`.

4. **Atomic Concurrency & Deadlock Prevention**:
   - Atomic database transactions (`BEGIN` ... `COMMIT`).
   - **Deadlock-Free P2P Transfers**: Accounts are sorted lexicographically before acquisition, preventing circular lock deadlocks during concurrent multi-account transfers.
   - Verified by concurrent stress tests (10 simultaneous tasks competing for funds).

5. **Reference ID Idempotency**:
   - Compound index `UNIQUE(account_id, reference_id)`.
   - Re-submitting an existing `reference_id` for the same account is rejected with `409 Conflict` (`DUPLICATE_REFERENCE_ID`) with zero balance change.
   - Different accounts can safely use the same reference ID.

6. **Interactive Web Dashboard**:
   - Served directly from the server at `http://localhost:3000/`.
   - Allows reviewers to test account creation, credit/debit postings, transfers, pagination, and invariant audits in real time.

---

## Quick Start Guide

### Prerequisites
- **Rust & Cargo** (v1.75+ or v1.80+ recommended)

### 1. Run Automated Test Suite
```bash
cargo test
```
All 10 integration and concurrency tests run against an in-memory SQLite database:
```bash
running 10 tests
test test_immutability_put_delete_blocked ... ok
test test_create_account_initial_balance_zero ... ok
test test_insufficient_balance_rejection ... ok
test test_duplicate_reference_id_rejected ... ok
test test_concurrent_debit_protection ... ok
test test_money_transfer_bonus ... ok
test test_ledger_specification_sequence ... ok
test test_pagination_and_sorting ... ok
test test_reject_client_initial_balance ... ok
test test_same_reference_id_different_accounts_allowed ... ok

test result: ok. 10 passed; 0 failed; finished in 0.03s
```

### 2. Start the Server
```bash
# Development mode
cargo run

# Or Optimized Production Release
cargo run --release
```
The server starts on port **`3000`**:
- **Live Web Dashboard:** [https://transaction-ledger-api-1teb.onrender.com](https://transaction-ledger-api-1teb.onrender.com)
- **Live Health Check:** [https://transaction-ledger-api-1teb.onrender.com/api/v1/health](https://transaction-ledger-api-1teb.onrender.com/api/v1/health)
- **Local Dashboard:** [http://localhost:3000](http://localhost:3000)

---

## API Endpoints Summary

| Method | Endpoint | Description | Status Code |
| :--- | :--- | :--- | :--- |
| `POST` | `/api/v1/accounts` | Create new account (Balance system-initialized to 0.00) | `201 Created` |
| `GET` | `/api/v1/accounts` | List all accounts | `200 OK` |
| `GET` | `/api/v1/accounts/:accountId` | Get account by ID | `200 OK` / `404` |
| `GET` | `/api/v1/accounts/:accountId/balance` | Get current balance | `200 OK` / `404` |
| `GET` | `/api/v1/accounts/:accountId/integrity` | Audit balance matches ledger sum | `200 OK` |
| `POST` | `/api/v1/accounts/:accountId/transactions` | Post credit or debit transaction | `201 Created` / `409` / `422` |
| `GET` | `/api/v1/accounts/:accountId/transactions` | Paginated & filtered transaction history | `200 OK` |
| `POST` | `/api/v1/transfers` | Bonus: Atomic P2P money transfer | `201 Created` / `422` |
| `PUT/DEL`| `/api/v1/accounts/:accountId/transactions/*` | Disallowed (Transaction Immutability) | `405 Method Not Allowed` |

For complete schemas and response examples, refer to `docs/API_DOCUMENTATION.md`.

---

## Codebase Architecture

```
transaction-ledger-api/
├── Cargo.toml
├── src/
│   ├── main.rs            # Application entrypoint & HTTP server bootstrap
│   ├── lib.rs             # Library root for integration testing
│   ├── app.rs             # Axum router, CORS layer, static file serving
│   ├── db.rs              # SQLite connection pool, WAL mode & migrations
│   ├── models.rs          # Strongly-typed domain models & DTOs
│   ├── error.rs           # AppError enum + IntoResponse mapping
│   ├── service.rs         # Core business logic & ACID atomic transactions
│   └── handlers/          # HTTP request handlers
│       ├── account.rs
│       ├── transaction.rs
│       └── transfer.rs
├── static/
│   └── index.html         # Single-Page Web Dashboard
├── tests/
│   └── ledger_tests.rs    # End-to-end integration and concurrency tests
├── docs/
│   ├── API_DOCUMENTATION.md
│   └── ASSUMPTIONS.md
└── README.md
```

---

## Author
**Sudesh Kumar**
- Email: [sudesh1428@gmail.com](mailto:sudesh1428@gmail.com)
- GitHub: [https://github.com/Su-desh](https://github.com/Su-desh)
- Location: Bangalore, India
