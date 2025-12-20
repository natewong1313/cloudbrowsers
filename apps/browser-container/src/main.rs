use axum::{Router, routing::get};
use chromiumoxide::browser::{Browser, BrowserConfig};

use futures::StreamExt;

use crate::launcher::launch_browser;
pub mod launcher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut browser_details = launch_browser().await?;
    let mut browser = browser_details.browser;

    // spawn a new task that continuously polls the handler
    let handle = tokio::spawn(async move {
        while let Some(h) = browser_details.handler.next().await {
            if h.is_err() {
                break;
            }
        }
    });

    // // create a new browser page and navigate to the url
    // let page = browser.new_page("https://en.wikipedia.org").await?;
    //
    // // find the search bar type into the search field and hit `Enter`,
    // // this triggers a new navigation to the search result page
    // page.find_element("input#searchInput")
    //     .await?
    //     .click()
    //     .await?
    //     .type_str("Rust programming language")
    //     .await?
    //     .press_key("Enter")
    //     .await?;
    //
    // let html = page.wait_for_navigation().await?.content().await?;
    //
    // browser.close().await?;
    handle.await?;
    Ok(())
    // let app = Router::new().route("/", get(|| async { "Hello, World!" }));
    //
    // let listener = tokio::net::TcpListener::bind("0.0.0.0:6700").await.unwrap();
    // axum::serve(listener, app).await.unwrap();
}
