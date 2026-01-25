use crate::browser::BrowserInstanceWrapper;
use anyhow::anyhow;
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, stream::SplitSink};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use uuid::Uuid;

type DOClientConnection = SplitSink<WebSocket, Message>;

pub struct BrowserScheduler {
    state: Arc<Mutex<CurrentState>>,
    browsers: Arc<Mutex<HashMap<Uuid, BrowserInstanceWrapper>>>,
    /// DO client connection, used for syncing
    do_client: Arc<Mutex<Option<DOClientConnection>>>,
}

/// Just the amount of browsers for now
/// WE DONT DERIVE THIS FROM THE HASHMAP SIZE
/// this is what we publish to the durable object
#[derive(Serialize, Deserialize, Debug)]
pub struct CurrentState {
    size: u32,
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
            state: Arc::new(Mutex::new(CurrentState { size: 0 })),
            browsers: Arc::new(Mutex::new(HashMap::new())),
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

    /// Keep the DO in sync with the current state
    #[tracing::instrument(skip(self), name = "publish_state")]
    pub async fn publish_state(&self) -> anyhow::Result<()> {
        tracing::debug!("publishing state update");
        let mut guard = self.do_client.lock().await;

        if let Some(client) = guard.as_mut() {
            let encoded = {
                let state = self.state.lock().await;
                serde_json::to_string(&*state)?
            };
            tracing::debug!("sending state update message");
            client.send(Message::Text(encoded.into())).await?;
        } else {
            tracing::warn!("tried to publish state but no client connection");
        }
        Ok(())
    }

    /// Returns the first browser instance that isn't in use
    /// TODO: gracefully handle not having instances
    pub async fn request_instance(&self) -> anyhow::Result<(Uuid, String)> {
        tracing::debug!("browser instance requested");
        let mut browsers = self.browsers.lock().await;
        for (id, browser) in browsers.iter_mut() {
            if !browser.in_use {
                browser.in_use = true;
                let ws_addr = browser.browser.websocket_address().clone();
                tracing::info!(browser_id = %id, "browser instance assigned");
                return Ok((*id, ws_addr));
            }
        }
        Err(anyhow!("no available browser instances"))
    }

    /// Internal function to spawn a instance and register it
    async fn launch_new_browser(&self) -> anyhow::Result<(Uuid, String)> {
        let browser = BrowserInstanceWrapper::new().await?;
        self.register_new_browser(browser).await
    }

    /// Inserts a browser into the pool, updates state, and publishes to DO
    #[tracing::instrument(skip(self, browser), fields(browser_id = %browser.id))]
    async fn register_new_browser(
        &self,
        browser: BrowserInstanceWrapper,
    ) -> anyhow::Result<(Uuid, String)> {
        let browser_id = browser.id;
        let browser_ws = browser.browser.websocket_address().clone();
        tracing::debug!("registering new browser in pool");

        {
            let mut browsers = self.browsers.lock().await;
            if browsers.len() >= MAX_BROWSERS {
                return Err(anyhow!("exceeded browser capacity"));
            }
            browsers.insert(browser_id, browser);
        }

        let new_size = {
            let mut state = self.state.lock().await;
            state.size += 1;
            state.size
        };

        self.publish_state().await.unwrap();

        tracing::info!(pool_size = new_size, "browser registered in pool");
        Ok((browser_id, browser_ws))
    }
}
