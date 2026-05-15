mod proto {
    include!(concat!(env!("OUT_DIR"), "/network_protocol.items.rs"));
}

pub mod codecs;
mod network_command;
mod network_event;

pub use network_command::{NetworkCommand, SendDestination, SendMessage};

pub use network_event::{
    ChannelSync, NetworkEvent, ReceiveDestination, ReceivedMessage, ServerHello, UserSync,
};

use std::io;

/// Type to uniquely identify clients.
pub type UserId = String;

impl TryFrom<proto::UserId> for UserId {
    type Error = io::Error;

    fn try_from(value: proto::UserId) -> Result<Self, Self::Error> {
        Ok(value.id)
    }
}

impl From<UserId> for proto::UserId {
    fn from(value: UserId) -> Self {
        proto::UserId { id: value }
    }
}

/// Type to uniquely identify channels.
pub type ChannelId = String;

impl TryFrom<proto::ChannelId> for ChannelId {
    type Error = io::Error;

    fn try_from(value: proto::ChannelId) -> Result<Self, Self::Error> {
        Ok(value.id)
    }
}

impl From<ChannelId> for proto::ChannelId {
    fn from(value: ChannelId) -> Self {
        proto::ChannelId { id: value }
    }
}

/// Default port the server listens to for new connections.
pub const DEFAULT_LISTENER_PORT: u16 = 12345;
