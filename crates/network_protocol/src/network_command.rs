use std::io;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
    ChannelId, UserId, io_err_invalid_data,
    proto::{self, CommandFrame, command_frame, send_message},
};

type ProtoSendDestination = send_message::Destination;

/// Where to send a chat message.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum SendDestination {
    /// Send to a channel with the given ID.
    Channel(ChannelId),

    /// Send to a user with the given ID.
    User(UserId),
}

impl TryFrom<ProtoSendDestination> for SendDestination {
    type Error = io::Error;

    fn try_from(value: ProtoSendDestination) -> Result<Self, Self::Error> {
        Ok(match value {
            ProtoSendDestination::ChannelId(id) => Self::Channel(id.try_into()?),
            ProtoSendDestination::UserId(id) => Self::User(id.try_into()?),
        })
    }
}

impl From<SendDestination> for ProtoSendDestination {
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

impl TryFrom<proto::SendMessage> for SendMessage {
    type Error = io::Error;

    fn try_from(value: proto::SendMessage) -> Result<Self, Self::Error> {
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

impl From<SendMessage> for proto::SendMessage {
    fn from(value: SendMessage) -> Self {
        Self {
            contents: value.contents,
            destination: Some(value.destination.into()),
        }
    }
}

/// First message from the client to the server, indicating a desire to connect and requesting the
/// given username.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ClientHello {
    pub requested_name: String,
}

impl TryFrom<proto::ClientHello> for ClientHello {
    type Error = io::Error;

    fn try_from(value: proto::ClientHello) -> Result<Self, Self::Error> {
        Ok(Self {
            requested_name: value.requested_name,
        })
    }
}

impl From<ClientHello> for proto::ClientHello {
    fn from(value: ClientHello) -> Self {
        Self {
            requested_name: value.requested_name,
        }
    }
}

/// A command sent from the client backend to the server.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum NetworkCommand {
    /// Initial message to the server, requesting to connect.
    ClientHello(ClientHello),

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
            Variant::ClientHello(hello) => Ok(NetworkCommand::ClientHello(hello.try_into()?)),

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
            NetworkCommand::ClientHello(hello) => CommandFrame {
                variant: Some(Variant::ClientHello(hello.into())),
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
