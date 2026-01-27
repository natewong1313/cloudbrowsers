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
const MAX_BROWSERS: usize = 2;
// If we can't setup all {MAX_BROWSERS} after 10 attempts somethings fucked
const MAX_WARMUP_ATTEMPTS: u32 = 10;

impl BrowserScheduler {
    /// The BrowserScheduler maintains the pool of browsers
    /// You should go through this API in order to provision browsers
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            browsers: Arc::new(Mutex::new(HashMap::new())),
            capacity: Arc::new(Mutex::new(0)),
            do_client: Arc::new(Mutex::new(None)),
        })
    }

    /// "warm up" the pool by opening up MAX_BROWSERS
    /// will continuosly attempt to satisfy spawning all browsers
    #[tracing::instrument(skip(self), name = "warmup")]
    pub async fn warmup(&self) -> anyhow::Result<()> {
        tracing::info!(max_browsers = MAX_BROWSERS, "starting browser pool warmup");
        let mut spawned = 0;
        let mut attempts = 0;

        while spawned < MAX_BROWSERS && attempts < MAX_WARMUP_ATTEMPTS {
            let remaining = MAX_BROWSERS - spawned;

            // chromiumoxide isn't cpu bound, since we only have the orchestration overhead
            let futures: Vec<_> = (0..remaining).map(|_| self.launch_new_browser()).collect();
            let results = futures::future::join_all(futures).await;
            for result in results {
                match result {
                    Ok(_) => spawned += 1,
                    Err(err) => tracing::warn!("failed to spawn browser instance: {}", err),
                }
            }
            // If we spawned 0 on the first pass then somethings wrong
            if spawned == 0 && attempts == 0 {
                return Err(anyhow!("could not spawn any browsers on first pass",));
            }
            attempts += 1;
        }

        if spawned == 0 {
            return Err(anyhow!(
                "could not spawn any browsers after {} attempts",
                attempts
            ));
        }

        if spawned < MAX_BROWSERS {
            tracing::warn!(
                spawned,
                max = MAX_BROWSERS,
                attempts,
                "partial warmup - not all browsers spawned"
            );
        }

        tracing::info!(spawned, "browser pool warmup complete");
        Ok(())
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

    /// Returns the first browser instance that isn't in use
    /// TODO: gracefully handle not having instances
    pub async fn request_instance(&self) -> anyhow::Result<Uuid> {
        tracing::debug!("browser instance requested");
        let mut browsers = self.browsers.lock().await;
        for (id, browser) in browsers.iter_mut() {
            if !browser.in_use {
                browser.in_use = true;
                tracing::info!(browser_id = %id, "browser instance assigned");
                return Ok(*id);
            }
        }
        Err(anyhow!("no available browser instances"))
    }

    pub async fn remove_instance(&self, id: Uuid) -> anyhow::Result<()> {
        tracing::debug!("removing browser instance");

        let capacity = {
            let mut browsers = self.browsers.lock().await;
            browsers.remove_entry(&id);

            let mut capacity = self.capacity.lock().await;
            tracing::debug!("prev capacity: {}", *capacity);
            *capacity -= 1;
            tracing::debug!("new capacity: {}", *capacity);
            capacity.clone()
        };

        if capacity < MAX_BROWSERS.try_into().unwrap() {
            // TODO: maybe silently fail here
            self.launch_new_browser().await?;
        } else {
            tracing::warn!("UNEXPECTED removed browser instance but still at max capacity");
        }

        Ok(())
    }

    /// Get the WebSocket address for a specific browser instance
    pub async fn get_browser_ws_addr(&self, id: Uuid) -> Option<String> {
        let browsers = self.browsers.lock().await;
        browsers
            .get(&id)
            .map(|b| b.browser.websocket_address().clone())
    }

    /// Internal function to spawn a instance and register it
    async fn launch_new_browser(&self) -> anyhow::Result<Uuid> {
        // TODO:retry if creating the browser fails
        // TODO: need to be more careful with the lock here
        let browser = BrowserInstanceWrapper::new().await?;
        self.register_new_browser(browser).await
    }

    /// Inserts a browser into the pool, updates state, and publishes to DO
    #[tracing::instrument(skip(self, browser), fields(browser_id = %browser.id))]
    async fn register_new_browser(&self, browser: BrowserInstanceWrapper) -> anyhow::Result<Uuid> {
        let browser_id = browser.id;
        tracing::debug!("registering new browser in pool");

        {
            let mut browsers = self.browsers.lock().await;
            if browsers.len() >= MAX_BROWSERS {
                return Err(anyhow!("exceeded browser capacity"));
            }
            browsers.insert(browser_id, browser);
        }

        let new_capcity = {
            let mut capacity = self.capacity.lock().await;
            *capacity += 1;
            capacity.clone()
        };

        self.publish_capacity().await.unwrap();

        tracing::info!(pool_size = new_capcity, "browser registered in pool");
        Ok(browser_id)
    }
}
