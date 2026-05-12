use std::io;

use crate::{
    ChannelId, UserId,
    protobuf_items::{CommandFrame, SendMessageFrame, command_frame, send_message_frame},
};

pub type SendDestinationFrame = send_message_frame::Destination;

/// Where to send a chat message.
#[derive(Debug, Clone)]
pub enum SendDestination {
    /// Send to a channel with the given ID.
    Channel(ChannelId),

    /// Send to a user with the given ID.
    User(UserId),
}

impl TryFrom<SendDestinationFrame> for SendDestination {
    type Error = io::Error;

    fn try_from(value: SendDestinationFrame) -> Result<Self, Self::Error> {
        Ok(match value {
            SendDestinationFrame::ChannelId(id) => Self::Channel(id),
            SendDestinationFrame::UserId(id) => Self::User(id),
        })
    }
}

impl From<SendDestination> for SendDestinationFrame {
    fn from(value: SendDestination) -> Self {
        match value {
            SendDestination::Channel(id) => Self::ChannelId(id),
            SendDestination::User(id) => Self::UserId(id),
        }
    }
}

/// A chat message sent to the server.
#[derive(Debug, Clone)]
pub struct SendMessage {
    /// The message's content.
    pub contents: String,

    /// The target of the message. May either be a direct user or a channel.
    pub destination: SendDestination,
}

impl TryFrom<SendMessageFrame> for SendMessage {
    type Error = io::Error;

    fn try_from(value: SendMessageFrame) -> Result<Self, Self::Error> {
        let destination: SendDestination = value
            .destination
            .ok_or_else(io_err_invalid_data)?
            .try_into()?;

        Ok(Self {
            contents: value.contents,
            destination,
        })
    }
}

impl From<SendMessage> for SendMessageFrame {
    fn from(value: SendMessage) -> Self {
        Self {
            contents: value.contents,
            destination: Some(value.destination.into()),
        }
    }
}

/// A command sent from the client backend to the server.
#[derive(Debug, Clone)]
pub enum NetworkCommand {
    /// Send the given message.
    SendMessage(SendMessage),

    /// Join a channel with the given ID.
    JoinChannel(ChannelId),
}

impl TryFrom<CommandFrame> for NetworkCommand {
    type Error = io::Error;

    fn try_from(value: CommandFrame) -> Result<Self, Self::Error> {
        use command_frame::Variant as Variant;

        match value.variant.ok_or_else(io_err_invalid_data)? {
            Variant::SendMessage(message) => {
                Ok(NetworkCommand::SendMessage(message.try_into()?))
            }

            Variant::JoinChannel(channel) => {
                Ok(NetworkCommand::JoinChannel(channel))
            }
        }
    }
}

impl From<NetworkCommand> for CommandFrame {
    fn from(value: NetworkCommand) -> Self {
        use command_frame::Variant as Variant;

        match value {
            NetworkCommand::SendMessage(message) => CommandFrame {
                variant: Some(Variant::SendMessage(message.into())),
            },

            NetworkCommand::JoinChannel(channel) => CommandFrame {
                variant: Some(Variant::JoinChannel(channel))
            },
        }
    }
}

fn io_err_invalid_data() -> io::Error {
    io::Error::from(io::ErrorKind::InvalidData)
}
