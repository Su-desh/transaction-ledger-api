# Transaction Ledger API — Technical Documentation (Rust Edition)

A high-performance financial ledger service written in **Rust**, powered by **Axum**, **Tokio**, and **SQLite**. Built with strict ACID consistency, decimal precision, transaction immutability, and deadlock-free funds transfer.

- **Base URL**: `http://localhost:3000/api/v1`
- **Web Dashboard**: `http://localhost:3000/`
- **Healthcheck**: `http://localhost:3000/api/v1/health`

---

## Error Handling & Response Format

All error responses adhere to a consistent JSON structure:

```json
{
  "success": false,
  "error": {
    "code": "INSUFFICIENT_FUNDS",
    "message": "Insufficient balance for debit. Current balance: 50.00, requested amount: 100.00"
  }
}
```

### HTTP Status Code Mapping
| Status Code | Code String | Trigger Scenario |
| :--- | :--- | :--- |
| `200 OK` | — | Successful query (balance, history, integrity) |
| `201 Created` | — | Account created, transaction posted, transfer executed |
| `400 Bad Request` | `VALIDATION_ERROR` | Empty name, non-positive amount, >2 decimal places, or client trying to set initial balance |
| `404 Not Found` | `NOT_FOUND` | Account ID does not exist |
| `405 Method Not Allowed` | `METHOD_NOT_ALLOWED` | Attempting `PUT`, `PATCH`, or `DELETE` on immutable transactions |
| `409 Conflict` | `DUPLICATE_REFERENCE_ID` | Re-submitting an existing `reference_id` for the same account |
| `422 Unprocessable` | `INSUFFICIENT_FUNDS` | Debit or transfer exceeding available balance |

---

## 1. Account Management

### 1.1 Create Account
Creates a new ledger account. System-generates `id` (UUIDv4) and `created_at` timestamp. Initial balance is always `0.00`.

- **Endpoint**: `POST /api/v1/accounts`
- **Request Body**:
  ```json
  {
    "name": "Sudesh Kumar"
  }
  ```
- **Response (201 Created)**:
  ```json
  {
    "success": true,
    "data": {
      "id": "e4a5d8b2-3f11-4f12-9c3a-8b8d91c7a2b1",
      "name": "Sudesh Kumar",
      "balance": 0.0,
      "created_at": "2026-10-28T11:00:00Z"
    }
  }
  ```

### 1.2 Get Account Balance
Retrieves current account balance in $O(1)$ time.

- **Endpoint**: `GET /api/v1/accounts/:accountId/balance`
- **Response (200 OK)**:
  ```json
  {
    "success": true,
    "data": {
      "account_id": "e4a5d8b2-3f11-4f12-9c3a-8b8d91c7a2b1",
      "name": "Sudesh Kumar",
      "balance": 150.0
    }
  }
  ```

### 1.3 Verify Invariant Integrity
Audits whether `account.balance` strictly equals $\sum \text{Credits} - \sum \text{Debits}$ from raw historical transactions.

- **Endpoint**: `GET /api/v1/accounts/:accountId/integrity`
- **Response (200 OK)**:
  ```json
  {
    "success": true,
    "data": {
      "isConsistent": true,
      "accountBalance": 150.0,
      "ledgerBalance": 150.0
    }
  }
  ```

---

## 2. Transactions

### 2.1 Post a Transaction (Credit / Debit)
Atomically applies a credit or debit to the account and mutates the cached balance.

- **Endpoint**: `POST /api/v1/accounts/:accountId/transactions`
- **Request Body**:
  ```json
  {
    "type": "credit",
    "amount": 100.50,
    "reference_id": "PAY-REF-001",
    "description": "Salary deposit"
  }
  ```
- **Response (201 Created)**:
  ```json
  {
    "success": true,
    "data": {
      "id": "7c9e6679-7425-40de-944b-e07fc1f90ae7",
      "account_id": "e4a5d8b2-3f11-4f12-9c3a-8b8d91c7a2b1",
      "type": "credit",
      "amount": 100.50,
      "description": "Salary deposit",
      "reference_id": "PAY-REF-001",
      "created_at": "2026-10-28T11:05:00Z"
    },
    "balance": 100.50
  }
  ```

### 2.2 Get Transaction History (Pagination, Sorting, Filtering)
- **Endpoint**: `GET /api/v1/accounts/:accountId/transactions`
- **Query Parameters**:
  - `page`: Page index (default `1`)
  - `limit`: Records per page (default `10`, max `100`)
  - `type`: Optional filter (`"credit"` or `"debit"`)
- **Response (200 OK)**:
  ```json
  {
    "success": true,
    "data": [
      {
        "id": "8d3e911a-1b42-4f01-9231-18e42b10a9c8",
        "account_id": "e4a5d8b2-3f11-4f12-9c3a-8b8d91c7a2b1",
        "type": "debit",
        "amount": 20.0,
        "description": "Coffee",
        "reference_id": "PAY-REF-002",
        "created_at": "2026-10-28T11:10:00Z"
      }
    ],
    "pagination": {
      "current_page": 1,
      "page_size": 10,
      "total_records": 1,
      "total_pages": 1
    }
  }
  ```

### 2.3 Transaction Immutability
Transactions can never be modified or deleted.
- `PUT /api/v1/accounts/:accountId/transactions/:id` $\rightarrow$ `405 Method Not Allowed`
- `DELETE /api/v1/accounts/:accountId/transactions/:id` $\rightarrow$ `405 Method Not Allowed`

---

## 3. Funds Transfer (Bonus)

Atomically transfers money from one account to another using double-entry linked transaction pairs.

- **Endpoint**: `POST /api/v1/transfers`
- **Request Body**:
  ```json
  {
    "source_account_id": "e4a5d8b2-3f11-4f12-9c3a-8b8d91c7a2b1",
    "destination_account_id": "9a1f2b3c-4d5e-6f7a-8b9c-0d1e2f3a4b5c",
    "amount": 30.00,
    "reference_id": "TRF-P2P-001",
    "description": "Settling dinner bill"
  }
  ```
- **Response (201 Created)**:
  ```json
  {
    "success": true,
    "data": {
      "transfer_id": "bf6928e4-18e3-4d69-a868-b8ce71ddf41e",
      "source_account_id": "e4a5d8b2-3f11-4f12-9c3a-8b8d91c7a2b1",
      "destination_account_id": "9a1f2b3c-4d5e-6f7a-8b9c-0d1e2f3a4b5c",
      "amount": 30.0,
      "reference_id": "TRF-P2P-001",
      "debit_transaction": {
        "id": "...",
        "account_id": "e4a5d8b2-3f11-4f12-9c3a-8b8d91c7a2b1",
        "type": "debit",
        "amount": 30.0,
        "description": "Settling dinner bill (Transfer to ...)",
        "reference_id": "TRF-P2P-001_DR",
        "created_at": "2026-10-28T11:15:00Z"
      },
      "credit_transaction": {
        "id": "...",
        "account_id": "9a1f2b3c-4d5e-6f7a-8b9c-0d1e2f3a4b5c",
        "type": "credit",
        "amount": 30.0,
        "description": "Settling dinner bill (Transfer from ...)",
        "reference_id": "TRF-P2P-001_CR",
        "created_at": "2026-10-28T11:15:00Z"
      },
      "source_balance": 70.0,
      "destination_balance": 80.0,
      "created_at": "2026-10-28T11:15:00Z"
    }
  }
  ```
