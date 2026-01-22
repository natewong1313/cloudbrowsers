use axum::{Router, routing::get};

use crate::browser::BrowserInstanceWrapper;

pub mod browser;

#[tokio::main]
async fn main() {
    let app = Router::new().route("/new", get(new_instance));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:6700").await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn new_instance() -> String {
    let instance = BrowserInstanceWrapper::new().await.unwrap();
    let ws_addr = instance.browser.websocket_address();
    return ws_addr.clone();
}
