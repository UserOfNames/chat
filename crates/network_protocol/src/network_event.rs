use std::io;

use crate::protobuf_items::{ChatMessageFrame, EventFrame, event_frame};

/// A message sent from a client to all other clients connected to the same server.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// The actual message.
    pub contents: String,
    /// The sender's nickname.
    pub sender: String,
}

impl From<ChatMessageFrame> for ChatMessage {
    fn from(value: ChatMessageFrame) -> Self {
        ChatMessage {
            contents: value.contents,
            sender: value.sender,
        }
    }
}

impl From<ChatMessage> for ChatMessageFrame {
    fn from(value: ChatMessage) -> Self {
        ChatMessageFrame {
            contents: value.contents,
            sender: value.sender,
        }
    }
}

/// An event sent from the server to the client backend.
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// Received a message from some other connected client.
    ReceivedMessage(ChatMessage),
}

impl TryFrom<EventFrame> for NetworkEvent {
    type Error = io::Error;

    fn try_from(value: EventFrame) -> Result<Self, Self::Error> {
        match value.variant {
            Some(event_frame::Variant::ReceivedMessage(message)) => {
                Ok(NetworkEvent::ReceivedMessage(message.into()))
            }

            _ => Err(io::Error::from(io::ErrorKind::InvalidData)),
        }
    }
}

impl From<NetworkEvent> for EventFrame {
    fn from(value: NetworkEvent) -> Self {
        match value {
            NetworkEvent::ReceivedMessage(message) => EventFrame {
                variant: Some(event_frame::Variant::ReceivedMessage(message.into())),
            },
        }
    }
}
