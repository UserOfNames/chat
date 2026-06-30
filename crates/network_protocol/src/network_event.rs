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
pub struct ChannelInfo {
    pub id: ChannelId,
    pub name: String,
}

impl TryFrom<proto::ChannelInfo> for ChannelInfo {
    type Error = io::Error;

    fn try_from(value: proto::ChannelInfo) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id.try_into()?,
            name: value.name,
        })
    }
}

impl From<ChannelInfo> for proto::ChannelInfo {
    fn from(value: ChannelInfo) -> Self {
        Self {
            id: value.id.into(),
            name: value.name,
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct UserInfo {
    pub id: UserId,
    pub name: String,
}

impl TryFrom<proto::UserInfo> for UserInfo {
    type Error = io::Error;

    fn try_from(value: proto::UserInfo) -> Result<Self, Self::Error> {
        let id = value.id.ok_or_else(io_err_invalid_data)?.try_into()?;

        Ok(Self {
            id,
            name: value.name,
        })
    }
}

impl From<UserInfo> for proto::UserInfo {
    fn from(value: UserInfo) -> Self {
        Self {
            id: Some(value.id.into()),
            name: value.name,
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ChannelSync {
    pub channels: Vec<ChannelInfo>,
}

impl TryFrom<proto::ChannelSync> for ChannelSync {
    type Error = io::Error;

    fn try_from(value: proto::ChannelSync) -> Result<Self, Self::Error> {
        let channels: Vec<ChannelInfo> = value
            .channels
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;

        Ok(Self { channels })
    }
}

impl From<ChannelSync> for proto::ChannelSync {
    fn from(value: ChannelSync) -> Self {
        let channels: Vec<proto::ChannelInfo> =
            value.channels.into_iter().map(Into::into).collect();

        Self { channels }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct UserSync {
    pub users: Vec<UserInfo>,
}

impl TryFrom<proto::UserSync> for UserSync {
    type Error = io::Error;

    fn try_from(value: proto::UserSync) -> Result<Self, Self::Error> {
        let users: Vec<UserInfo> = value
            .users
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;

        Ok(Self { users })
    }
}

impl From<UserSync> for proto::UserSync {
    fn from(value: UserSync) -> Self {
        let users: Vec<proto::UserInfo> = value.users.into_iter().map(Into::into).collect();

        Self { users }
    }
}

/// The type of `ErrorEvent` that occurred.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[non_exhaustive]
pub enum ErrorKind {
    Unknown,
    NameTaken,
    InvalidName,
}

impl TryFrom<i32> for ErrorKind {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Unknown),
            1 => Ok(Self::NameTaken),
            2 => Ok(Self::InvalidName),
            _ => Err(()),
        }
    }
}

impl From<ErrorKind> for i32 {
    fn from(value: ErrorKind) -> Self {
        match value {
            ErrorKind::Unknown => 0,
            ErrorKind::NameTaken => 1,
            ErrorKind::InvalidName => 2,
        }
    }
}

/// An event indicating an error occured on the server.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ErrorEvent {
    pub kind: ErrorKind,
    pub message: String,
}

impl TryFrom<proto::ErrorEvent> for ErrorEvent {
    type Error = io::Error;

    fn try_from(value: proto::ErrorEvent) -> Result<Self, Self::Error> {
        let kind: ErrorKind = value.code.try_into().map_err(|()| io_err_invalid_data())?;

        Ok(Self {
            kind,
            message: value.message,
        })
    }
}

impl From<ErrorEvent> for proto::ErrorEvent {
    fn from(value: ErrorEvent) -> Self {
        Self {
            code: value.kind.into(),
            message: value.message,
        }
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
    UserJoined(UserInfo),

    /// A user left the server.
    UserLeft(UserId),

    /// A user's information changed.
    UserInfoUpdated(UserInfo),

    /// Received a message from some other connected client.
    ReceivedMessage(ReceivedMessage),

    ErrorEvent(ErrorEvent),
}

impl NetworkEvent {
    /// Returns the name of the active variant as an `&'static str`.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::ServerHello(_) => "ServerHello",
            Self::ChannelSync(_) => "ChannelSync",
            Self::UserSync(_) => "UserSync",
            Self::UserJoined(_) => "UserJoined",
            Self::UserLeft(_) => "UserLeft",
            Self::UserInfoUpdated(_) => "UserInfoUpdated",
            Self::ReceivedMessage(_) => "ReceivedMessage",
            Self::ErrorEvent(_) => "ErrorEvent",
        }
    }
}

impl TryFrom<EventFrame> for NetworkEvent {
    type Error = io::Error;

    fn try_from(value: EventFrame) -> Result<Self, Self::Error> {
        use event_frame::Variant;

        match value.variant.ok_or_else(io_err_invalid_data)? {
            Variant::ServerHello(hello) => Ok(Self::ServerHello(hello.try_into()?)),

            Variant::ChannelSync(channel_sync) => Ok(Self::ChannelSync(channel_sync.try_into()?)),

            Variant::UserSync(user_sync) => Ok(Self::UserSync(user_sync.try_into()?)),

            Variant::UserJoined(user_info) => Ok(Self::UserJoined(user_info.try_into()?)),

            Variant::UserLeft(user_id) => Ok(Self::UserLeft(user_id.try_into()?)),

            Variant::UserInfoUpdated(user_info) => Ok(Self::UserInfoUpdated(user_info.try_into()?)),

            Variant::ReceivedMessage(message) => {
                Ok(NetworkEvent::ReceivedMessage(message.try_into()?))
            }

            Variant::ErrorEvent(error) => Ok(NetworkEvent::ErrorEvent(error.try_into()?)),
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

            NetworkEvent::UserJoined(user_info) => Self {
                variant: Some(Variant::UserJoined(user_info.into())),
            },

            NetworkEvent::UserLeft(user_id) => Self {
                variant: Some(Variant::UserLeft(user_id.into())),
            },

            NetworkEvent::UserInfoUpdated(user_info) => Self {
                variant: Some(Variant::UserInfoUpdated(user_info.into())),
            },

            NetworkEvent::ReceivedMessage(message) => Self {
                variant: Some(Variant::ReceivedMessage(message.into())),
            },

            NetworkEvent::ErrorEvent(error) => Self {
                variant: Some(Variant::ErrorEvent(error.into())),
            },
        }
    }
}
