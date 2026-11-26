use network_protocol::{ErrorEvent, ErrorKind, NetworkEvent, UserId, UserInfo};
use thiserror::Error;
use tokio::sync::mpsc;

pub const ALLOWED_NON_ALPHANUMERIC_CHARACTERS: [char; 2] = ['_', '-'];

/// Represents a connected user.
#[derive(Debug, Clone)]
pub struct User {
    pub info: UserInfo,
    pub sender: mpsc::Sender<NetworkEvent>,
}

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
