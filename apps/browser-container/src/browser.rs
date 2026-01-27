use anyhow::anyhow;
use chromiumoxide::BrowserConfig;
use futures::StreamExt;
use port_check::free_local_port;
use std::{env, time::Duration};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind};
use tempfile::tempdir;
use tokio::{task::JoinHandle, time::sleep};
use uuid::Uuid;

pub struct BrowserInstanceWrapper {
    pub id: Uuid,
    pub browser: chromiumoxide::Browser,
    poller_handle: JoinHandle<()>,
    watchdog_handle: JoinHandle<()>,
}
impl BrowserInstanceWrapper {
    /// Creates a new browser instance
    /// This will also spawn the chromiumoxide poller and the watchdog
    /// You will need to call cleanup() whenever finished
    #[tracing::instrument(name = "browser_new")]
    pub async fn new() -> anyhow::Result<Self> {
        tracing::debug!("creating new browser instance");
        let user_data_dir = tempdir()?;

        let Some(free_port) = free_local_port() else {
            return Err(anyhow!("could not get a free local port"));
        };

        let mut base_config = BrowserConfig::builder().with_head();
        if env::var("IN_DOCKER").unwrap_or_default() == "true" {
            tracing::debug!("using headless mode");
            base_config = base_config
                .new_headless_mode()
                .arg("--disable-gpu")
                .arg("--disable-setuid-sandbox")
                .arg("--disable-dev-shm-usage")
        }

        let browser_config = base_config
            .user_data_dir(user_data_dir)
            .no_sandbox()
            .arg(format!("--remote-debugging-port={}", free_port))
            .build()
            .map_err(|e| anyhow::anyhow!(e))?;

        let (mut browser, handler) = chromiumoxide::Browser::launch(browser_config).await?;

        // Must poll before doing any events
        let poller_handle = tokio::spawn(async move {
            if let Err(err) = Self::browser_handler_loop(handler).await {
                tracing::warn!("{}", err);
            };
        });

        // the browser itself runs in a seperate process
        let pid = browser
            .get_mut_child()
            .ok_or_else(|| anyhow!("error getting browser child proc"))?
            .as_mut_inner()
            .id()
            .ok_or_else(|| anyhow!("no pid from browser child proc"))?;

        let watchdog_handle = tokio::spawn(async move {
            if let Err(err) = Self::watchdog_loop(pid).await {
                tracing::warn!("{}", err);
            };
        });

        let id = Uuid::new_v4();
        tracing::info!(browser_id = %id, pid = pid, "browser instance created");
        Ok(BrowserInstanceWrapper {
            id,
            browser,
            poller_handle,
            watchdog_handle,
        })
    }

    #[tracing::instrument(skip(self), fields(browser_id = %self.id))]
    pub async fn cleanup(&mut self) {
        tracing::info!("cleaning up browser instance");
        self.poller_handle.abort();
        self.watchdog_handle.abort();

        // we can ignore errors here
        if let Some(child) = self.browser.get_mut_child() {
            // kill child process
            let _ = child.kill().await;
        }
        let _ = self.browser.close().await;
        tracing::debug!("browser cleanup complete");
    }

    /// Poll the chromiumoxide events
    async fn browser_handler_loop(mut handler: chromiumoxide::Handler) -> anyhow::Result<()> {
        while let Some(event) = handler.next().await {
            if let Err(err) = event {
                return Err(anyhow!("unexpected browser handler error: {}", err));
            }
        }
        Ok(())
    }

    /// monitor memory, cpu usage. currently doesn't do anything to kill the process
    /// and does not handle noisy neighbor issues
    async fn watchdog_loop(pid: u32) -> anyhow::Result<()> {
        let s_pid = sysinfo::Pid::from_u32(pid);
        let monitoring_attrs = ProcessRefreshKind::nothing().with_memory().with_cpu();
        let mut monitoring_instance = sysinfo::System::new_with_specifics(
            RefreshKind::nothing().with_processes(monitoring_attrs),
        );
        loop {
            if let Some(p) = monitoring_instance.process(s_pid)
                && p.exists()
            {
                let mem = p.memory() as f64 / 1_000_000.0;
                let cpu = p.cpu_usage();
                // tracing::debug!(memory_mb = %mem, cpu_pct = %cpu, pid = pid, "browser watchdog");

                monitoring_instance.refresh_processes_specifics(
                    ProcessesToUpdate::Some(&[s_pid]),
                    true,
                    monitoring_attrs,
                );
                sleep(Duration::from_millis(100)).await;
            } else {
                return Ok(());
            }
        }
    }
}
