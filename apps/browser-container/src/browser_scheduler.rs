use crate::browser::BrowserInstanceWrapper;
use anyhow::anyhow;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

pub struct BrowserScheduler {
    browsers: Arc<Mutex<HashMap<Uuid, BrowserInstanceWrapper>>>,
}

// lol
const MAX_BROWSERS: u32 = 3;
// If we can't setup all {MAX_BROWSERS} after 10 attempts somethings fucked
const MAX_WARMUP_ATTEMPTS: u32 = 10;

impl BrowserScheduler {
    /// The BrowserScheduler maintains the pool of browsers
    /// You should go through this API in order to provision browsers
    pub fn new() -> anyhow::Result<Self> {
        let browsers = Arc::new(Mutex::new(HashMap::new()));

        return Ok(Self { browsers });
    }

    /// "warm up" the pool by opening up MAX_BROWSERS
    /// will continuosly attempt to satisfy spawning all browsers
    pub async fn warmup(&self) -> anyhow::Result<()> {
        let mut spawned = 0;
        let mut attempts = 0;

        while spawned < MAX_BROWSERS && attempts < MAX_WARMUP_ATTEMPTS {
            let remaining = MAX_BROWSERS - spawned;

            // chromiumoxide isn't cpu bound, since we only have the orchestration overhead
            let futures: Vec<_> = (0..remaining).map(|_| self.request_instance()).collect();
            let results = futures::future::join_all(futures).await;
            for result in results {
                match result {
                    Ok(_) => spawned += 1,
                    Err(err) => eprintln!("Failed to spawn browser instance: {}", err),
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
            eprintln!(
                "only spawned {}/{} browsers after {} attempts",
                spawned, MAX_BROWSERS, attempts
            );
        }

        Ok(())
    }

    /// Adds a new browser instance and loads a new page
    /// For now, this returns the instance id and its websocket connection
    pub async fn request_instance(&self) -> anyhow::Result<(Uuid, String)> {
        let browser = BrowserInstanceWrapper::new().await?;
        let browser_id = browser.id;
        let browser_ws = browser.browser.websocket_address().clone();

        self.browsers
            .lock()
            .map_err(|err| anyhow!("could not acquire browser lock: {}", err))?
            .insert(browser_id, browser);

        Ok((browser_id, browser_ws))
    }
}
