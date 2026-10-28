# Architectural Decisions & Domain Assumptions (Rust Implementation)

This document outlines the core engineering assumptions, type-system guarantees, and architectural trade-offs made during the implementation of the **Rust Transaction Ledger Engine**.

---

## 1. Zero-Cost Safety & Exact Decimal Representation

### 1.1 Eliminating IEEE-754 Floating-Point Inaccuracies
- **Decision**: In financial ledgers, using `f64` or `f32` is dangerous due to binary floating-point representation errors (e.g. `0.1 + 0.2 != 0.3`).
- **Implementation**: We model all monetary amounts using `rust_decimal::Decimal` (128-bit fixed-point decimal arithmetic).
- **Validation**:
  - `amount > Decimal::ZERO`
  - `amount.scale() <= 2` (enforcing max 2 decimal places for cents/fractions)

### 1.2 Database-Level Invariants & Engine Constraints
- Stored as strings/exact decimals in SQLite with check constraints:
  - `CHECK (CAST(balance AS REAL) >= 0.00)`: Guarantees at the SQLite storage layer that no balance can drop below zero under any execution path.
  - `CHECK (CAST(amount AS REAL) > 0.00)`: Prevents non-positive transactions from being inserted.
  - `UNIQUE (account_id, reference_id)`: Prevents duplicate reference IDs per account.

---

## 2. Concurrency Model & Deadlock Prevention

### 2.1 Thread-Safe Synchronization
- The SQLite connection is wrapped in `Arc<Mutex<rusqlite::Connection>>`.
- Inside `tokio::spawn` asynchronous handlers, locking the connection guarantees that multi-statement ledger transactions (`BEGIN` ... `COMMIT`) execute sequentially and atomically, eliminating race conditions and lost updates.

### 2.2 Deadlock Prevention in P2P Transfers
- When funds are transferred between Account A and Account B, both accounts must be queried and locked.
- If Account A transfers to Account B while Account B transfers to Account A concurrently, naive lock acquisition causes a deadlock.
- **Solution**: Account IDs are sorted lexicographically (`[id_a, id_b].sort()`) before row evaluation, enforcing a strict global lock hierarchy that guarantees deadlock freedom.

---

## 3. Transaction Immutability & Double-Entry Linking

- **Immutability**:
  - The API explicitly registers `PUT`, `PATCH`, and `DELETE` on all transaction endpoints to return `405 Method Not Allowed`.
  - Database schema has no `UPDATE` or `DELETE` triggers or endpoints for transactions.
- **Transfer Auditing (Linked Records)**:
  - A transfer between Account A and Account B creates **two linked immutable transaction records** within the same atomic database transaction:
    1. A `debit` record on Account A with reference ID `${ref}_DR`
    2. A `credit` record on Account B with reference ID `${ref}_CR`
  - If any step fails (e.g. source has insufficient balance), the entire transaction rolls back completely. Partial transfers are physically impossible.

---

## 4. Invariant Verification Endpoint

- The endpoint `GET /api/v1/accounts/:accountId/integrity` dynamically computes:
  $$\text{Ledger Balance} = \sum_{\text{Credit}} \text{amount} - \sum_{\text{Debit}} \text{amount}$$
- It asserts that $\text{account.balance} == \text{ledger\_balance}$ and returns:
  ```json
  {
    "isConsistent": true,
    "accountBalance": "100",
    "ledgerBalance": "100"
  }
  ```
