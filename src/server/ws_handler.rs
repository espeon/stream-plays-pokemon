use std::{collections::HashMap, sync::{atomic::{AtomicU16, Ordering}, Arc}};

use axum::{
    extract::{ws::Message, ws::WebSocket, Query, State, WebSocketUpgrade},
    response::Response,
};
use tokio::sync::broadcast;

use crate::{emulator::KEYINPUT_ALL_RELEASED, types::BroadcastMessage};

#[derive(Clone)]
pub struct WsState {
    pub broadcast_tx: broadcast::Sender<BroadcastMessage>,
    pub overlay_keys: Arc<AtomicU16>,
    pub admin_token: String,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<WsState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let is_overlay = params
        .get("token")
        .map(|t| t == &state.admin_token)
        .unwrap_or(false);
    ws.on_upgrade(move |socket| {
        handle_socket(socket, state.broadcast_tx, state.overlay_keys, is_overlay)
    })
}

async fn handle_socket(
    mut socket: WebSocket,
    broadcast_tx: broadcast::Sender<BroadcastMessage>,
    overlay_keys: Arc<AtomicU16>,
    is_overlay: bool,
) {
    let mut rx = broadcast_tx.subscribe();

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(msg) => {
                        let framed = frame_message(&msg);
                        if socket.send(framed).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("ws client lagged, dropped {n} messages");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Binary(data))) if is_overlay => {
                        handle_overlay_input(&data, &overlay_keys);
                    }
                    None | Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }

    if is_overlay {
        overlay_keys.store(KEYINPUT_ALL_RELEASED, Ordering::Relaxed);
    }
}

fn handle_overlay_input(data: &[u8], overlay_keys: &Arc<AtomicU16>) {
    if data.len() < 2 {
        return;
    }
    let tag = data[0];
    let button_id = data[1];
    if button_id > 9 {
        return;
    }
    let bit = 1u16 << button_id;
    match tag {
        0x06 => {
            overlay_keys.fetch_and(!bit, Ordering::Relaxed);
        }
        0x07 => {
            overlay_keys.fetch_or(bit, Ordering::Relaxed);
        }
        _ => {}
    }
}

fn frame_message(msg: &BroadcastMessage) -> Message {
    let bytes = match msg {
        BroadcastMessage::Frame(data) => prefix_bytes(0x01, data),
        BroadcastMessage::Audio(data) => prefix_bytes(0x02, data),
        BroadcastMessage::State(data) => prefix_bytes(0x03, data),
        BroadcastMessage::Party(data) => prefix_bytes(0x04, data),
        BroadcastMessage::Location(data) => prefix_bytes(0x05, data),
    };
    Message::Binary(bytes.into())
}

fn prefix_bytes(prefix: u8, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + data.len());
    out.push(prefix);
    out.extend_from_slice(data);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_message_prefixes_frame() {
        let msg = BroadcastMessage::Frame(vec![0xAA, 0xBB]);
        let framed = frame_message(&msg);
        if let Message::Binary(b) = framed {
            assert_eq!(b[0], 0x01);
            assert_eq!(&b[1..], &[0xAA, 0xBB]);
        } else {
            panic!("expected Binary message");
        }
    }

    #[test]
    fn test_frame_message_prefixes_audio() {
        let msg = BroadcastMessage::Audio(vec![0x01, 0x02, 0x03]);
        let framed = frame_message(&msg);
        if let Message::Binary(b) = framed {
            assert_eq!(b[0], 0x02);
            assert_eq!(&b[1..], &[0x01, 0x02, 0x03]);
        } else {
            panic!("expected Binary message");
        }
    }

    #[test]
    fn test_frame_message_prefixes_state() {
        let msg = BroadcastMessage::State(b"{}".to_vec());
        let framed = frame_message(&msg);
        if let Message::Binary(b) = framed {
            assert_eq!(b[0], 0x03);
            assert_eq!(&b[1..], b"{}");
        } else {
            panic!("expected Binary message");
        }
    }

    #[test]
    fn test_frame_message_empty_payload() {
        let msg = BroadcastMessage::Frame(vec![]);
        let framed = frame_message(&msg);
        if let Message::Binary(b) = framed {
            assert_eq!(b.len(), 1);
            assert_eq!(b[0], 0x01);
        } else {
            panic!("expected Binary message");
        }
    }

    #[test]
    fn test_frame_message_prefixes_location() {
        let msg = BroadcastMessage::Location(b"{\"map_bank\":0}".to_vec());
        let framed = frame_message(&msg);
        if let Message::Binary(b) = framed {
            assert_eq!(b[0], 0x05);
            assert_eq!(&b[1..], b"{\"map_bank\":0}");
        } else {
            panic!("expected Binary message");
        }
    }

    #[test]
    fn test_handle_overlay_input_button_down() {
        let keys = Arc::new(AtomicU16::new(KEYINPUT_ALL_RELEASED));
        handle_overlay_input(&[0x06, 0], &keys); // A button down
        assert_eq!(keys.load(Ordering::Relaxed) & 1, 0); // bit 0 cleared
    }

    #[test]
    fn test_handle_overlay_input_button_up() {
        let keys = Arc::new(AtomicU16::new(0u16)); // all pressed
        handle_overlay_input(&[0x07, 0], &keys); // A button up
        assert_eq!(keys.load(Ordering::Relaxed) & 1, 1); // bit 0 set
    }

    #[test]
    fn test_handle_overlay_input_ignores_invalid_button() {
        let keys = Arc::new(AtomicU16::new(KEYINPUT_ALL_RELEASED));
        handle_overlay_input(&[0x06, 10], &keys); // button_id 10 is out of range
        assert_eq!(keys.load(Ordering::Relaxed), KEYINPUT_ALL_RELEASED);
    }

    #[test]
    fn test_handle_overlay_input_ignores_short_data() {
        let keys = Arc::new(AtomicU16::new(KEYINPUT_ALL_RELEASED));
        handle_overlay_input(&[0x06], &keys);
        assert_eq!(keys.load(Ordering::Relaxed), KEYINPUT_ALL_RELEASED);
    }
}
