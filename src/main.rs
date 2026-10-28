use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod app;
mod db;
mod error;
mod handlers;
mod models;
mod service;

use app::create_app;
use db::init_db;
use service::LedgerService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "transaction_ledger_api=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Initialize database
    let pool = init_db(None)?;
    let service = Arc::new(LedgerService::new(pool));

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    let app = create_app(service);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    println!("====================================================");
    println!("Rust Transaction Ledger API running on port {}", port);
    println!("Web Dashboard Playground: http://localhost:{}", port);
    println!("Healthcheck:              http://localhost:{}/api/v1/health", port);
    println!("====================================================");

    info!("Server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
