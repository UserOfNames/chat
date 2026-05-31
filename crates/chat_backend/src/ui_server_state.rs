use std::collections::{BTreeSet, HashMap};
use std::net::SocketAddr;

use network_protocol::{ChannelId, ReceiveDestination, ReceivedMessage, UserId};

use crate::client_event::{ClientEvent, InitialSync};

/// What message list to display.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum MessageContext {
    Channel(ChannelId),
    User(UserId),
}

/// State struct holding information about the current connection, such as the address of the
/// server, a list of channels and users, the message history, etc.
///
/// Includes a helper method to easily update the state using [`ClientEvent`]s.
///
/// Note that a UI is free to implement its own state handling based on [`ClientEvent`]s if desired.
/// This is provided for convenience, as a reasonable default implementation.
#[derive(Debug)]
pub struct UIServerState {
    /// Your user ID for the session. If this is `None`, it means the backend has not synced the
    /// server state. This most commonly means you aren't connected to a server.
    pub your_id: UserId,

    /// The address of the server you're currently connected to, if any.
    pub connected_addr: SocketAddr,

    /// The current message context. This determines what messages will be displayed. If `None`,
    /// there is no current context.
    pub message_context: Option<MessageContext>,

    /// List of channels in the current server.
    pub channels: BTreeSet<ChannelId>,

    /// List of users in the current server.
    pub users: BTreeSet<UserId>,

    /// Message history in the current server.
    pub messages: HashMap<MessageContext, Vec<ReceivedMessage>>,
}

impl UIServerState {
    /// Create a new [`UIState`] instance with small, pre-allocated buffers.
    #[must_use]
    pub fn new(initial_sync: InitialSync) -> Self {
        let InitialSync {
            your_id,
            default_channel_id,
            server_addr,
        } = initial_sync;

        // None of the pre-allocation numbers are significant; just reasonable defaults
        Self {
            your_id,
            connected_addr: server_addr,
            message_context: default_channel_id.map(MessageContext::Channel),
            channels: BTreeSet::new(),
            users: BTreeSet::new(),
            messages: HashMap::new(),
        }
    }

    /// Update the UI state from a [`ClientEvent`].
    ///
    /// # Invalid variants
    /// Most [`ClientEvent`] variants are valid for this method, but some must be handled specially:
    /// * [`ClientEvent::InitialSync`]: There should only be one [`InitialSync`], which is passed to
    ///   [`Self::new`].
    /// * [`ClientEvent::Disconnected`]: This should result in the total destruction of [`Self`].
    /// * [`ClientEvent::ServerShutDown`]: This should result in the total destruction of [`Self`].
    pub fn update_from_event(&mut self, event: ClientEvent) {
        match event {
            ClientEvent::InitialSync(sync) => todo!("Log error (should not sync twice)"),

            ClientEvent::UserSync(sync) => self.users.extend(sync.user_ids),

            ClientEvent::ChannelSync(sync) => self.channels.extend(sync.channel_ids),

            ClientEvent::UserJoined(user_id) => {
                self.users.insert(user_id);
            }

            ClientEvent::UserLeft(user_id) => {
                if let Some(MessageContext::User(id)) = &self.message_context
                    && id == &user_id
                {
                    self.message_context = None;
                }

                self.users.remove(&user_id);
            }

            ClientEvent::ReceivedMessage(message) => self.push_message(message),

            ClientEvent::Disconnected => todo!("Log error (destroy self)"),
            ClientEvent::ServerShutDown => todo!("Log error (destroy self)"),
        }
    }

    /// Add a new message to a message list.
    fn push_message(&mut self, message: ReceivedMessage) {
        let context = match message.destination {
            // If we sent the message, its context is the destination.
            ReceiveDestination::User(ref id) if message.sender_id == self.your_id => {
                MessageContext::User(id.clone())
            }

            // Otherwise, the context is the sender.
            ReceiveDestination::User(_) => MessageContext::User(message.sender_id.clone()),

            ReceiveDestination::Channel(ref channel) => MessageContext::Channel(channel.clone()),
        };

        // Default vector capacity of 128 is only a reasonable default, not a significant value
        self.messages
            .entry(context)
            .or_insert(Vec::with_capacity(128))
            .push(message);
    }
}
