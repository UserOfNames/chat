use network_protocol::{
    ChannelId, ChannelInfo, ErrorEvent, ErrorKind, NetworkEvent, UpdateInfo, UserId, UserInfo,
};
use scc::{HashMap, HashSet};
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
    TargetNotFound(UserId),

    /// Your own user ID is no longer known to the server. This indicates a fatal state mismatch.
    #[error("fatal state mismatch, your ID was not found on the server")]
    YourIdNotFound,
}

impl From<UserError> for ErrorEvent {
    fn from(value: UserError) -> Self {
        match value {
            UserError::Name(e @ UserNameError::AlreadyTaken(_)) => Self {
                kind: ErrorKind::NameTaken,
                message: e.to_string(),
            },

            // All other UserNameError variants are handled the same way
            UserError::Name(other) => Self {
                kind: ErrorKind::InvalidName,
                message: other.to_string(),
            },

            e @ UserError::TargetNotFound(_) => Self {
                kind: ErrorKind::TargetNotFound,
                message: e.to_string(),
            },

            e @ UserError::YourIdNotFound => Self {
                kind: ErrorKind::ServerError,
                message: e.to_string(),
            },
        }
    }
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

    /// Maximum allowed length of users' display names.
    max_username_length: usize,

    /// Broadcast sender to send an event to all connected clients.
    global_broadcast: broadcast::Sender<NetworkEvent>,

    /// Map from channel IDs to channels.
    channels: HashMap<ChannelId, Channel>,

    /// Map from user IDs to users.
    users: HashMap<UserId, User>,

    /// Set of all connected users' names. Used for fast, atomic lookups to enforce username
    /// uniqueness.
    taken_names: HashSet<String>,
}

impl ServerState {
    /// Initialize a `ServerState` instance.
    pub fn new(default_channel_id: Option<ChannelId>, max_username_length: usize) -> Self {
        const CHANNEL_INIT_CAPACITY: usize = 64;
        const USER_INIT_CAPACITY: usize = 4096;

        Self {
            default_channel_id,
            max_username_length,
            global_broadcast: broadcast::channel(128).0, // TODO: Buffer size
            channels: HashMap::with_capacity(CHANNEL_INIT_CAPACITY),
            users: HashMap::with_capacity(USER_INIT_CAPACITY),
            taken_names: HashSet::with_capacity(USER_INIT_CAPACITY),
        }
    }

    /// Get the default channel ID, if there is one.
    pub fn default_channel_id(&self) -> Option<ChannelId> {
        self.default_channel_id
    }

