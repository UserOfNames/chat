use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_util::sync::CancellationToken;

use crate::run::ServerState;

use super::connection::Connection;

/// A task struct designed to listen for new client connections.
pub struct Listener {
    /// Server state - users, channels, etc.
    server_state: Arc<ServerState>,

    /// Cancellation token for the main task to signal for shutdown.
    cancellation_token: CancellationToken,

    /// Wrapper around a [`ClientConfig`](rustls::ClientConfig) for TLS handshakes.
    tls_acceptor: TlsAcceptor,

    /// The address on which to bind the listener.
    bind_address: SocketAddr,
}

impl Listener {
    /// Create a new `Listener`.
    pub fn new(
        server_state: Arc<ServerState>,
        tls_acceptor: TlsAcceptor,
        bind_address: SocketAddr,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            server_state,
            cancellation_token,
            tls_acceptor,
            bind_address,
        }
    }

    /// Start an initialized `Listener`. This should be spawned as a [`tokio`] task: `tokio::spawn(listener)`.
    pub async fn start(self) -> io::Result<()> {
        let address = self.bind_address;

        let listener = TcpListener::bind(address).await?;

        loop {
            tokio::select! {
                conn = listener.accept() => match conn {
                    Ok((stream, _addr)) => {
                        tokio::spawn(Connection::start(
                            self.server_state.clone(),
                            self.tls_acceptor.clone(),
                            stream
                        ));
                    }

                    Err(e) => todo!("Log error"),
                },

                () = self.cancellation_token.cancelled() => break,
            }
        }

        Ok(())
    }
}
