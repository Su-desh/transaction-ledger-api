use axum::{
    response::Html,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::handlers::{account, transaction, transfer};
use crate::service::LedgerService;

pub fn create_app(service: Arc<LedgerService>) -> Router {
    let api_routes = Router::new()
        .route("/health", get(healthcheck))
        .route(
            "/accounts",
            post(account::create_account).get(account::get_all_accounts),
        )
        .route("/accounts/{account_id}", get(account::get_account_by_id))
        .route(
            "/accounts/{account_id}/balance",
            get(account::get_account_balance),
        )
        .route(
            "/accounts/{account_id}/integrity",
            get(account::verify_integrity),
        )
        .route(
            "/accounts/{account_id}/transactions",
            post(transaction::add_transaction)
                .get(transaction::get_transaction_history)
                .put(transaction::method_not_allowed)
                .patch(transaction::method_not_allowed)
                .delete(transaction::method_not_allowed),
        )
        .route(
            "/accounts/{account_id}/transactions/{transaction_id}",
            post(transaction::method_not_allowed)
                .put(transaction::method_not_allowed)
                .patch(transaction::method_not_allowed)
                .delete(transaction::method_not_allowed),
        )
        .route("/transfers", post(transfer::transfer_funds));

    Router::new()
        .route("/", get(serve_index))
        .nest("/api/v1", api_routes)
        .fallback_service(ServeDir::new("static").append_index_html_on_directories(true))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(service)
}

async fn serve_index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn healthcheck() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "UP",
        "service": "Transaction Ledger API (Rust Engine)",
        "version": "1.0.0",
        "engine": "Rust / Axum / Tokio / SQLite"
    }))
}