    /// Get the maximum allowed username length.
    pub fn max_username_length(&self) -> usize {
        self.max_username_length
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
    #[expect(dead_code)]
    pub async fn get_channel_info(&self, id: ChannelId) -> Option<ChannelInfo> {
        self.channels
            .read_async(&id, |_, channel| channel.info.clone())
            .await
    }

    /// Get the [`ChannelInfo`] of every channel on the server. If there are no channels, returns an
    /// empty [`Vec`].
    pub async fn get_all_channel_info(&self) -> Vec<ChannelInfo> {
        let mut res = Vec::with_capacity(self.channels.len());

        self.channels
            .iter_async(|_, value| {
                res.push(value.info.clone());
                true
            })
            .await;

        res
    }

    /// Send a [`NetworkEvent`] to a channel with the given ID, if that ID is associated with a
    /// channel on the server.
    ///
    /// # Errors
    /// Returns [`ChannelError::DoesNotExist`] if the target channel ID was not found.
    pub async fn send_event_to_channel(
        &self,
        target_id: ChannelId,
        event: NetworkEvent,
    ) -> Result<(), ChannelError> {
        // The only failure condition for sending through a broadcast channel is if there are no
        // receivers, but we don't actually care if nobody gets this message. As such, we ignore
        // the Result.
        self.channels
            .read_async(&target_id, |_, value| value.broadcast.send(event))
            .await
            .map(|_ignored_result| ())
            .ok_or(ChannelError::DoesNotExist(target_id))
    }

    /// Add a new channel to the server.
    ///
    /// It is the server administrator's responsibility to ensure that each channel has a unique ID.
    /// Channels may have duplicate names.
    ///
    /// # Errors
    /// Returns [`ChannelError`] if a called with an ID that is already present.
    pub async fn add_channel(
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

        self.channels
            .insert_async(id, channel)
            .await
            .map_err(|_| ChannelError::AlreadyExists(id))
    }

    /// Subscribe to all channels on the server. Returns a [`Vec`] of [`broadcast::Receiver`]s for
    /// every channel.
    pub async fn subscribe_to_channels(&self) -> Vec<broadcast::Receiver<NetworkEvent>> {
        let mut res = Vec::with_capacity(self.channels.len());

        self.channels
            .iter_async(|_, value| {
                res.push(value.broadcast.subscribe());
                true
            })
            .await;

        res
    }

    /// Get a user's [`UserInfo`] by their ID, if the ID is associated with a user on the server.
    #[expect(dead_code)]
    pub async fn get_user_info(&self, id: UserId) -> Option<UserInfo> {
        self.users
            .read_async(&id, |_, value| value.info.clone())
            .await
    }

    /// Get the [`UserInfo`] of every user on the server. If there are no users, returns an empty
    /// [`Vec`].
    pub async fn get_all_user_info(&self) -> Vec<UserInfo> {
        let mut res = Vec::with_capacity(self.users.len());

        self.users
            .iter_async(|_, value| {
                res.push(value.info.clone());
                true
            })
            .await;

        res
    }

    /// Send a [`NetworkEvent`] to a client with the given ID, if that ID is associated with a user
    /// on the server.
    ///
    /// # Errors
    /// Returns [`UserError::TargetNotFound`] if the target ID was not found.
    pub async fn send_event_to_user(
        &self,
        target_id: UserId,
        event: NetworkEvent,
    ) -> Result<(), UserError> {
        let sender = self
            .users
            .read_async(&target_id, |_, value| value.sender.clone())
            .await
            .ok_or(UserError::TargetNotFound(target_id))?;

        // The only failure condition for sending through this channel is if the channel is closed,
        // but that can only happen if the target user disconnected, which we don't really care
        // about. As such, we ignore this Result.
        let _: Result<_, _> = sender.send(event).await;
        Ok(())
    }

    /// Register a new (ID, name) user pair. This will:
    /// 1. Ensure the name is not empty, and does not exceed the maximum length.
    /// 2. Ensure the name contains no invalid characters.
    /// 3. Ensure the name is not already registered (case-insensitive).
    /// 4. Register the name.
    ///
    /// # Errors
    /// Returns a [`NameRegistrationError`] if name registration fails.
    pub async fn handle_new_user(
        &self,
        name: String,
        max_username_length: usize,
        event_tx: mpsc::Sender<NetworkEvent>,
    ) -> Result<UserToken, UserError> {
        let name = name.trim();

        Self::validate_username(name, max_username_length)?;
        let normalized_name = Self::normalize_username(name);

        if self
            .taken_names
            .insert_async(normalized_name)
            .await
            .is_err()
        {
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

        self.users.insert_async(user_id, user).await.expect(
            "This error would indicate a UUID collision, which we can assume to be impossible",
        );

        self.send_global_event(NetworkEvent::UserJoined(user_info));

        Ok(UserToken(user_id))
    }

    /// Update a user's information with the given [`UpdateInfo`]. `Some` fields will be updated,
    /// while `None` fields will be unmodified. The update operation is atomic - if any updates fail
    /// (for example, if a username is invalid), the entire update will fail.
    pub async fn update_user_info(
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
            taken_names: &'a HashSet<String>,
            added_name: Option<String>,
            committed: bool,
        }

        impl Drop for DropGuard<'_> {
            fn drop(&mut self) {
                if self.committed {
                    return;
                }

                if let Some(added) = self.added_name.take() {
                    self.taken_names
                        .remove_sync(&added)
                        .expect("If Some(added), we definitely already added the name to the set, so it will still be there");
                }
            }
        }

        // Before anything else, if the entire `UpdateInfo` is all None, this whole function is a
        // NOP. We check that first.
        if matches!(new_info, UpdateInfo { name: None }) {
            return Ok(());
        }

        let mut drop_guard = DropGuard {
            taken_names: &self.taken_names,
            added_name: None,
            committed: false,
        };

        let Some(mut proposed_user_info) = self
            .users
            .read_async(&token.id(), |_, value| value.info.clone())
            .await
        else {
            return Err(UserError::YourIdNotFound);
        };

        // Name to remove from `taken_names` if we update the user's name
        let mut old_name_to_remove: Option<String> = None;

        if let Some(new_name) = new_info.name {
            let new_name = new_name.trim();

            if let Err(e) = Self::validate_username(new_name, max_username_length) {
                return Err(e.into());
            }

            let normalized_new_name = Self::normalize_username(new_name);
            let normalized_old_name = Self::normalize_username(&proposed_user_info.name);

            // We should allow users to change normalized information, since it's all
            // inconsequential representation stuff. As such, if the normalized representations are
            // identical, we can skip all set updates.
            if normalized_new_name != normalized_old_name {
                if self
                    .taken_names
                    .insert_async(normalized_new_name.clone())
                    .await
                    .is_err()
                {
                    // Nothing changes here - we're abandoning the operation - so we don't want to
                    // mutate the set, just return.
                    return Err(UserError::Name(UserNameError::AlreadyTaken(
                        new_name.to_owned(),
                    )));
                }

                drop_guard.added_name = Some(normalized_new_name);

                // We defer removal until we know the entire transaction succeded
                old_name_to_remove = Some(normalized_old_name);
            }

            new_name.clone_into(&mut proposed_user_info.name);
        }

        let updated = self
            .users
            .update_async(&token.id(), |_, user_entry| {
                user_entry.info = proposed_user_info.clone();
            })
            .await
            .is_some();

        if updated {
            drop_guard.committed = true;

            // We have to do this here, not earlier, to avoid the following race condition:
            // 1. Bob asks to change his name to Bob1234, removes "bob" from `taken_names`, and gets
            //    preempted.
            // 2. Bob4321 asks to change his name to Bob. It succeeds.
            // 3. Bob (original)'s state change fails.
            // This would allow 2 users to have the same name. Therefore, we must be sure the
            // transaction succeeded before removing the old name.
            if let Some(name) = old_name_to_remove {
                // `None` is an edge case that should never happen, but we don't really care if it
                // somehow does.
                let _: Option<_> = self.taken_names.remove_async(&name).await;
            }

            self.send_global_event(NetworkEvent::UserInfoUpdated(proposed_user_info));
            Ok(())
        } else {
            Err(UserError::YourIdNotFound)
        }
    }

    /// Remove a user with the given ID from the server, if the ID is present.
    pub async fn remove_user(&self, token: UserToken) -> Result<(), UserError> {
        let Some((_, user)) = self.users.remove_async(&token.id()).await else {
            return Err(UserError::YourIdNotFound);
        };

        let normalized_name = Self::normalize_username(&user.info.name);
        // We don't care about this state inconsistency since we're disconnecting anyways.
        let _: Option<_> = self.taken_names.remove_async(&normalized_name).await;

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
