use dashmap::{DashMap, DashSet};
use network_protocol::{ChannelId, ChannelInfo, NetworkEvent, UpdateInfo, UserId, UserInfo};
use tokio::sync::{broadcast, mpsc};

use thiserror::Error;

use crate::run::{Channel, User};

const ALLOWED_NON_ALPHANUMERIC_CHARACTERS: [char; 2] = ['_', '-'];

/// Error when handling a username.
#[derive(Debug, Clone, Error)]
pub enum UserNameError {
    /// The username is empty.
    #[error("usernames cannot be empty")]
    Empty,

    /// The username is too long.
    #[error("usernames cannot be longer than {0} characters")]
    TooLong(usize),

    /// The username contains an invalid character.
    #[error(
        "usernames may only contain letters, numbers, and any of {ALLOWED_NON_ALPHANUMERIC_CHARACTERS:?}"
    )]
    InvalidCharacter,

    /// The username was already taken.
    #[error("username '{0}' already taken")]
    AlreadyTaken(String),
}

/// Error when managing users on the server.
#[derive(Debug, Clone, Error)]
pub enum UserError {
    /// Error when updating a user's name.
    #[error("username error: {0}")]
    Name(#[from] UserNameError),

    /// The user ID given is not associated with a known user.
    #[error("user ID '{0}' does not exist")]
    DoesNotExist(UserId),
}

/// Error when managing channels on the server.
#[derive(Debug, Clone, Error)]
pub enum ChannelError {
    #[error("duplicate channel ID: {0}")]
    AlreadyExists(ChannelId),
}

/// Unique token representing a specific user. This wraps the user's `UserId`, but can't be forged
/// by another user.
///
/// It is used to enforce the invariant that a `Connection` is only allowed to modify the `UserInfo`
/// of its associated user. This prevents race conditions in some methods.
#[derive(Debug)]
#[repr(transparent)]
pub struct UserToken(UserId);

impl UserToken {
    pub fn id(&self) -> UserId {
        self.0
    }
}

/// State shared between all tasks.
#[derive(Debug)]
pub struct ServerState {
    /// The default channel's ID.
    default_channel_id: Option<ChannelId>,

    /// Broadcast sender to send an event to all connected clients.
    global_broadcast: broadcast::Sender<NetworkEvent>,

    /// Map from channel IDs to channels.
    channels: DashMap<ChannelId, Channel>,

    /// Map from user IDs to users.
    users: DashMap<UserId, User>,

    /// Set of all connected users' names. Used for fast, atomic lookups to enforce username
    /// uniqueness.
    taken_names: DashSet<String>,
}

impl ServerState {
    /// Initialize a `ServerState` instance.
    pub fn new(default_channel_id: Option<ChannelId>) -> Self {
        Self {
            default_channel_id,
            global_broadcast: broadcast::channel(128).0, // TODO: Buffer size
            channels: DashMap::new(),
            users: DashMap::new(),
            taken_names: DashSet::new(),
        }
    }

    /// Get the default channel ID, if there is one.
    pub fn default_channel_id(&self) -> Option<ChannelId> {
        self.default_channel_id
    }

    /// Send an event to all active users.
    pub fn send_global_event(&self, event: NetworkEvent) {
        // The only failure condition for sending through a broadcast channel is if there are no
        // receivers, but we don't actually care if nobody gets this message. As such, we ignore
        // this error.
        let _: Result<_, _> = self.global_broadcast.send(event);
    }

    /// Subscribe to the global broadcast channel.
    pub fn subscribe_to_global(&self) -> broadcast::Receiver<NetworkEvent> {
        self.global_broadcast.subscribe()
    }

    /// Get a channel's [`ChannelInfo`] by its ID, if the ID is associated with a channel on the
    /// server.
    pub fn get_channel_info(&self, id: ChannelId) -> Option<ChannelInfo> {
        self.channels.get(&id).map(|channel| channel.info.clone())
    }

    /// Get the [`ChannelInfo`] of every channel on the server. If there are no channels, returns an
    /// empty [`Vec`].
    pub fn get_all_channel_info(&self) -> Vec<ChannelInfo> {
        self.channels
            .iter()
            .map(|entry| entry.info.clone())
            .collect()
    }

