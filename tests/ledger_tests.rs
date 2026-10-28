use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use rust_decimal_macros::dec;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

use transaction_ledger_api::{app::create_app, db::init_db, service::LedgerService};

fn setup_test_app() -> axum::Router {
    let pool = init_db(Some(":memory:")).expect("Failed to initialize in-memory DB");
    let service = Arc::new(LedgerService::new(pool));
    create_app(service)
}

async fn parse_response(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = if bytes.is_empty() {
        json!(null)
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({ "raw": String::from_utf8_lossy(&bytes) }))
    };
    (status, body)
}

#[tokio::test]
async fn test_create_account_initial_balance_zero() {
    let app = setup_test_app();

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/accounts")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "name": "Sudesh Kumar" }).to_string()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    let (status, body) = parse_response(resp).await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["name"], "Sudesh Kumar");
    assert_eq!(body["data"]["balance"].as_f64().unwrap(), 0.0);
    assert!(body["data"]["id"].is_string());
}

#[tokio::test]
async fn test_reject_client_initial_balance() {
    let app = setup_test_app();

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/accounts")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "name": "Hacker", "balance": 1000 }).to_string(),
        ))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    let (status, body) = parse_response(resp).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
}

#[tokio::test]
async fn test_ledger_specification_sequence() {
    let app = setup_test_app();

    // 1. Create account
    let create_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/accounts")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "name": "Sequence Test" }).to_string()))
        .unwrap();
    let (_, create_body) = parse_response(app.clone().oneshot(create_req).await.unwrap()).await;
    let account_id = create_body["data"]["id"].as_str().unwrap();

    // Sequence: Credit 100, Credit 50, Debit 30, Debit 20 => Final balance 100
    let txs = [
        ("credit", 100, "SEQ-1"),
        ("credit", 50, "SEQ-2"),
        ("debit", 30, "SEQ-3"),
        ("debit", 20, "SEQ-4"),
    ];

    for (tx_type, amount, ref_id) in txs {
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/accounts/{}/transactions", account_id))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({
                    "type": tx_type,
                    "amount": amount,
                    "reference_id": ref_id,
                    "description": format!("Test {}", ref_id)
                })
                .to_string(),
            ))
            .unwrap();

        let (status, body) = parse_response(app.clone().oneshot(req).await.unwrap()).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["success"], true);
    }

    // Verify balance endpoint returns exactly 100
    let bal_req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/accounts/{}/balance", account_id))
        .body(Body::empty())
        .unwrap();
    let (bal_status, bal_body) = parse_response(app.clone().oneshot(bal_req).await.unwrap()).await;
    assert_eq!(bal_status, StatusCode::OK);
    assert_eq!(bal_body["data"]["balance"].as_f64().unwrap(), 100.0);

    // Verify integrity endpoint confirms invariant (Balance == Ledger Sum)
    let int_req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/accounts/{}/integrity", account_id))
        .body(Body::empty())
        .unwrap();
    let (int_status, int_body) = parse_response(app.clone().oneshot(int_req).await.unwrap()).await;
    assert_eq!(int_status, StatusCode::OK);
    assert_eq!(int_body["data"]["isConsistent"], true);
    assert_eq!(int_body["data"]["accountBalance"].as_f64().unwrap(), 100.0);
    assert_eq!(int_body["data"]["ledgerBalance"].as_f64().unwrap(), 100.0);
}

#[tokio::test]
async fn test_insufficient_balance_rejection() {
    let app = setup_test_app();

    // Create account
    let create_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/accounts")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "name": "Overdraft Test" }).to_string()))
        .unwrap();
    let (_, create_body) = parse_response(app.clone().oneshot(create_req).await.unwrap()).await;
    let account_id = create_body["data"]["id"].as_str().unwrap();

    // Credit 100
    let credit_req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/accounts/{}/transactions", account_id))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "type": "credit", "amount": 100, "reference_id": "CR-100" }).to_string(),
        ))
        .unwrap();
    let (cr_status, _) = parse_response(app.clone().oneshot(credit_req).await.unwrap()).await;
    assert_eq!(cr_status, StatusCode::CREATED);

    // Attempt Debit 150 (must fail with 422)
    let debit_req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/accounts/{}/transactions", account_id))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "type": "debit", "amount": 150, "reference_id": "DR-FAIL" }).to_string(),
        ))
        .unwrap();
    let (dr_status, dr_body) = parse_response(app.clone().oneshot(debit_req).await.unwrap()).await;
    assert_eq!(dr_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(dr_body["error"]["code"], "INSUFFICIENT_FUNDS");

    // Balance must remain unchanged at 100
    let bal_req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/accounts/{}/balance", account_id))
        .body(Body::empty())
        .unwrap();
    let (_, bal_body) = parse_response(app.clone().oneshot(bal_req).await.unwrap()).await;
    assert_eq!(bal_body["data"]["balance"].as_f64().unwrap(), 100.0);
}

