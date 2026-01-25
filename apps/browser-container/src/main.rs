use crate::browser_scheduler::BrowserScheduler;
use axum::{
    Json, Router,
    extract::{State, WebSocketUpgrade, ws::WebSocket},
    response::{IntoResponse, Response},
    routing::{any, get, post},
};
use futures::StreamExt;
use serde::Serialize;
use std::sync::Arc;
use tokio::signal;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

pub mod browser;
pub mod browser_scheduler;

#[derive(Clone)]
struct AppState {
    scheduler: Arc<BrowserScheduler>,
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
        .route("/ping", get(health_handler))
        .route("/state", any(state_handler))
        .route("/new", post(new_session_handler))
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

async fn health_handler() -> impl IntoResponse {
    "ok"
}

#[tracing::instrument(skip_all)]
async fn new_session_handler(State(state): State<AppState>) -> impl IntoResponse {
    tracing::info!("handling new browser session request");

    match state.scheduler.request_instance().await {
        Ok((id, ws_addr)) => {
            let response = BrowserSessionResponse { id, ws_addr };
            Json(response).into_response()
        }
        Err(e) => {
            tracing::error!("failed to create browser session: {}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create browser session: {}", e),
            )
                .into_response()
        }
    }
}

async fn state_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(|socket| new_state_connection(socket, state))
}

/// Handles a new state broadcast connection
#[tracing::instrument(skip_all, fields(conn_id = %uuid::Uuid::new_v4()))]
async fn new_state_connection(socket: WebSocket, state: AppState) {
    tracing::info!("new state websocket connection");
    let (sender, _receiver) = socket.split();

    // Register the client for state broadcasts
    state.scheduler.register_do_client(sender).await.unwrap();
    state.scheduler.publish_state().await.unwrap();

    tracing::info!("state websocket connection registered");
}

#[derive(Serialize)]
struct BrowserSessionResponse {
    id: Uuid,
    ws_addr: String,
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
