pub use network_protocol::ChatMessage;

use std::io;
use std::result::Result as StdResult;

use thiserror::Error;

use network_protocol::NetworkEvent;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error {0}")]
    Io(#[from] io::Error),
}

pub type Result = StdResult<ClientEvent, Error>;

#[derive(Debug)]
pub enum ClientEvent {
    Connected,
    Disconnected,
    ReceivedMessage(ChatMessage),
}

impl From<NetworkEvent> for ClientEvent {
    fn from(value: NetworkEvent) -> Self {
        match value {
            NetworkEvent::ReceivedMessage(m) => Self::ReceivedMessage(m),
        }
    }
}
