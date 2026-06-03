mod proto {
    include!(concat!(env!("OUT_DIR"), "/network_protocol.items.rs"));
}

pub mod codecs;
mod network_command;
mod network_event;

pub use network_command::{
    FetchChannels, FetchUsers, NetworkCommand, SendDestination, SendMessage,
};

pub use network_event::{
    ChannelInfo, ChannelSync, NetworkEvent, ReceiveDestination, ReceivedMessage, ServerHello,
    UserInfo, UserSync,
};

use std::fmt::{self, Display, Formatter};
use std::io;
use std::num::ParseIntError;
use std::str::FromStr;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Default port the server listens to for new connections.
pub const DEFAULT_LISTENER_PORT: u16 = 12345;

impl TryFrom<proto::Uuid> for Uuid {
    type Error = io::Error;

    fn try_from(value: proto::Uuid) -> Result<Self, Self::Error> {
        let arr: [u8; 16] = value.value.try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid UUID length: bytes length != 16",
            )
        })?;

        Ok(Uuid::from_bytes(arr))
    }
}

impl From<Uuid> for proto::Uuid {
    fn from(value: Uuid) -> Self {
        let value = Vec::from(value.as_bytes());

        Self { value }
    }
}

/// Type to uniquely identify clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct UserId(pub Uuid);

impl TryFrom<proto::UserId> for UserId {
    type Error = io::Error;

    fn try_from(value: proto::UserId) -> Result<Self, Self::Error> {
        let value = value.id.ok_or_else(io_err_invalid_data)?.try_into()?;

        Ok(UserId(value))
    }
}

impl From<UserId> for proto::UserId {
    fn from(value: UserId) -> Self {
        Self {
            id: Some(value.0.into()),
        }
    }
}

impl FromStr for UserId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::from_str(s)?))
    }
}

impl Display for UserId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "UserId({})", self.0)
    }
}

/// Type to uniquely identify channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ChannelId(u64);

impl TryFrom<proto::ChannelId> for ChannelId {
    type Error = io::Error;

    fn try_from(value: proto::ChannelId) -> Result<Self, Self::Error> {
        Ok(ChannelId(value.id))
    }
}

impl From<ChannelId> for proto::ChannelId {
    fn from(value: ChannelId) -> Self {
        Self { id: value.0 }
    }
}

impl FromStr for ChannelId {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}

impl Display for ChannelId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "ChannelId({})", self.0)
    }
}

fn io_err_invalid_data() -> io::Error {
    io::Error::from(io::ErrorKind::InvalidData)
}
