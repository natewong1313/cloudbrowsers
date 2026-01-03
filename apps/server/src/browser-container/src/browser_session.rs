use anyhow::anyhow;
use chromiumoxide::BrowserConfig;
use futures::StreamExt;
use port_check::free_local_port;
use std::time::Duration;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind};
use tempfile::tempdir;
use tokio::{task::JoinHandle, time::sleep};
use uuid::Uuid;

pub struct BrowserSession {
    id: Uuid,
    browser: chromiumoxide::Browser,
    browser_proc_pid: u32,
    poller_handle: JoinHandle<()>,
    watchdog_handle: JoinHandle<()>,
}
impl BrowserSession {
    pub async fn launch() -> anyhow::Result<Self> {
        tracing::info!("Starting browser session launch...");
        
        let id = Uuid::new_v4();
        let tmp_dir = tempdir()?;

        tracing::debug!("using directory at {}", tmp_dir.path().to_string_lossy());

        let Some(free_port) = free_local_port() else {
            return Err(anyhow!("Could not get a free local port"));
        };
        tracing::debug!("Launching browser @ 127.0.0.1:{}", free_port);
        let port_arg = format!("--remote-debugging-port={}", free_port);

        tracing::info!("Building browser configuration...");
        let config = match BrowserConfig::builder()
            .with_head()
            // .new_headless_mode()
            .user_data_dir(tmp_dir.path())
            .arg(port_arg)
            .arg("--no-sandbox")
            .arg("--disable-setuid-sandbox")
            .arg("--disable-dev-shm-usage")
            .arg("--disable-gpu")
            .arg("--single-process")
            .build()
        {
            Ok(config) => config,
            // it returns an error as a string -_-
            Err(err) => return Err(anyhow!("Unknown config error: {}", err)),
        };
        
        tracing::info!("Launching Chrome browser...");
        let (mut browser, handler) = match chromiumoxide::Browser::launch(config).await {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("Failed to launch Chrome browser: {}", e);
                return Err(anyhow!("Chrome launch failed: {}", e));
            }
        };

        let child_proc = match browser.get_mut_child() {
            Some(proc) => proc.as_mut_inner(),
            None => return Err(anyhow!("Unexpected error getting browser process")),
        };
        let pid = match child_proc.id() {
            Some(pid) => pid,
            None => return Err(anyhow!("Unexpected error getting browser pid")),
        };

        // Start the poller
        let poller_handle = tokio::spawn(async move {
            if let Err(err) = poll_browser_handler(handler).await {
                tracing::warn!("{}", err);
            };
        });
        // Start the watchdog
        let watchdog_handle = tokio::spawn(async move {
            if let Err(err) = start_watchdog(pid).await {
                tracing::warn!("{}", err);
            };
        });

        Ok(Self {
            id,
            browser,
            browser_proc_pid: pid,
            poller_handle,
            watchdog_handle,
        })
    }

    pub async fn cleanup(&mut self) {
        tracing::debug!("cleaning up session");

        self.poller_handle.abort();
        self.watchdog_handle.abort();

        match self.browser.close().await {
            Ok(_) => tracing::debug!("browser closed successfully"),
            Err(e) => tracing::warn!("unexpected error closing browser: {}", e),
        }

        let s = sysinfo::System::new_all();
        if let Some(p) = s.process(sysinfo::Pid::from_u32(self.browser_proc_pid)) {
            p.kill();
        };
    }

    pub fn ws_addr(&mut self) -> &String {
        return self.browser.websocket_address();
    }
}

async fn poll_browser_handler(mut handler: chromiumoxide::Handler) -> anyhow::Result<()> {
    while let Some(event) = handler.next().await {
        if let Err(err) = event {
            return Err(anyhow!("unexpected browser handler error: {}", err));
        }
    }
    Ok(())
}

async fn start_watchdog(browser_proc_pid: u32) -> anyhow::Result<()> {
    let proc_kind = ProcessRefreshKind::nothing().with_memory().with_cpu();

    let mut sys_info =
        sysinfo::System::new_with_specifics(RefreshKind::nothing().with_processes(proc_kind));
    let pid = sysinfo::Pid::from_u32(browser_proc_pid);
    loop {
        if let Some(p) = sys_info.process(pid)
            && p.exists()
        {
            let mem = p.memory() as f64 / 1_000_000.0;
            let cpu = p.cpu_usage();
            println!("memory: {}mb | cpu: {}%", mem, cpu);
            sys_info.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, proc_kind);
            sleep(Duration::from_millis(100)).await;
        } else {
            tracing::debug!("Process no longer exists");
            return Ok(());
        }
    }
}
