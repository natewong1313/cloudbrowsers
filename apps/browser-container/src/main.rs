use axum::{Router, routing::get};
use tokio::signal;

use crate::browser::BrowserInstanceWrapper;

pub mod browser;
pub mod browser_scheduler;

#[tokio::main]
async fn main() {
    let app = Router::new().route("/new", get(new_instance));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:6700").await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn new_instance() -> String {
    let instance = BrowserInstanceWrapper::new().await.unwrap();
    let ws_addr = instance.browser.websocket_address();
    return ws_addr.clone();
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
