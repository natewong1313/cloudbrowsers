use anyhow::Context;
use tokio::sync::mpsc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub mod browser_scheduler;
pub mod browser_session;
pub mod server;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "browser_container=debug,tungstenite=warn".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    // let (send, recv) = mpsc::channel(69);
    // tokio::spawn(async move {
    //     browser_scheduler::start(recv).await;
    // });

    server::serve().await.context("Server exploded").unwrap();
}
