use std::io;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
    ChannelId, UserId, io_err_invalid_data,
    proto::{
        self, ClientHello, CommandFrame, SendMessage as ProtoSendMessage, command_frame,
        send_message,
    },
};

type ProtoDestinationFrame = send_message::Destination;

/// Where to send a chat message.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum SendDestination {
    /// Send to a channel with the given ID.
    Channel(ChannelId),

    /// Send to a user with the given ID.
    User(UserId),
}

impl TryFrom<ProtoDestinationFrame> for SendDestination {
    type Error = io::Error;

    fn try_from(value: ProtoDestinationFrame) -> Result<Self, Self::Error> {
        Ok(match value {
            ProtoDestinationFrame::ChannelId(id) => Self::Channel(id.try_into()?),
            ProtoDestinationFrame::UserId(id) => Self::User(id.try_into()?),
        })
    }
}

impl From<SendDestination> for ProtoDestinationFrame {
    fn from(value: SendDestination) -> Self {
        match value {
            SendDestination::Channel(id) => Self::ChannelId(id.into()),
            SendDestination::User(id) => Self::UserId(id.into()),
        }
    }
}

/// A request to fetch the server's channel list in bulk.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FetchChannels;

impl TryFrom<proto::FetchChannels> for FetchChannels {
    type Error = io::Error;

    fn try_from(_: proto::FetchChannels) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl From<FetchChannels> for proto::FetchChannels {
    fn from(_: FetchChannels) -> Self {
        Self { empty: Some(()) }
    }
}

/// A request to fetch the server's user list in bulk.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FetchUsers;

impl TryFrom<proto::FetchUsers> for FetchUsers {
    type Error = io::Error;

    fn try_from(_: proto::FetchUsers) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl From<FetchUsers> for proto::FetchUsers {
    fn from(_: FetchUsers) -> Self {
        Self { empty: Some(()) }
    }
}

/// A chat message sent to the server.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SendMessage {
    /// The message's content.
    pub contents: String,

    /// The target of the message. May either be a direct user or a channel.
    pub destination: SendDestination,
}

impl TryFrom<ProtoSendMessage> for SendMessage {
    type Error = io::Error;

    fn try_from(value: ProtoSendMessage) -> Result<Self, Self::Error> {
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

impl From<SendMessage> for ProtoSendMessage {
    fn from(value: SendMessage) -> Self {
        Self {
            contents: value.contents,
            destination: Some(value.destination.into()),
        }
    }
}

/// A command sent from the client backend to the server.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum NetworkCommand {
    /// Initial message to the server, requesting to connect.
    ClientHello,

    /// Fetch the server's channel list in bulk.
    FetchChannels(FetchChannels),

    /// Fetch the server's user list in bulk.
    FetchUsers(FetchUsers),

    /// Send the given message.
    SendMessage(SendMessage),
}

impl TryFrom<CommandFrame> for NetworkCommand {
    type Error = io::Error;

    fn try_from(value: CommandFrame) -> Result<Self, Self::Error> {
        use command_frame::Variant;

        match value.variant.ok_or_else(io_err_invalid_data)? {
            Variant::ClientHello(ClientHello { hello: _ }) => Ok(NetworkCommand::ClientHello),

            Variant::FetchChannels(fetch) => Ok(NetworkCommand::FetchChannels(fetch.try_into()?)),

            Variant::FetchUsers(fetch) => Ok(NetworkCommand::FetchUsers(fetch.try_into()?)),

            Variant::SendMessage(message) => Ok(NetworkCommand::SendMessage(message.try_into()?)),
        }
    }
}

impl From<NetworkCommand> for CommandFrame {
    fn from(value: NetworkCommand) -> Self {
        use command_frame::Variant;

        match value {
            NetworkCommand::ClientHello => CommandFrame {
                variant: Some(Variant::ClientHello(ClientHello { hello: Some(()) })),
            },

            NetworkCommand::FetchChannels(fetch) => CommandFrame {
                variant: Some(Variant::FetchChannels(fetch.into())),
            },

            NetworkCommand::FetchUsers(fetch) => CommandFrame {
                variant: Some(Variant::FetchUsers(fetch.into())),
            },

            NetworkCommand::SendMessage(message) => CommandFrame {
                variant: Some(Variant::SendMessage(message.into())),
            },
        }
    }
}
