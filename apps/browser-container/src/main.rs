use crate::browser_scheduler::BrowserScheduler;
use axum::{
    Router,
    extract::{State, WebSocketUpgrade, ws::WebSocket},
    response::Response,
    routing::any,
};
use futures::StreamExt;
use std::sync::Arc;
use tokio::signal;

pub mod browser;
pub mod browser_scheduler;

#[derive(Clone)]
struct AppState {
    scheduler: Arc<BrowserScheduler>,
}

#[tokio::main]
async fn main() {
    let scheduler = BrowserScheduler::new().expect("failed to create browser scheduler");
    scheduler
        .warmup()
        .await
        .expect("failed to warm up browser pool");

    let app_state = AppState {
        scheduler: Arc::new(scheduler),
    };
    let app = Router::new()
        .route("/ws", any(ws_handler))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:6700").await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(|socket| new_ws_connection(socket, state))
}

/// Handles a new client connection
async fn new_ws_connection(socket: WebSocket, state: AppState) {
    let (sender, mut receiver) = socket.split();

    // TODO: refactor
    state.scheduler.register_do_client(sender).await.unwrap();
    state.scheduler.publish_state().await.unwrap();

    while let Some(Ok(_msg)) = receiver.next().await {
        // handle new_instance
    }
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
