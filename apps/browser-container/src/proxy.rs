use crate::AppState;
use axum::extract::ws::WebSocket;
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message as TungsteniteMessage};
use uuid::Uuid;

/// DO connects via websocket to recieve capacity updates
#[tracing::instrument(skip_all, fields(conn_id = %uuid::Uuid::new_v4()))]
pub async fn new_do_connection(socket: WebSocket, state: AppState) {
    tracing::info!("new do websocket connection");
    let (sender, _receiver) = socket.split();

    // Register the DO client and publish the current capacity
    state.scheduler.register_do_client(sender).await.unwrap();
    state.scheduler.publish_capacity().await.unwrap();

    tracing::info!("state websocket connection registered");
}

/// Proxies WebSocket messages between client and Chrome DevTools
#[tracing::instrument(skip(client_socket, state), fields(session_id = %session_id))]
pub async fn proxy_to_browser(client_socket: WebSocket, session_id: Uuid, state: AppState) {
    tracing::info!("starting websocket proxy");

    let browser_ws_addr = match state.scheduler.get_browser_ws_addr(session_id).await {
        Some(addr) => addr,
        None => {
            tracing::error!("session not found");
            return;
        }
    };
    tracing::info!(browser_ws_addr = %browser_ws_addr, "connecting to browser");

    // Open a websocket connection with the browser
    let (browser_ws, _) = match connect_async(&browser_ws_addr).await {
        Ok(conn) => conn,
        Err(e) => {
            tracing::error!("failed to connect to browser: {}", e);
            return;
        }
    };

    tracing::info!("connected to browser, starting proxy");

    let (mut client_tx, mut client_rx) = client_socket.split();
    let (mut browser_tx, mut browser_rx) = browser_ws.split();

    // Proxy between opened browser ws connection and the connected client
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
    // At this point, we can assume the client terminated the connection or the session crashed
    if let Err(e) = state.scheduler.remove_instance(session_id).await {
        tracing::warn!("error removing instance: {}", e.to_string());
    };
}
