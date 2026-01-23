use crate::browser_scheduler::BrowserScheduler;
use axum::{Router, extract::State, http::StatusCode, routing::get};
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
        .route("/new", get(new_instance))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:6700").await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn new_instance(State(state): State<AppState>) -> Result<String, (StatusCode, String)> {
    let (_, ws_addr) = state
        .scheduler
        .request_instance()
        .await
        // shouldn't publicly expose the error but im gonna refactor error handling anyways
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    return Ok(ws_addr);
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