    /// Send a [`NetworkEvent`] to a channel with the given ID, if that ID is associated with a
    /// channel on the server.
    ///
    /// Returns `false` if the channel was not present.
    pub fn send_event_to_channel(&self, target_id: ChannelId, event: NetworkEvent) -> bool {
        let Some(channel) = self.channels.get(&target_id) else {
            return false;
        };

        // The only failure condition for sending through a broadcast channel is if there are no
        // receivers, but we don't actually care if nobody gets this message. As such, we ignore
        // this error.
        let _: Result<_, _> = channel.broadcast.send(event);
        true
    }

    /// Add a new channel to the server.
    ///
    /// It is the server administrator's responsibility to ensure that each channel has a unique ID.
    /// Channels may have duplicate names.
    ///
    /// # Errors
    /// Returns [`ChannelError`] if a called with an ID that is already present.
    pub fn add_channel(
        &self,
        id: ChannelId,
        name: String,
        event_tx: broadcast::Sender<NetworkEvent>,
    ) -> Result<(), ChannelError> {
        let channel_info = ChannelInfo { id, name };

        let channel = Channel {
            info: channel_info,
            broadcast: event_tx,
        };

        if let dashmap::Entry::Vacant(entry) = self.channels.entry(id) {
            entry.insert(channel);
            Ok(())
        } else {
            Err(ChannelError::AlreadyExists(id))
        }
    }

    /// Subscribe to all channels on the server. Returns a [`Vec`] of [`broadcast::Receiver`]s for
    /// every channel.
    pub fn subscribe_to_channels(&self) -> Vec<broadcast::Receiver<NetworkEvent>> {
        self.channels
            .iter()
            .map(|pair| pair.value().broadcast.subscribe())
            .collect()
    }

    /// Get a user's [`UserInfo`] by their ID, if the ID is associated with a user on the server.
    pub fn get_user_info(&self, id: UserId) -> Option<UserInfo> {
        self.users.get(&id).map(|user| user.info.clone())
    }

    /// Get the [`UserInfo`] of every user on the server. If there are no users, returns an empty
    /// [`Vec`].
    pub fn get_all_user_info(&self) -> Vec<UserInfo> {
        self.users.iter().map(|entry| entry.info.clone()).collect()
    }

    /// Send a [`NetworkEvent`] to a client with the given ID, if that ID is associated with a user
    /// on the server.
    ///
    /// Returns `true` if the user was present.
    pub async fn send_event_to_user(&self, target_id: UserId, event: NetworkEvent) -> bool {
        // We have to do it this way to avoid holding the lock over the `await` point, which could
        // deadlock.
        let sender = {
            let Some(user) = self.users.get(&target_id) else {
                return false;
            };
            user.sender.clone()
        };

        let _: Result<_, _> = sender.send(event).await;
        true
    }

    /// Register a new (ID, name) user pair. This will:
    /// 1. Ensure the name is not empty, and does not exceed the maximum length.
    /// 2. Ensure the name contains no invalid characters.
    /// 3. Ensure the name is not already registered (case-insensitive).
    /// 4. Register the name.
    ///
    /// # Errors
    /// Returns a [`NameRegistrationError`] if name registration fails.
    pub fn handle_new_user(
        &self,
        name: String,
        max_username_length: usize,
        event_tx: mpsc::Sender<NetworkEvent>,
    ) -> Result<UserToken, UserError> {
        let name = name.trim();

        Self::validate_username(name, max_username_length)?;
        let normalized_name = Self::normalize_username(name);

        if !self.taken_names.insert(normalized_name) {
            return Err(UserError::Name(UserNameError::AlreadyTaken(
                name.to_owned(),
            )));
        }

        let user_id = UserId(uuid::Uuid::now_v7());

        let user_info = UserInfo {
            id: user_id,
            name: name.to_owned(),
        };

        let user = User {
            info: user_info.clone(),
            sender: event_tx,
        };

        self.users.insert(user_id, user);

        self.send_global_event(NetworkEvent::UserJoined(user_info));

        Ok(UserToken(user_id))
    }