#[tokio::test]
async fn test_duplicate_reference_id_rejected() {
    let app = setup_test_app();

    // Create account
    let create_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/accounts")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "name": "Dup Ref Test" }).to_string()))
        .unwrap();
    let (_, create_body) = parse_response(app.clone().oneshot(create_req).await.unwrap()).await;
    let account_id = create_body["data"]["id"].as_str().unwrap();

    // First transaction
    let req1 = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/accounts/{}/transactions", account_id))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "type": "credit", "amount": 50, "reference_id": "UNIQUE-REF" }).to_string(),
        ))
        .unwrap();
    let (s1, _) = parse_response(app.clone().oneshot(req1).await.unwrap()).await;
    assert_eq!(s1, StatusCode::CREATED);

    // Duplicate transaction with same reference_id
    let req2 = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/accounts/{}/transactions", account_id))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "type": "credit", "amount": 50, "reference_id": "UNIQUE-REF" }).to_string(),
        ))
        .unwrap();
    let (s2, body2) = parse_response(app.clone().oneshot(req2).await.unwrap()).await;
    assert_eq!(s2, StatusCode::CONFLICT);
    assert_eq!(body2["error"]["code"], "DUPLICATE_REFERENCE_ID");

    // Balance must still be 50, not 100
    let bal_req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/accounts/{}/balance", account_id))
        .body(Body::empty())
        .unwrap();
    let (_, bal_body) = parse_response(app.clone().oneshot(bal_req).await.unwrap()).await;
    assert_eq!(bal_body["data"]["balance"].as_f64().unwrap(), 50.0);
}

#[tokio::test]
async fn test_same_reference_id_different_accounts_allowed() {
    let app = setup_test_app();

    // Account A
    let req_a = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/accounts")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "name": "User A" }).to_string()))
        .unwrap();
    let (_, body_a) = parse_response(app.clone().oneshot(req_a).await.unwrap()).await;
    let id_a = body_a["data"]["id"].as_str().unwrap();

    // Account B
    let req_b = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/accounts")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "name": "User B" }).to_string()))
        .unwrap();
    let (_, body_b) = parse_response(app.clone().oneshot(req_b).await.unwrap()).await;
    let id_b = body_b["data"]["id"].as_str().unwrap();

    // User A uses SHARED-REF
    let tx_a = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/accounts/{}/transactions", id_a))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "type": "credit", "amount": 50, "reference_id": "SHARED-REF" }).to_string(),
        ))
        .unwrap();
    let (sa, _) = parse_response(app.clone().oneshot(tx_a).await.unwrap()).await;
    assert_eq!(sa, StatusCode::CREATED);

    // User B uses SAME SHARED-REF (must succeed)
    let tx_b = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/accounts/{}/transactions", id_b))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "type": "credit", "amount": 75, "reference_id": "SHARED-REF" }).to_string(),
        ))
        .unwrap();
    let (sb, _) = parse_response(app.clone().oneshot(tx_b).await.unwrap()).await;
    assert_eq!(sb, StatusCode::CREATED);
}

#[tokio::test]
async fn test_immutability_put_delete_blocked() {
    let app = setup_test_app();

    let put_req = Request::builder()
        .method(Method::PUT)
        .uri("/api/v1/accounts/11111111-1111-1111-1111-111111111111/transactions/22222222-2222-2222-2222-222222222222")
        .body(Body::empty())
        .unwrap();
    let (put_status, put_body) = parse_response(app.clone().oneshot(put_req).await.unwrap()).await;
    assert_eq!(put_status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(put_body["error"]["code"], "METHOD_NOT_ALLOWED");

    let del_req = Request::builder()
        .method(Method::DELETE)
        .uri("/api/v1/accounts/11111111-1111-1111-1111-111111111111/transactions/22222222-2222-2222-2222-222222222222")
        .body(Body::empty())
        .unwrap();
    let (del_status, del_body) = parse_response(app.clone().oneshot(del_req).await.unwrap()).await;
    assert_eq!(del_status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(del_body["error"]["code"], "METHOD_NOT_ALLOWED");
}

#[tokio::test]
async fn test_pagination_and_sorting() {
    let app = setup_test_app();

    let create_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/accounts")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "name": "Pagination User" }).to_string()))
        .unwrap();
    let (_, create_body) = parse_response(app.clone().oneshot(create_req).await.unwrap()).await;
    let account_id = create_body["data"]["id"].as_str().unwrap();

    // Seed 5 transactions
    for i in 1..=5 {
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/accounts/{}/transactions", account_id))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({
                    "type": "credit",
                    "amount": i * 10,
                    "reference_id": format!("PAGE-REF-{}", i),
                    "description": format!("Tx {}", i)
                })
                .to_string(),
            ))
            .unwrap();
        parse_response(app.clone().oneshot(req).await.unwrap()).await;
    }

    // Page 1, limit 2 (must return 2 records, latest first)
    let p1_req = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/api/v1/accounts/{}/transactions?page=1&limit=2",
            account_id
        ))
        .body(Body::empty())
        .unwrap();
    let (p1_status, p1_body) = parse_response(app.clone().oneshot(p1_req).await.unwrap()).await;
    assert_eq!(p1_status, StatusCode::OK);
    assert_eq!(p1_body["pagination"]["current_page"], 1);
    assert_eq!(p1_body["pagination"]["page_size"], 2);
    assert_eq!(p1_body["pagination"]["total_records"], 5);
    assert_eq!(p1_body["pagination"]["total_pages"], 3);
    assert_eq!(p1_body["data"].as_array().unwrap().len(), 2);

    // Latest first: first item should be PAGE-REF-5
    assert_eq!(p1_body["data"][0]["reference_id"], "PAGE-REF-5");
}

