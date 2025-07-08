use std::env;
use std::time::Duration;

use reqwest::Client;
use reqwest_eventsource::{Event, EventSource};
use futures_util::StreamExt;
use tokio::time::sleep;

use tracing::{info, warn};

use crate::server::Change;

/// Connect to the server and listen for change events via SSE.
///
/// The server URL is taken from the `HIT_SERVER_URL` environment variable,
/// defaulting to `http://localhost:8888` if unset.
///
/// On each incoming event a log line is printed. The function retries with
/// exponential backoff if the connection drops and exits cleanly on `Ctrl+C`.
pub async fn sync_from_server() {
    let base = env::var("HIT_SERVER_URL").unwrap_or_else(|_| "http://localhost:8888".into());
    let url = format!("{}/events", base.trim_end_matches('/'));

    let client = Client::new();
    let mut backoff = 1u64;

    loop {
        info!(url = %url, "connecting");
        let request = client.get(&url);
        match EventSource::new(request) {
            Ok(mut source) => {
                let mut shutdown = Box::pin(tokio::signal::ctrl_c());
                loop {
                    tokio::select! {
                        _ = &mut shutdown => {
                            let _ = source.close();
                            return;
                        }
                        message = source.next() => match message {
                            Some(Ok(Event::Open)) => {
                                backoff = 1;
                                info!("connected");
                            }
                            Some(Ok(Event::Message(msg))) => {
                                match serde_json::from_str::<Change>(&msg.data) {
                                    Ok(change) => {
                                        info!(hash = %change.hash, path = %change.path, "Would apply change");
                                    }
                                    Err(e) => warn!(%e, "failed to parse event"),
                                }
                            }
                            Some(Err(e)) => {
                                warn!(%e, "stream error");
                                break;
                            }
                            None => {
                                warn!("server closed connection");
                                break;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                warn!(%e, "failed to connect");
            }
        }

        let delay = backoff.min(30);
        info!(delay, "reconnecting");
        sleep(Duration::from_secs(delay)).await;
        backoff = (backoff * 2).min(30);
    }
}
