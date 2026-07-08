use std::sync::Arc;

use network_protocol::{NetworkEvent, UserId};
use tracing::warn;

use crate::run::server_state::{ServerState, UserError, UserToken};

/// RAII guard that automatically unregisters a user when dropped.
#[derive(Debug)]
pub struct ConnectionGuard {
    token: Option<UserToken>,
    server_state: Arc<ServerState>,
}

impl ConnectionGuard {
    pub fn new(token: UserToken, server_state: Arc<ServerState>) -> Self {
        Self {
            token: Some(token),
            server_state,
        }
    }

    pub fn token(&self) -> &UserToken {
        self.token
            .as_ref()
            .expect("Token is always present while running")
    }

    pub fn id(&self) -> UserId {
        self.token().id()
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        let token = self
            .token
            .take()
            .expect("Token is always present while running");

        let id = token.id();
        let server_state = self.server_state.clone();

        tokio::spawn(async move {
            match server_state.remove_user(token).await {
                Err(UserError::YourIdNotFound) => {
                    warn!(%id, "State mismatch detected while disconnecting: ID not found");
                }

                _ => server_state.send_global_event(NetworkEvent::UserLeft(id)),
            }
        });
    }
}
