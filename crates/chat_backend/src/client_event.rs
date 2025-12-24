use std::io;

use thiserror::Error;

use protocol::NetworkEvent;

#[derive(Debug, Error)]
pub enum EventError {
    #[error("I/O error {0}")]
    Io(#[from] io::Error),
}

pub type EventResult = Result<ClientEvent, EventError>;

#[derive(Debug)]
pub enum ClientEvent {
    Connected,
    Disconnected,
    ReceivedMessage(String),
}

impl From<NetworkEvent> for ClientEvent {
    fn from(value: NetworkEvent) -> Self {
        match value {
            NetworkEvent::ReceivedMessage(m) => Self::ReceivedMessage(m),
        }
    }
}
