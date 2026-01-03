use anyhow::Ok;
use flume::Receiver;
use tokio::sync::mpsc;

use crate::browser_session::BrowserSession;

const MAX_BROWSERS: u32 = 2;

pub async fn start(rx: mpsc::Receiver<u32>) {}

// pub struct BrowserScheduler {
//     current_size: u32,
//     max_size: u32,
// }
//
// impl BrowserScheduler {
//     pub async fn start(rx: mpsc::Sender<u32>) -> anyhow::Result<Self> {
//         let handle = tokio::spawn(async move {
//             BrowserSession::launch().await;
//         });
//         handle.await;
//         // let launch_tasks = (1..=MAX_BROWSERS).map(|_| BrowserSession::launch());
//         // let browsers = future::join_all(launch_tasks).await;
//
//         Ok(Self {
//             current_size: MAX_BROWSERS,
//             max_size: MAX_BROWSERS,
//         })
//     }
// }

