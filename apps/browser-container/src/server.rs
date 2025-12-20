use anyhow::Context;
use axum::extract::WebSocketUpgrade;
use axum::extract::ws::WebSocket;
use axum::{Router, response::IntoResponse, routing::any};
use chromiumoxide::Browser;
use futures_util::{sink::SinkExt, stream::StreamExt};
use std::net::SocketAddr;
use tokio_tungstenite::connect_async;
use tungstenite::client::IntoClientRequest;

use crate::launcher;

pub async fn serve() -> anyhow::Result<(), anyhow::Error> {
    let app = Router::new().route("/connect", any(ws_handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:6700")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket_proxy)
}

// our server forwards messages to/from the client to/from the browser
async fn handle_socket_proxy(client_socket: WebSocket) {
    // TODO: dont unwrap and move to pool structure
    let browser_info = launcher::launch_browser().await.unwrap();
    let browser = browser_info.browser;
    let mut browser_handler = browser_info.handler;
    let _handle = tokio::spawn(async move {
        while let Some(h) = browser_handler.next().await {
            if h.is_err() {
                break;
            }
        }
    });

    let browser_stream = connect_to_browser(browser).await.unwrap();
    let (mut browser_sender, mut browser_recv) = browser_stream.split();

    let (mut client_sender, mut client_recv) = client_socket.split();

    // client -> browser
    let mut client_to_browser_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = client_recv.next().await {
            let browser_msg = axum_msg_to_tungstenite(msg);
            if browser_sender.send(browser_msg).await.is_err() {
                break;
            }
        }
    });

    // browser -> client
    let mut browser_to_client_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = browser_recv.next().await {
            let client_msg = tungstenite_msg_to_axum(msg);
            if client_sender.send(client_msg).await.is_err() {
                break;
            }
        }
    });

    // If any one of the tasks exit, abort the other.
    tokio::select! {
        ctb_res = (&mut client_to_browser_task) => {
            match ctb_res {
                Ok(_) => println!("client to browser proxy finished"),
                Err(err) => println!("err forwarding messages {err:?}")
            }
            browser_to_client_task.abort();
        },
        btc_res = (&mut browser_to_client_task) => {
            match btc_res {
                Ok(_) => println!("browser to client proxy finished"),
                Err(err) => println!("err forwarding messages {err:?}")
            }
            client_to_browser_task.abort();
        }
    }
}

// Connects to the browsers websocket port and returns the sender and reciever handles
async fn connect_to_browser(
    browser: Browser,
) -> anyhow::Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    anyhow::Error,
> {
    let addr = browser.websocket_address();
    let request = addr.into_client_request().context("into client req")?;

    let (ws_stream, _) = connect_async(request).await.context("connect browser ws")?;
    tracing::debug!("connected to browser ws on {}", addr);

    Ok(ws_stream)
}

fn tungstenite_msg_to_axum(msg: tungstenite::Message) -> axum::extract::ws::Message {
    return match msg {
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
            panic!("Fix this eventually")
        }
    };
}

fn axum_msg_to_tungstenite(msg: axum::extract::ws::Message) -> tungstenite::Message {
    return match msg {
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
    };
}
