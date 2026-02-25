use axum::{
    extract::{ws::Message, ws::WebSocket, State, WebSocketUpgrade},
    response::Response,
};
use tokio::sync::broadcast;

use crate::types::BroadcastMessage;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(broadcast_tx): State<broadcast::Sender<BroadcastMessage>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, broadcast_tx))
}

async fn handle_socket(mut socket: WebSocket, broadcast_tx: broadcast::Sender<BroadcastMessage>) {
    let mut rx = broadcast_tx.subscribe();

    loop {
        match rx.recv().await {
            Ok(msg) => {
                let framed = frame_message(&msg);
                if socket.send(framed).await.is_err() {
                    break; // client disconnected
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!("ws client lagged, dropped {n} messages");
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
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
}
