use network_protocol::{ChannelId, ChannelInfo, NetworkEvent};
use thiserror::Error;
use tokio::sync::broadcast;

/// Represents a channel.
#[derive(Debug, Clone)]
pub struct Channel {
    pub info: ChannelInfo,
    pub broadcast: broadcast::Sender<NetworkEvent>,
}

/// Error when managing channels on the server.
#[derive(Debug, Clone, Error)]
pub enum ChannelError {
    /// Attempted to add a channel ID that already exists.
    #[error("duplicate channel ID: {0}")]
    AlreadyExists(ChannelId),

    /// Attempted to access a channel ID that does not exist.
    #[error("channel does not exist: {0}")]
    DoesNotExist(ChannelId),
}
