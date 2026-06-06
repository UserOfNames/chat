pub use network_protocol::ReceivedMessage;

use std::io;
use std::net::SocketAddr;
use std::result::Result as StdResult;

use thiserror::Error;

use network_protocol::{
    ChannelId, ChannelSync, ErrorEvent, NetworkEvent, UserId, UserInfo, UserSync,
};

/// An error arising in the client backend while processing a `ClientCommand`.
#[derive(Debug, Error)]
pub enum Error {
    /// An I/O error occurred while handling the command. This most commonly indicates an error
    /// while attempting to communicate with the server.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

/// Struct holding initial information about the server connection.
#[derive(Debug)]
pub struct InitialSync {
    pub your_id: UserId,
    pub default_channel_id: Option<ChannelId>,
    pub server_addr: SocketAddr,
}

/// A specialized `Result` type for carrying `ClientEvent`s to the frontend.
///
/// [`client_event::Error`](Error) is not to be confused with [`ClientEvent::ErrorEvent`]. The
/// former indicates that a command or event failed to process. The latter is an event originating
/// from the server, that successfully processed and transmitted, but indicates an error occurred
/// elsewhere.
pub type Result = StdResult<ClientEvent, Error>;

/// An event from the client backend to the UI.
#[derive(Debug)]
pub enum ClientEvent {
    /// Initial state sync, holding basic information about the server connection.
    InitialSync(InitialSync),

    /// Disconnected from the server.
    Disconnected,

    /// Server was shut down while connected.
    ServerShutDown,

    /// Bulk state update for the channel list.
    ChannelSync(ChannelSync),

    /// Bulk state update for the user list.
    UserSync(UserSync),

    /// A new user joined the server.
    UserJoined(UserInfo),

    /// A user left the server.
    UserLeft(UserId),

    /// A user's information was updated.
    UserInfoUpdated(UserInfo),

    /// A new message was received.
    ReceivedMessage(ReceivedMessage),

    /// An error occurred on the server.
    ErrorEvent(ErrorEvent),
}

/// Attempt to convert a [`NetworkEvent`] into a corresponding [`ClientEvent`].
///
/// # Errors
/// The conversion only fails if the [`NetworkEvent`] variant does not map to any [`ClientEvent`]
/// variant.
///
/// Invalid variants are:
/// * [`NetworkEvent::ServerHello`]: `InitialSync` carries some information from this variant, but
///   additional information is needed. This should only be sent once, when starting a connection.
impl TryFrom<NetworkEvent> for ClientEvent {
    type Error = ();

    fn try_from(value: NetworkEvent) -> StdResult<Self, Self::Error> {
        Ok(match value {
            NetworkEvent::UserSync(sync) => Self::UserSync(sync),
            NetworkEvent::ChannelSync(sync) => Self::ChannelSync(sync),
            NetworkEvent::UserJoined(user_id) => Self::UserJoined(user_id),
            NetworkEvent::UserLeft(user_id) => Self::UserLeft(user_id),
            NetworkEvent::ReceivedMessage(message) => Self::ReceivedMessage(message),
            NetworkEvent::UserInfoUpdated(info) => Self::UserInfoUpdated(info),
            NetworkEvent::ErrorEvent(error) => Self::ErrorEvent(error),

            NetworkEvent::ServerHello(_) => Err(())?,
        })
    }
}
