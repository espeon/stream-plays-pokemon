use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use serde::Deserialize;
use tokio::time::Instant;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::StreamExt;

use crate::input::types::ChatMessage;
use crate::vote::engine::VoteEngine;

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

pub async fn run_chat_client(ws_url: String, engine: Arc<Mutex<VoteEngine>>) {
    let mut backoff = Duration::from_secs(1);
    loop {
        match connect_and_run(&ws_url, Arc::clone(&engine)).await {
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

        let Ok(view) = serde_json::from_str::<MessageView>(&text) else {
            continue;
        };

        if view.type_field != "place.stream.chat.defs#messageView" {
            continue;
        }

        let chat_msg = ChatMessage {
            user: view.author.handle,
            text: view.record.text,
            ts: chrono::Utc::now().timestamp_millis(),
        };

        engine.lock().submit(chat_msg);
    }

    Ok(())
}
