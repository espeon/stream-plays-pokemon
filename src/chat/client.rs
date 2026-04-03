use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use parking_lot::Mutex;
use serde::Deserialize;
use tokio::time::Instant;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::input::types::ChatMessage;
use crate::vote::engine::VoteEngine;
use crate::ViewerCountTracker;

const BACKFILL_DISCARD_MS: u64 = 1000;

#[derive(Debug, Deserialize)]
struct MessageView {
    #[serde(rename = "$type")]
    type_field: String,
    author: Author,
    record: Record,
}

#[derive(Debug, Deserialize)]
struct Author {
    handle: String,
}

#[derive(Debug, Deserialize)]
struct Record {
    text: String,
}

// {"$type":"place.stream.livestream#viewerCount","count":21}
#[derive(Debug, Deserialize)]
struct ViewerCount {
    #[serde(rename = "$type")]
    type_field: String,
    count: u32,
}

pub async fn run_chat_client(
    ws_url: String,
    engine: Arc<Mutex<VoteEngine>>,
    count_tracker: Arc<Mutex<ViewerCountTracker>>,
) {
    let mut backoff = Duration::from_secs(1);
    loop {
        match connect_and_run(&ws_url, Arc::clone(&engine), Arc::clone(&count_tracker)).await {
            Ok(()) => {
                tracing::info!("chat WS closed cleanly, reconnecting");
            }
            Err(e) => {
                tracing::warn!("chat WS error: {e}, reconnecting in {backoff:?}");
            }
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
}

async fn connect_and_run(
    ws_url: &str,
    engine: Arc<Mutex<VoteEngine>>,
    count_tracker: Arc<Mutex<ViewerCountTracker>>,
) -> Result<(), anyhow::Error> {
    tracing::info!("connecting to chat WS: {ws_url}");
    let (ws_stream, _) = connect_async(ws_url).await?;
    tracing::info!("chat WS connected");

    // Reset backoff on successful connection is handled by the caller.
    let connect_time = Instant::now();
    let (_, mut read) = ws_stream.split();

    while let Some(msg) = read.next().await {
        let msg = msg?;
        let text = match msg {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };

        // Discard backfill messages from the first second
        if connect_time.elapsed() < Duration::from_millis(BACKFILL_DISCARD_MS) {
            continue;
        }

        // let Ok(view) = serde_json::from_str::<MessageView>(&text) else {
        //     continue;
        // };

        // if view.type_field != "place.stream.chat.defs#messageView" {
        //     continue;
        // }

        // let chat_msg = ChatMessage {
        //     user: view.author.handle,
        //     text: view.record.text,
        //     ts: chrono::Utc::now().timestamp_millis(),
        // };

        // get message type
        let msg_type = match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(v) => {
                let v_cloned = v.clone();
                let fnl = v_cloned.get("$type").and_then(|t| t.as_str()).unwrap_or("");
                fnl.to_string()
            }
            Err(_) => continue,
        };

        match &msg_type.as_str() {
            &"place.stream.chat.defs#messageView" => {
                let view: MessageView = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let chat_msg = ChatMessage {
                    user: view.author.handle,
                    text: view.record.text,
                    ts: chrono::Utc::now().timestamp_millis(),
                };
                engine.lock().submit(chat_msg);
            }
            &"place.stream.livestream#viewerCount" => {
                let count: ViewerCount = match serde_json::from_str(&text) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                count_tracker.lock().update(count.count);
            }
            _ => continue,
        }
    }

    Ok(())
}
