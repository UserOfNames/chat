pub use network_protocol::ChatMessage;

use std::io;
use std::result::Result as StdResult;

use thiserror::Error;

use network_protocol::NetworkEvent;

/// An error arising in the client backend while processing a `ClientCommand`.
#[derive(Debug, Error)]
pub enum Error {
    /// An I/O error occurred while handling the command. This most commonly indicates an error
    /// while attempting to communicate with the server.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

/// A specialized `Result` type for carrying `ClientEvent`s to the frontend.
pub type Result = StdResult<ClientEvent, Error>;

/// An event from the client backend to the UI.
#[derive(Debug)]
pub enum ClientEvent {
    /// Successfully connected to the server with address `String`.
    Connected(String),
    /// Server disconnected.
    Disconnected,
    /// Received a message sent by another client.
    ReceivedMessage(ChatMessage),
}

impl From<NetworkEvent> for ClientEvent {
    fn from(value: NetworkEvent) -> Self {
        match value {
            NetworkEvent::ReceivedMessage(m) => Self::ReceivedMessage(m),
        }
    }
}