#[tokio::test]
async fn test_money_transfer_bonus() {
    let app = setup_test_app();

    // Create Account A with 100
    let req_a = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/accounts")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "name": "Account A" }).to_string()))
        .unwrap();
    let (_, body_a) = parse_response(app.clone().oneshot(req_a).await.unwrap()).await;
    let id_a = body_a["data"]["id"].as_str().unwrap();

    let cr_a = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/accounts/{}/transactions", id_a))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "type": "credit", "amount": 100, "reference_id": "INIT-A" }).to_string(),
        ))
        .unwrap();
    parse_response(app.clone().oneshot(cr_a).await.unwrap()).await;

    // Create Account B with 50
    let req_b = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/accounts")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "name": "Account B" }).to_string()))
        .unwrap();
    let (_, body_b) = parse_response(app.clone().oneshot(req_b).await.unwrap()).await;
    let id_b = body_b["data"]["id"].as_str().unwrap();

    let cr_b = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/accounts/{}/transactions", id_b))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "type": "credit", "amount": 50, "reference_id": "INIT-B" }).to_string(),
        ))
        .unwrap();
    parse_response(app.clone().oneshot(cr_b).await.unwrap()).await;

    // Transfer 30 from A to B => Final A: 70, B: 80
    let trf_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/transfers")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "source_account_id": id_a,
                "destination_account_id": id_b,
                "amount": 30,
                "reference_id": "TRF-30"
            })
            .to_string(),
        ))
        .unwrap();
    let (trf_status, trf_body) = parse_response(app.clone().oneshot(trf_req).await.unwrap()).await;
    assert_eq!(trf_status, StatusCode::CREATED);
    assert_eq!(trf_body["data"]["source_balance"].as_f64().unwrap(), 70.0);
    assert_eq!(
        trf_body["data"]["destination_balance"].as_f64().unwrap(),
        80.0
    );

    // Verify Account A balance
    let bal_a = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/accounts/{}/balance", id_a))
        .body(Body::empty())
        .unwrap();
    let (_, ba) = parse_response(app.clone().oneshot(bal_a).await.unwrap()).await;
    assert_eq!(ba["data"]["balance"].as_f64().unwrap(), 70.0);

    // Verify Account B balance
    let bal_b = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/accounts/{}/balance", id_b))
        .body(Body::empty())
        .unwrap();
    let (_, bb) = parse_response(app.clone().oneshot(bal_b).await.unwrap()).await;
    assert_eq!(bb["data"]["balance"].as_f64().unwrap(), 80.0);
}

#[tokio::test]
async fn test_concurrent_debit_protection() {
    let pool = init_db(Some(":memory:")).expect("Failed to initialize in-memory DB");
    let service = Arc::new(LedgerService::new(pool));

    // Create account
    let account = service
        .create_account("Concurrent Tester".to_string())
        .unwrap();

    // Credit 100
    service
        .add_transaction(
            account.id,
            transaction_ledger_api::models::CreateTransactionRequest {
                transaction_type: transaction_ledger_api::models::TransactionType::Credit,
                amount: dec!(100),
                reference_id: "INIT-CONCURRENT".to_string(),
                description: None,
            },
        )
        .unwrap();

    // Spawn 10 concurrent tasks each attempting to debit 20 (total requested: 200, only 100 available)
    // Exactly 5 must succeed, 5 must fail with InsufficientFunds
    let mut handles = Vec::new();
    for i in 0..10 {
        let s = Arc::clone(&service);
        let acc_id = account.id;
        handles.push(tokio::spawn(async move {
            s.add_transaction(
                acc_id,
                transaction_ledger_api::models::CreateTransactionRequest {
                    transaction_type: transaction_ledger_api::models::TransactionType::Debit,
                    amount: dec!(20),
                    reference_id: format!("CONCURRENT-DR-{}", i),
                    description: None,
                },
            )
        }));
    }

    let mut success_count = 0;
    let mut failure_count = 0;

    for handle in handles {
        let res = handle.await.unwrap();
        match res {
            Ok(_) => success_count += 1,
            Err(transaction_ledger_api::error::AppError::InsufficientFunds { .. }) => {
                failure_count += 1
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    assert_eq!(success_count, 5);
    assert_eq!(failure_count, 5);

    // Final balance must be 0
    let final_acc = service.get_account(account.id).unwrap();
    assert_eq!(final_acc.balance, dec!(0));

    // Invariant must be consistent
    let integrity = service.verify_integrity(account.id).unwrap();
    assert!(integrity.is_consistent);
    assert_eq!(integrity.account_balance, dec!(0));
    assert_eq!(integrity.ledger_balance, dec!(0));
}
