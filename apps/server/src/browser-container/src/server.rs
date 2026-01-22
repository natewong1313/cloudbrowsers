use anyhow::Context;
use axum::extract::WebSocketUpgrade;
use axum::extract::ws::WebSocket;
use axum::{
    Router,
    response::IntoResponse,
    routing::{any, get},
};
use chromiumoxide::Browser;
use futures::stream::{SplitSink, SplitStream};
use futures_util::{sink::SinkExt, stream::StreamExt};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio_tungstenite::{WebSocketStream, connect_async};
use tungstenite::client::IntoClientRequest;

use crate::browser_session::{self, BrowserSession};

pub async fn serve() -> anyhow::Result<(), anyhow::Error> {
    let app = Router::new()
        // .route("/new", get(new_session_handler))
        .route("/connect", any(ws_handler));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:6700").await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(async |socket| {
        tracing::debug!("WebSocket connection established");
        if let Err(err) = handle_socket_proxy(socket).await {
            tracing::error!("exited handle_socket_proxy unexpectedly with {}", err);
        };
    })
}

// our server forwards messages to/from the client to/from the browser
async fn handle_socket_proxy(client_socket: WebSocket) -> anyhow::Result<()> {
    let mut browser_session = BrowserSession::launch().await?;
    let browser_ws_addr = browser_session.ws_addr().clone();

    // TODO: return which side ended
    handle_proxy(browser_ws_addr, client_socket).await?;

    tracing::debug!("Proxy ended");
    browser_session.cleanup().await;
    drop(browser_session);
    Ok(())
}

async fn handle_proxy(browser_ws_addr: String, client_socket: WebSocket) -> anyhow::Result<()> {
    let browser_stream = connect_to_browser_ws(browser_ws_addr)
        .await
        .context("failed to connect to browser")?;
    let (browser_tx, browser_rx) = browser_stream.split();
    let (client_tx, client_rx) = client_socket.split();

    // race whichever proxy side disconnects
    tokio::select! {
        _ = forward_client_to_browser(client_rx, browser_tx) => {},
        _ = forward_browser_to_client(browser_rx, client_tx) => {},
    }

    Ok(())
}

// Connects to the browsers websocket port and returns the websocket stream
async fn connect_to_browser_ws(
    browser_ws_addr: String,
) -> anyhow::Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
> {
    let request = browser_ws_addr
        .clone()
        .into_client_request()
        .context("failed getting websocket request")?;

    let (ws_stream, _) = connect_async(request)
        .await
        .context("failed to connect to browser websocket")?;
    tracing::debug!("Connected to browser websocket at {}", browser_ws_addr);

    Ok(ws_stream)
}

// Forwards client ws msgs to browser ws msgs
async fn forward_client_to_browser(
    mut client_rx: SplitStream<WebSocket>,
    mut browser_tx: SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        tungstenite::Message,
    >,
) -> anyhow::Result<()> {
    while let Some(msg) = client_rx.next().await {
        let msg = match msg {
            Ok(msg) => msg,
            Err(err) => {
                tracing::warn!("Client message recv err: {}", err);
                continue; // TODO: we may need to exit here, not sure
            }
        };

        let browser_msg = match axum_to_tungstenite(msg) {
            Ok(browser_msg) => browser_msg,
            Err(err) => {
                tracing::warn!("Client message convert err: {}", err);
                continue; // TODO: we may need to exit here, not sure
            }
        };

        if let Err(err) = browser_tx.send(browser_msg).await {
            return Err(anyhow::anyhow!(
                "Error forwarding message to browser: {}",
                err
            ));
        }
    }
    Ok(())
}

async fn forward_browser_to_client(
    mut browser_rx: SplitStream<
        WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    >,
    mut client_tx: SplitSink<WebSocket, axum::extract::ws::Message>,
) -> anyhow::Result<()> {
    while let Some(msg) = browser_rx.next().await {
        let msg = match msg {
            Ok(msg) => msg,
            Err(err) => {
                tracing::warn!("Browser message recv err: {}", err);
                continue; // TODO: we may need to exit here, not sure
            }
        };

        let client_msg = match tungstenite_to_axum(msg) {
            Ok(client_msg) => client_msg,
            Err(err) => {
                tracing::warn!("Browser message convert err: {}", err);
                continue; // TODO: we may need to exit here, not sure
            }
        };

        if let Err(err) = client_tx.send(client_msg).await {
            return Err(anyhow::anyhow!(
                "Error forwarding message to client: {}",
                err
            ));
        }
    }
    Ok(())
}

// Some fuckery because axum uses tungstenite internally but doesnt let you use it
fn tungstenite_to_axum(msg: tungstenite::Message) -> Result<axum::extract::ws::Message, String> {
    Ok(match msg {
        tungstenite::Message::Text(text) => {
            axum::extract::ws::Message::Text(text.to_string().into())
        }
        tungstenite::Message::Binary(data) => axum::extract::ws::Message::Binary(data),
        tungstenite::Message::Ping(data) => axum::extract::ws::Message::Ping(data),
        tungstenite::Message::Pong(data) => axum::extract::ws::Message::Pong(data),
        tungstenite::Message::Close(frame) => {
            let close_frame = frame.map(|f| axum::extract::ws::CloseFrame {
                code: f.code.into(),
                reason: f.reason.to_string().into(),
            });
            axum::extract::ws::Message::Close(close_frame)
        }
        tungstenite::Message::Frame(_) => {
            return Err("Raw frame messages are not supported".to_string());
        }
    })
}

fn axum_to_tungstenite(msg: axum::extract::ws::Message) -> Result<tungstenite::Message, String> {
    Ok(match msg {
        axum::extract::ws::Message::Text(text) => {
            tungstenite::Message::Text(text.to_string().into())
        }
        axum::extract::ws::Message::Binary(data) => tungstenite::Message::Binary(data),
        axum::extract::ws::Message::Ping(data) => tungstenite::Message::Ping(data),
        axum::extract::ws::Message::Pong(data) => tungstenite::Message::Pong(data),
        axum::extract::ws::Message::Close(frame) => {
            let close_frame = frame.map(|f| tungstenite::protocol::CloseFrame {
                code: tungstenite::protocol::frame::coding::CloseCode::from(f.code),
                reason: f.reason.to_string().into(),
            });
            tungstenite::Message::Close(close_frame)
        }
    })
}
