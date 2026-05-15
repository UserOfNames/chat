use std::{collections::HashMap, net::SocketAddr};

use network_protocol::{ChannelId, NetworkEvent, ReceivedMessage, UserId};

use crate::client_event::ClientEvent;

/// State struct holding information for the client, such as the current connection (if any), a
/// channel list, a user list, a message list, etc.
///
/// Includes a helper method to easily update the state using [`ClientEvent`]s.
///
/// Note that a UI is free to implement its own state handling based on [`ClientEvent`]s if desired.
/// This is provided for convenience.
#[derive(Debug, Default)]
pub struct UIState {
    connected_addr: Option<SocketAddr>,
    channels: Vec<ChannelId>,
    users: Vec<UserId>,
    messages: Vec<ReceivedMessage>,
}

impl UIState {
    /// Create a new [`UIState`] instance with small, pre-allocated buffers.
    pub fn new() -> Self {
        // None of the pre-allocation numbers are significant; just reasonable defaults
        Self {
            connected_addr: None,
            channels: Vec::with_capacity(8),
            users: Vec::with_capacity(32),
            messages: Vec::with_capacity(128),
        }
    }

    /// Attempt to update state based on a [`ClientEvent`]. If the given [`ClientEvent`] does not
    /// correspond to a valid state update, this will fail.
    ///
    /// # Valid variants
    /// Every [`ClientEvent`] variant besides
    /// [`NetworkEvent(NetworkEvent)`](ClientEvent::NetworkEvent) is a valid state change. After
    /// all, any such variant is designed specifically for the backend to update the UI.
    ///
    /// Currently, all [`NetworkEvent`]s represent a valid state change. This is not guaranteed to
    /// be the case always.
    ///
    /// Valid [`NetworkEvent`] variants for this method are listed below:
    /// * [`ServerHello`](NetworkEvent::ServerHello)
    /// * [`ChannelSync`](NetworkEvent::ChannelSync)
    /// * [`UserSync`](NetworkEvent::UserSync)
    /// * [`ReceivedMessage`](NetworkEvent::ReceivedMessage)
    pub fn update_from_event(&mut self, event: ClientEvent) -> Result<(), ()> {
        match event {
            ClientEvent::Connected(addr) => self.connected_addr = Some(addr),
            ClientEvent::Disconnected => self.connected_addr = None,

            ClientEvent::NetworkEvent(net_event) => match net_event {
                NetworkEvent::ServerHello(hello) => self.hello,
            }
        }
    }
}
