use std::collections::HashMap;
use std::net::SocketAddr;

use chat_backend::{
    client_event::{ClientEvent, InitialSync},
    network_protocol::{ChannelId, ReceiveDestination, ReceivedMessage, UserId, UserInfo},
};

const CHANNEL_INIT_CAPACITY: usize = 64;
const USER_INIT_CAPACITY: usize = 128;
const MESSAGE_INIT_CAPACITY: usize = 1024;

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
#[derive(Debug)]
pub struct ConnectionState {
    /// Your user ID for the session.
    pub your_id: UserId,

    /// The address of the server you're currently connected to.
    pub connected_addr: SocketAddr,

    /// The current message context. This determines what messages will be displayed. If `None`,
    /// there is no current context.
    pub message_context: Option<MessageContext>,

    /// List of channels in the current server.
    pub channels: HashMap<ChannelId, String>,

    /// Order in which channels are rendered.
    pub channel_render_order: Vec<ChannelId>,

    /// List of users in the current server.
    pub users: HashMap<UserId, String>,

    /// Order in which users are rendered.
    pub user_render_order: Vec<UserId>,

    /// Message history in the current server.
    pub messages: HashMap<MessageContext, Vec<ReceivedMessage>>,
}

impl ConnectionState {
    /// Create a new [`ConnectionState`] instance.
    #[must_use]
    pub fn new(initial_sync: InitialSync) -> Self {
        let InitialSync {
            your_id,
            default_channel_id,
            server_addr,
        } = initial_sync;

        Self {
            your_id,
            connected_addr: server_addr,
            message_context: default_channel_id.map(MessageContext::Channel),
            channels: HashMap::with_capacity(CHANNEL_INIT_CAPACITY),
            channel_render_order: Vec::with_capacity(CHANNEL_INIT_CAPACITY),
            users: HashMap::with_capacity(USER_INIT_CAPACITY),
            user_render_order: Vec::with_capacity(USER_INIT_CAPACITY),
            messages: HashMap::with_capacity(MESSAGE_INIT_CAPACITY),
        }
    }

    /// Update the UI state from a [`ClientEvent`].
    ///
    /// # Invalid variants
    /// Most [`ClientEvent`] variants are valid for this method, but some must be handled specially:
    /// * [`ClientEvent::InitialSync`]: There should only be one [`InitialSync`], which is passed to
    ///   [`Self::new`].
    /// * [`ClientEvent::Disconnected`]: This should result in dropping [`Self`].
    /// * [`ClientEvent::ServerShutDown`]: This should result in dropping [`Self`].
    pub fn update_from_event(&mut self, event: ClientEvent) {
        match event {
            ClientEvent::UserSync(sync) => {
                self.users
                    .extend(sync.users.into_iter().map(|user| (user.id, user.name)));
                self.rebuild_user_cache();
            }

            ClientEvent::ChannelSync(sync) => {
                self.channels
                    .extend(sync.channels.into_iter().map(|user| (user.id, user.name)));
                self.rebuild_channel_cache();
            }

            ClientEvent::UserJoined(user_info) => {
                self.users.insert(user_info.id, user_info.name);
                self.rebuild_user_cache();
            }

            ClientEvent::UserLeft(user_id) => {
                if let Some(MessageContext::User(id)) = &self.message_context
                    && id == &user_id
                {
                    self.message_context = None;
                }

                self.users.remove(&user_id);
                self.rebuild_user_cache();
            }

            ClientEvent::UserInfoUpdated(info) => {
                self.update_info(info);
                self.rebuild_user_cache();
            }

            ClientEvent::ReceivedMessage(message) => self.push_message(message),

            // Currently, no server errors demand a ConnectionState update. Because this may change
            // in the future, we make this a NOP instead of an error.
            ClientEvent::ErrorEvent(_) => {}

            // ==== INVALID EVENTS ====
            ClientEvent::InitialSync(_) => unreachable!(
                "Initial sync should result in the creation of a ConnectionState, not be routed to it"
            ),

            ClientEvent::Disconnected | ClientEvent::ServerShutDown => unreachable!(
                "Disconnection events should result in the destruction of CreationState, not be routed to it"
            ),
        }
    }

    /// Add a new message to a message list.
    fn push_message(&mut self, message: ReceivedMessage) {
        let context = match message.destination {
            // If we sent the message, its context is the destination.
            ReceiveDestination::User(id) if message.sender_id == self.your_id => {
                MessageContext::User(id)
            }

            // Otherwise, the context is the sender.
            ReceiveDestination::User(_) => MessageContext::User(message.sender_id),

            // Of course, the context of a channel is just the channel.
            ReceiveDestination::Channel(id) => MessageContext::Channel(id),
        };

        // Default vector capacity of 128 is only a reasonable default, not a significant value
        self.messages
            .entry(context)
            .or_insert(Vec::with_capacity(128))
            .push(message);
    }

    /// Update a user's info.
    fn update_info(&mut self, new_info: UserInfo) {
        self.users.insert(new_info.id, new_info.name);
    }

    /// Get the name of a channel with the given ID, if known.
    pub fn get_channel_name(&self, id: ChannelId) -> Option<&str> {
        self.channels.get(&id).map(String::as_str)
    }

    /// Get the name of a user with the given ID, if known.
    pub fn get_user_name(&self, id: UserId) -> Option<&str> {
        self.users.get(&id).map(String::as_str)
    }

    /// Rebuild [`Self::user_render_order`].
    fn rebuild_user_cache(&mut self) {
        // TODO: Optimize
        let mut others: Vec<UserId> = self
            .users
            .keys()
            .copied()
            .filter(|id| *id != self.your_id)
            .collect();

        others.sort_by_key(|id| {
            self.users
                .get(id)
                .expect("We just got the ID list from the hashmap keys, and nothing else could have changed the map in between")
                .to_lowercase()
        });

        self.user_render_order.clear();
        self.user_render_order.push(self.your_id);
        self.user_render_order.append(&mut others);
    }

    /// Rebuild [`Self::channel_render_order`].
    fn rebuild_channel_cache(&mut self) {
        // TODO: Optimize
        let mut channels: Vec<ChannelId> = self.channels.keys().copied().collect();

        channels.sort_by_key(|id| {
            self.channels
                .get(id)
                .expect("We just got the ID list from the hashmap keys, and nothing else could have changed the map in between")
                .to_lowercase()
        });

        self.channel_render_order.clear();
        self.channel_render_order.append(&mut channels);
    }
}
