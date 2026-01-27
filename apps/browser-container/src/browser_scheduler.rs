use crate::browser::BrowserInstanceWrapper;
use anyhow::anyhow;
use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use futures::{SinkExt, stream::SplitSink};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use uuid::Uuid;

type DOClientConnection = SplitSink<WebSocket, Message>;

/// This should be called before starting the server to fail fast if the browser args are messed up
#[tracing::instrument(name = "test_browser_start")]
pub async fn test_browser_start() -> anyhow::Result<()> {
    tracing::info!("testing browser startup...");
    let mut browser = BrowserInstanceWrapper::new().await?;
    browser.cleanup().await;
    tracing::info!("browser startup test passed");
    Ok(())
}

pub struct BrowserScheduler {
    browsers: Arc<Mutex<HashMap<Uuid, BrowserInstanceWrapper>>>,
    capacity: Arc<Mutex<u32>>,
    /// DO client connection, used for syncing
    do_client: Arc<Mutex<Option<DOClientConnection>>>,
}

// safe to hard code this for now since we can't change instance type dynamically
const MAX_BROWSERS: u32 = 2;

impl BrowserScheduler {
    /// The BrowserScheduler maintains the pool of browsers
    /// You should go through this API in order to provision browsers
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            browsers: Arc::new(Mutex::new(HashMap::new())),
            capacity: Arc::new(Mutex::new(MAX_BROWSERS)),
            do_client: Arc::new(Mutex::new(None)),
        })
    }

    /// Close all opened browsers
    pub async fn cleanup(&self) {
        let mut browsers = self.browsers.lock().await;
        for browser in browsers.values_mut() {
            browser.cleanup().await;
        }
        browsers.clear();
    }

    /// Registers a new connected durable object client
    #[tracing::instrument(skip(self), name = "register_do_client")]
    pub async fn register_do_client(&self, do_client: DOClientConnection) -> anyhow::Result<()> {
        tracing::debug!("registering new do client");
        let mut guard = self.do_client.lock().await;
        *guard = Some(do_client);

        tracing::debug!("registered new do client");
        Ok(())
    }

    /// Returns a clone of the DO client connection handle for sending messages
    pub fn get_do_client(&self) -> Arc<Mutex<Option<DOClientConnection>>> {
        self.do_client.clone()
    }

    /// Keep the DO in sync with the current capacity
    #[tracing::instrument(skip(self), name = "publish_capacity")]
    pub async fn publish_capacity(&self) -> anyhow::Result<()> {
        tracing::debug!("publishing state update");
        let mut guard = self.do_client.lock().await;

        if let Some(client) = guard.as_mut() {
            let capacity = self.capacity.lock().await;
            let capacity_str = Utf8Bytes::from(capacity.to_string());
            tracing::debug!("sending capacity update message");
            client.send(Message::Text(capacity_str)).await?;
        } else {
            tracing::warn!("tried to publish state but no client connection");
        }
        Ok(())
    }

    /// Returns the first browser instance that isn't in use, or creates one if capacity allows
    pub async fn request_instance(&self) -> anyhow::Result<Uuid> {
        tracing::debug!("browser instance requested");

        let able_to_allocate = {
            let mut capacity = self.capacity.lock().await;
            match *capacity > 0 {
                true => {
                    *capacity -= 1;
                    true
                }
                false => false,
            }
        };

        if !able_to_allocate {
            return Err(anyhow!(
                "no available browser instances and no capacity to create new ones"
            ));
        }
        self.publish_capacity().await.unwrap();

        let browser = BrowserInstanceWrapper::new().await?;
        let browser_id = browser.id.clone();

        let mut browsers = self.browsers.lock().await;
        browsers.insert(browser.id, browser);

        tracing::info!(browser_id = %browser_id, "new browser instance created");
        Ok(browser_id)
    }

    pub async fn remove_instance(&self, id: Uuid) -> anyhow::Result<()> {
        tracing::debug!("removing browser instance");

        {
            let mut capacity = self.capacity.lock().await;
            tracing::debug!("prev capacity: {}", *capacity);
            *capacity += 1;
            tracing::debug!("new capacity: {}", *capacity);

            let mut browsers = self.browsers.lock().await;
            browsers.remove_entry(&id);
        };

        self.publish_capacity().await?;

        Ok(())
    }

    /// Get the WebSocket address for a specific browser instance
    pub async fn get_browser_ws_addr(&self, id: Uuid) -> Option<String> {
        let browsers = self.browsers.lock().await;
        browsers
            .get(&id)
            .map(|b| b.browser.websocket_address().clone())
    }
}
