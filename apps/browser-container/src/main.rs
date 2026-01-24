use crate::browser_scheduler::BrowserScheduler;
use anyhow::anyhow;
use axum::{
    Router,
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::{IntoResponse, Response},
    routing::{any, get},
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::signal;
use tracing_subscriber::EnvFilter;

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
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive(tracing::Level::DEBUG.into()),
        )
        .init();

    let scheduler = Arc::new(BrowserScheduler::new().expect("failed to create browser scheduler"));

    let app_state = AppState {
        scheduler: scheduler.clone(),
    };

    let app = Router::new()
        .route("/ping", get(health_handler))
        .route("/ws", any(ws_handler))
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

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(|socket| new_ws_connection(socket, state))
}

/// Handles a new client connection
#[tracing::instrument(skip_all, fields(conn_id = %uuid::Uuid::new_v4()))]
async fn new_ws_connection(socket: WebSocket, state: AppState) {
    tracing::info!("new websocket connection");
    let (sender, mut receiver) = socket.split();

    // TODO: refactor
    state.scheduler.register_do_client(sender).await.unwrap();
    state.scheduler.publish_state().await.unwrap();

    while let Some(Ok(msg)) = receiver.next().await {
        if let Err(e) = handle_new_message(msg, &state).await {
            tracing::error!("error handling message: {}", e);
        };
    }
    tracing::info!("websocket connection closed");
}

#[tracing::instrument(skip_all)]
async fn handle_new_message(msg: Message, state: &AppState) -> anyhow::Result<()> {
    tracing::debug!("handling incoming message");
    let parsed_msg = get_parsed_msg(msg)?;

    match parsed_msg.msg_type {
        MessageFromDOType::NewBrowserSession => {
            state.scheduler.request_instance().await?;
        }
    }

    Ok(())
}

#[derive(Deserialize)]
enum MessageFromDOType {
    NewBrowserSession,
}
#[derive(Deserialize)]
struct MessageFromDO {
    msg_type: MessageFromDOType,
}

fn get_parsed_msg(msg: Message) -> anyhow::Result<MessageFromDO> {
    if let Message::Text(msg) = msg {
        let parsed_msg = serde_json::from_str::<MessageFromDO>(&msg)?;
        return Ok(parsed_msg);
    }
    Err(anyhow!("invalid message type provided"))
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
