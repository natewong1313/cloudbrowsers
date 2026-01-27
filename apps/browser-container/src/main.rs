use crate::browser_scheduler::BrowserScheduler;
use axum::{
    Router,
    routing::{any, get, post},
};
use serde::Serialize;
use std::sync::Arc;
use tokio::signal;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

pub mod browser;
pub mod browser_scheduler;
pub mod handlers;
pub mod proxy;

#[derive(Clone)]
pub struct AppState {
    pub scheduler: Arc<BrowserScheduler>,
}

#[derive(Serialize)]
pub struct NewBrowserSessionResponse {
    pub id: Uuid,
}

#[tokio::main]
async fn main() {
    // Initialize tracing subscriber with human-readable output
    // Default level is DEBUG, override with RUST_LOG env var
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
        .init();

    let scheduler = Arc::new(BrowserScheduler::new().expect("failed to create browser scheduler"));

    let app_state = AppState {
        scheduler: scheduler.clone(),
    };

    let app = Router::new()
        .route("/ping", get(handlers::health_handler))
        .route("/capacity", any(handlers::do_capacity_handler))
        .route("/new", post(handlers::new_session_handler))
        .route("/session/{id}", any(handlers::session_ws_handler))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:6700").await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());

    tokio::spawn(async move {
        if let Err(e) = scheduler.warmup().await {
            tracing::error!("failed to warm up browser pool: {}", e)
        }
    });

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
