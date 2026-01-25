use crate::browser_scheduler::BrowserScheduler;
use axum::{
    Json, Router,
    extract::{Path, State, WebSocketUpgrade, ws::WebSocket},
    response::{IntoResponse, Response},
    routing::{any, get, post},
};
use futures::{SinkExt, StreamExt};
use serde::Serialize;
use std::sync::Arc;
use tokio::signal;
use tokio_tungstenite::{connect_async, tungstenite::Message as TungsteniteMessage};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

pub mod browser;
pub mod browser_scheduler;

#[derive(Clone)]
struct AppState {
    scheduler: Arc<BrowserScheduler>,
}

#[derive(Serialize)]
struct NewBrowserSessionResponse {
    id: Uuid,
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
        .route("/session/{id}", any(session_ws_handler))
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
        Ok(id) => {
            let response = NewBrowserSessionResponse { id };
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
    state.scheduler.publish_capacity().await.unwrap();

    tracing::info!("state websocket connection registered");
}

/// Handles WebSocket proxy requests to browser DevTools
#[tracing::instrument(skip(ws, state), fields(session_id = %id))]
async fn session_ws_handler(
    ws: WebSocketUpgrade,
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Response {
    tracing::info!("session websocket proxy request");

    let session_id = match Uuid::parse_str(&id) {
        Ok(uuid) => uuid,
        Err(e) => {
            tracing::error!("invalid session id: {}", e);
            return (
                axum::http::StatusCode::BAD_REQUEST,
                format!("Invalid session ID: {}", e),
            )
                .into_response();
        }
    };

    ws.on_upgrade(move |socket| proxy_to_browser(socket, session_id, state))
}

/// Proxies WebSocket messages between client and Chrome DevTools
#[tracing::instrument(skip(client_socket, state), fields(session_id = %session_id))]
async fn proxy_to_browser(client_socket: WebSocket, session_id: Uuid, state: AppState) {
    tracing::info!("starting websocket proxy");

    // Get the browser's WebSocket address
    let browser_ws_addr = match state.scheduler.get_browser_ws_addr(session_id).await {
        Some(addr) => addr,
        None => {
            tracing::error!("session not found");
            return;
        }
    };

    tracing::info!(browser_ws_addr = %browser_ws_addr, "connecting to browser");

    // Connect to the browser's DevTools WebSocket
    let (browser_ws, _) = match connect_async(&browser_ws_addr).await {
        Ok(conn) => conn,
        Err(e) => {
            tracing::error!("failed to connect to browser: {}", e);
            return;
        }
    };

    tracing::info!("connected to browser, starting message relay");

    let (mut client_tx, mut client_rx) = client_socket.split();
    let (mut browser_tx, mut browser_rx) = browser_ws.split();

    // Relay messages bidirectionally
    let client_to_browser = async {
        while let Some(msg) = client_rx.next().await {
            match msg {
                Ok(axum::extract::ws::Message::Text(text)) => {
                    if let Err(e) = browser_tx
                        .send(TungsteniteMessage::Text(text.to_string()))
                        .await
                    {
                        tracing::error!("error sending to browser: {}", e);
                        break;
                    }
                }
                Ok(axum::extract::ws::Message::Binary(data)) => {
                    if let Err(e) = browser_tx
                        .send(TungsteniteMessage::Binary(data.to_vec()))
                        .await
                    {
                        tracing::error!("error sending to browser: {}", e);
                        break;
                    }
                }
                Ok(axum::extract::ws::Message::Close(_)) => {
                    tracing::info!("client closed connection");
                    break;
                }
                Err(e) => {
                    tracing::error!("error receiving from client: {}", e);
                    break;
                }
                _ => {}
            }
        }
    };

    let browser_to_client = async {
        while let Some(msg) = browser_rx.next().await {
            match msg {
                Ok(TungsteniteMessage::Text(text)) => {
                    if let Err(e) = client_tx
                        .send(axum::extract::ws::Message::Text(text.into()))
                        .await
                    {
                        tracing::error!("error sending to client: {}", e);
                        break;
                    }
                }
                Ok(TungsteniteMessage::Binary(data)) => {
                    if let Err(e) = client_tx
                        .send(axum::extract::ws::Message::Binary(data.into()))
                        .await
                    {
                        tracing::error!("error sending to client: {}", e);
                        break;
                    }
                }
                Ok(TungsteniteMessage::Close(_)) => {
                    tracing::info!("browser closed connection");
                    break;
                }
                Err(e) => {
                    tracing::error!("error receiving from browser: {}", e);
                    break;
                }
                _ => {}
            }
        }
    };

    tokio::select! {
        _ = client_to_browser => tracing::info!("client to browser relay ended"),
        _ = browser_to_client => tracing::info!("browser to client relay ended"),
    }

    tracing::info!("websocket proxy session ended");
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