    /// Update a user's information with the given [`UpdateInfo`]. `Some` fields will be updated,
    /// while `None` fields will be unmodified. The update operation is atomic - if any updates fail
    /// (for example, if a username is invalid), the entire update will fail.
    pub fn update_user_info(
        &self,
        token: &UserToken,
        new_info: UpdateInfo,
        max_username_length: usize,
    ) -> Result<(), UserError> {
        /// Drop guard for name operations. To make operations on the `taken_names` set atomic, we
        /// need to mutate the set. However, that mutation makes the whole function non-atomic, as
        /// we touch persistent state between checkpoints. This patches that hole.
        #[derive(Debug)]
        struct DropGuard<'a> {
            taken_names: &'a DashSet<String>,
            added_name: Option<String>,
            removed_name: Option<String>,
            committed: bool,
        }

        impl Drop for DropGuard<'_> {
            fn drop(&mut self) {
                if self.committed {
                    return;
                }

                if let Some(added) = self.added_name.take() {
                    self.taken_names.remove(&added);
                }

                if let Some(removed) = self.removed_name.take() {
                    self.taken_names.insert(removed);
                }
            }
        }

        let mut drop_guard = DropGuard {
            taken_names: &self.taken_names,
            added_name: None,
            removed_name: None,
            committed: false,
        };

        let Some(mut proposed_user_info) =
            self.users.get(&token.id()).map(|inner| inner.info.clone())
        else {
            return Err(UserError::DoesNotExist(token.id()));
        };

        if let Some(new_name) = new_info.name {
            let new_name = new_name.trim();

            if let Err(e) = Self::validate_username(new_name, max_username_length) {
                todo!("Log and report validation error {e}, return");
            }

            let normalized_new_name = Self::normalize_username(new_name);
            let normalized_old_name = Self::normalize_username(&proposed_user_info.name);

            // We should allow users to change normalized information, since it's all
            // inconsequential representation stuff. As such, if the normalized representations are
            // identical, we can skip all set updates.
            if normalized_new_name != normalized_old_name {
                if !self.taken_names.insert(normalized_new_name.clone()) {
                    // Nothing changes here - we're abandoning the operation - so we don't want to
                    // mutate the set, just return.
                    return Err(UserError::Name(UserNameError::AlreadyTaken(
                        new_name.to_owned(),
                    )));
                }
                drop_guard.added_name = Some(normalized_new_name);

                self.taken_names.remove(&normalized_old_name);
                drop_guard.removed_name = Some(normalized_old_name);
            }

            new_name.clone_into(&mut proposed_user_info.name);
        }

        if let Some(mut user_entry) = self.users.get_mut(&token.id()) {
            user_entry.info = proposed_user_info.clone();
            drop_guard.committed = true;
            self.send_global_event(NetworkEvent::UserInfoUpdated(proposed_user_info));
            Ok(())
        } else {
            Err(UserError::DoesNotExist(token.id()))
        }
    }

    /// Remove a user with the given ID from the server, if the ID is present.
    pub fn remove_user(&self, token: UserToken) -> Result<(), UserError> {
        let Some((_, user)) = self.users.remove(&token.id()) else {
            return Err(UserError::DoesNotExist(token.id()));
        };

        let normalized_name = Self::normalize_username(&user.info.name);
        self.taken_names.remove(&normalized_name);

        Ok(())
    }

    /// Validate a username. Validation involves:
    /// * Ensuring it is not empty.
    /// * Ensuring it does not exceed the maximum length.
    /// * Ensuring it contains no invalid characters.
    ///
    /// This function does not check for duplicate names; that must be done separately.
    fn validate_username(name: &str, max_length: usize) -> Result<(), UserNameError> {
        if name.is_empty() {
            return Err(UserNameError::Empty);
        }

        if name.len() > max_length {
            return Err(UserNameError::TooLong(max_length));
        }

        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || ALLOWED_NON_ALPHANUMERIC_CHARACTERS.contains(&c))
        {
            return Err(UserNameError::InvalidCharacter);
        }

        Ok(())
    }

    /// Normalize a username. This is useful to enforce that usernames aren't duplicated with
    /// inconsequential differences. As such, normalized usernames should be favored in
    /// [`Self::taken_names`].
    ///
    /// Normalization involves:
    /// * Turning all letters to lowercase.
    fn normalize_username(name: &str) -> String {
        name.to_lowercase()
    }
}
