use std::io;

use crate::{
    ChannelId, UserId,
    protobuf_items::{EventFrame, ReceiveMessageFrame, event_frame, receive_message_frame},
};

pub type ReceiveDestinationFrame = receive_message_frame::Destination;

/// Details about where a chat message is sent to.
#[derive(Debug, Clone)]
pub enum ReceiveDestination {
    /// Message is sent directly to the client.
    Direct,

    /// Message is sent to a channel with the given ID.
    Channel(ChannelId),
}

impl TryFrom<ReceiveDestinationFrame> for ReceiveDestination {
    type Error = io::Error;

    fn try_from(value: ReceiveDestinationFrame) -> Result<Self, Self::Error> {
        Ok(match value {
            ReceiveDestinationFrame::IsDirect(()) => Self::Direct,
            ReceiveDestinationFrame::ChannelId(id) => Self::Channel(id),
        })
    }
}

impl From<ReceiveDestination> for ReceiveDestinationFrame {
    fn from(value: ReceiveDestination) -> Self {
        match value {
            ReceiveDestination::Channel(id) => Self::ChannelId(id),
            ReceiveDestination::Direct => Self::IsDirect(()),
        }
    }
}

/// A message sent from some other client to either a specific user, or a whole channel.
#[derive(Debug, Clone)]
pub struct ReceiveMessage {
    /// The message's content.
    pub contents: String,

    /// The sender's user ID.
    pub sender_id: UserId,

    /// The destination of the message.
    pub destination: ReceiveDestination,
}

impl TryFrom<ReceiveMessageFrame> for ReceiveMessage {
    type Error = io::Error;

    fn try_from(value: ReceiveMessageFrame) -> Result<Self, Self::Error> {
        let destination: ReceiveDestination = value
            .destination
            .ok_or_else(io_err_invalid_data)?
            .try_into()?;

        Ok(ReceiveMessage {
            contents: value.contents,
            sender_id: value.sender_id,
            destination,
        })
    }
}

impl From<ReceiveMessage> for ReceiveMessageFrame {
    fn from(value: ReceiveMessage) -> Self {
        Self {
            contents: value.contents,
            sender_id: value.sender_id,
            destination: Some(value.destination.into()),
        }
    }
}

/// An event sent from the server to the client backend.
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// Received a message from some other connected client.
    ReceivedMessage(ReceiveMessage),
}

impl TryFrom<EventFrame> for NetworkEvent {
    type Error = io::Error;

    fn try_from(value: EventFrame) -> Result<Self, Self::Error> {
        use event_frame::Variant;

        match value.variant.ok_or_else(io_err_invalid_data)? {
            Variant::ReceivedMessage(message) => {
                Ok(NetworkEvent::ReceivedMessage(message.try_into()?))
            }
        }
    }
}

impl From<NetworkEvent> for EventFrame {
    fn from(value: NetworkEvent) -> Self {
        use event_frame::Variant;

        match value {
            NetworkEvent::ReceivedMessage(message) => EventFrame {
                variant: Some(Variant::ReceivedMessage(message.into())),
            },
        }
    }
}

fn io_err_invalid_data() -> io::Error {
    io::Error::from(io::ErrorKind::InvalidData)
}
