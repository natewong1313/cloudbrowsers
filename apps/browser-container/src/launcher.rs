use anyhow::anyhow;
use chromiumoxide::{Browser, BrowserConfig, Handler};
use port_check::free_local_port;

pub struct BrowserInstance {
    pub browser: Browser,
    pub handler: Handler,
}

// Handles launching a new browser on a free port
pub async fn launch_browser() -> anyhow::Result<BrowserInstance, anyhow::Error> {
    // TODO: check if this works properly in prod
    let Some(free_port) = free_local_port() else {
        return Err(anyhow!("Could not get a free local port"));
    };
    let port_arg = format!("--remote-debugging-port={}", free_port);
    println!("launching browser @ 127.0.0.1:{}", free_port);

    // Creates a new headed browser with a custom port
    // TODO: toggle headed / headless based on environment
    let config = match BrowserConfig::builder().with_head().arg(port_arg).build() {
        Ok(config) => config,
        // it returns an error as a string -_-
        Err(err) => return Err(anyhow!("Unknown config error: {}", err)),
    };

    // Browser controls the actual chromium browser and handler is for the websocket stuff
    // (mut browser, mut handler)
    let browser_details = Browser::launch(config).await?;
    let browser_instance = BrowserInstance {
        browser: browser_details.0,
        handler: browser_details.1,
    };

    let ws_addr = browser_instance.browser.websocket_address();
    println!("browser reachable @ {}", ws_addr);

    Ok(browser_instance)
}
