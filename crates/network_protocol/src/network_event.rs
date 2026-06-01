use std::io;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
    ChannelId, UserId, io_err_invalid_data,
    proto::{self, EventFrame, event_frame, received_message},
};

type ProtoReceiveDestination = received_message::Destination;

/// Details about where a chat message is sent to.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ReceiveDestination {
    /// Message is sent directly to the client.
    User(UserId),

    /// Message is sent to a channel with the given ID.
    Channel(ChannelId),
}

impl TryFrom<ProtoReceiveDestination> for ReceiveDestination {
    type Error = io::Error;

    fn try_from(value: ProtoReceiveDestination) -> Result<Self, Self::Error> {
        Ok(match value {
            ProtoReceiveDestination::UserId(id) => Self::User(id.try_into()?),
            ProtoReceiveDestination::ChannelId(id) => Self::Channel(id.try_into()?),
        })
    }
}

impl From<ReceiveDestination> for ProtoReceiveDestination {
    fn from(value: ReceiveDestination) -> Self {
        match value {
            ReceiveDestination::Channel(id) => Self::ChannelId(id.into()),
            ReceiveDestination::User(id) => Self::UserId(id.into()),
        }
    }
}

/// A message sent from some other client to either a specific user, or a whole channel.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ReceivedMessage {
    /// The message's content.
    pub contents: String,

    /// The sender's user ID.
    pub sender_id: UserId,

    /// The destination of the message.
    pub destination: ReceiveDestination,
}

impl TryFrom<proto::ReceivedMessage> for ReceivedMessage {
    type Error = io::Error;

    fn try_from(value: proto::ReceivedMessage) -> Result<Self, Self::Error> {
        let destination: ReceiveDestination = value
            .destination
            .ok_or_else(io_err_invalid_data)?
            .try_into()?;

        let sender_id: UserId = value
            .sender_id
            .ok_or_else(io_err_invalid_data)?
            .try_into()?;

        Ok(ReceivedMessage {
            contents: value.contents,
            sender_id,
            destination,
        })
    }
}

impl From<ReceivedMessage> for proto::ReceivedMessage {
    fn from(value: ReceivedMessage) -> Self {
        Self {
            contents: value.contents,
            sender_id: Some(value.sender_id.into()),
            destination: Some(value.destination.into()),
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ServerHello {
    pub your_id: UserId,
    pub default_channel_id: Option<ChannelId>,
}

impl TryFrom<proto::ServerHello> for ServerHello {
    type Error = io::Error;

    fn try_from(value: proto::ServerHello) -> Result<Self, Self::Error> {
        let your_id: UserId = value.your_id.ok_or_else(io_err_invalid_data)?.try_into()?;

        let default_channel_id = value
            .default_channel_id
            .map(TryInto::try_into)
            .transpose()?;

        Ok(Self {
            your_id,
            default_channel_id,
        })
    }
}

impl From<ServerHello> for proto::ServerHello {
    fn from(value: ServerHello) -> Self {
        Self {
            your_id: Some(value.your_id.into()),
            default_channel_id: value.default_channel_id.map(Into::into),
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ChannelSync {
    pub channel_ids: Vec<ChannelId>,
}

impl TryFrom<proto::ChannelSync> for ChannelSync {
    type Error = io::Error;

    fn try_from(value: proto::ChannelSync) -> Result<Self, Self::Error> {
        let channel_ids: Vec<ChannelId> = value
            .channel_ids
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;

        Ok(Self { channel_ids })
    }
}

impl From<ChannelSync> for proto::ChannelSync {
    fn from(value: ChannelSync) -> Self {
        let channel_ids: Vec<proto::ChannelId> =
            value.channel_ids.into_iter().map(Into::into).collect();

        Self { channel_ids }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct UserSync {
    pub user_ids: Vec<UserId>,
}

impl TryFrom<proto::UserSync> for UserSync {
    type Error = io::Error;

    fn try_from(value: proto::UserSync) -> Result<Self, Self::Error> {
        let user_ids: Vec<UserId> = value
            .user_ids
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;

        Ok(Self { user_ids })
    }
}

impl From<UserSync> for proto::UserSync {
    fn from(value: UserSync) -> Self {
        let user_ids: Vec<proto::UserId> = value.user_ids.into_iter().map(Into::into).collect();

        Self { user_ids }
    }
}

/// An event sent from the server to the client backend.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum NetworkEvent {
    /// Initial message to give the client session info and state.
    ServerHello(ServerHello),

    /// Message to sync information about channels on the server.
    ChannelSync(ChannelSync),

    /// Message to sync information about users on the server.
    UserSync(UserSync),

    /// A new user joined the server.
    UserJoined(UserId),

    /// A user left the server.
    UserLeft(UserId),

    /// Received a message from some other connected client.
    ReceivedMessage(ReceivedMessage),
}

impl TryFrom<EventFrame> for NetworkEvent {
    type Error = io::Error;

    fn try_from(value: EventFrame) -> Result<Self, Self::Error> {
        use event_frame::Variant;

        match value.variant.ok_or_else(io_err_invalid_data)? {
            Variant::ServerHello(hello) => Ok(Self::ServerHello(hello.try_into()?)),

            Variant::ChannelSync(channel_sync) => Ok(Self::ChannelSync(channel_sync.try_into()?)),

            Variant::UserSync(user_sync) => Ok(Self::UserSync(user_sync.try_into()?)),

            Variant::UserJoined(user_id) => Ok(Self::UserJoined(user_id.try_into()?)),

            Variant::UserLeft(user_id) => Ok(Self::UserLeft(user_id.try_into()?)),

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
            NetworkEvent::ServerHello(hello) => Self {
                variant: Some(Variant::ServerHello(hello.into())),
            },

            NetworkEvent::ChannelSync(channel_sync) => Self {
                variant: Some(Variant::ChannelSync(channel_sync.into())),
            },

            NetworkEvent::UserSync(user_sync) => Self {
                variant: Some(Variant::UserSync(user_sync.into())),
            },

            NetworkEvent::UserJoined(user_id) => Self {
                variant: Some(Variant::UserJoined(user_id.into())),
            },

            NetworkEvent::UserLeft(user_id) => Self {
                variant: Some(Variant::UserLeft(user_id.into())),
            },

            NetworkEvent::ReceivedMessage(message) => EventFrame {
                variant: Some(Variant::ReceivedMessage(message.into())),
            },
        }
    }
}
