use std::io;

use crate::protobuf_items::{EventFrame, event_frame};

#[derive(Debug, Clone)]
pub enum NetworkEvent {
    ReceivedMessage(String),
}

impl TryFrom<EventFrame> for NetworkEvent {
    type Error = io::Error;

    fn try_from(value: EventFrame) -> Result<Self, Self::Error> {
        match value.variant {
            Some(event_frame::Variant::ReceivedMessage(message)) => {
                Ok(NetworkEvent::ReceivedMessage(message))
            }

            _ => Err(io::Error::from(io::ErrorKind::InvalidData)),
        }
    }
}

impl From<NetworkEvent> for EventFrame {
    fn from(value: NetworkEvent) -> Self {
        match value {
            NetworkEvent::ReceivedMessage(message) => EventFrame {
                variant: Some(event_frame::Variant::ReceivedMessage(message)),
            },
        }
    }
}
