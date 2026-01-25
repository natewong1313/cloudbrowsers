use crate::{AppState, NewBrowserSessionResponse, proxy};
use axum::{
    Json,
    extract::{Path, State, WebSocketUpgrade},
    response::{IntoResponse, Response},
};
use uuid::Uuid;

pub async fn health_handler() -> impl IntoResponse {
    "ok"
}

#[tracing::instrument(skip_all)]
pub async fn new_session_handler(State(state): State<AppState>) -> impl IntoResponse {
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

pub async fn do_capacity_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(|socket| proxy::new_do_connection(socket, state))
}

/// Proxy from a users puppeteer/playwright/etc instance to the browsers devtools
#[tracing::instrument(skip(ws, state), fields(session_id = %id))]
pub async fn session_ws_handler(
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

    ws.on_upgrade(move |socket| proxy::proxy_to_browser(socket, session_id, state))
}
