use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{debug, info, instrument, warn};

use crate::run::ServerState;

use super::connection::Connection;

/// A task struct designed to listen for new client connections.
pub struct Listener {
    /// Server state - users, channels, etc.
    server_state: Arc<ServerState>,

    /// Cancellation token for the main task to signal for shutdown.
    cancellation_token: CancellationToken,

    /// Task tracker for the main task to join all tasks on shutdown.
    task_tracker: TaskTracker,

    /// Wrapper around a [`ClientConfig`](rustls::ClientConfig) for TLS handshakes.
    tls_acceptor: TlsAcceptor,

    /// The address on which to bind the listener.
    bind_address: SocketAddr,
}

impl Listener {
    /// Create a new `Listener`.
    pub fn new(
        server_state: Arc<ServerState>,
        cancellation_token: CancellationToken,
        task_tracker: TaskTracker,
        tls_acceptor: TlsAcceptor,
        bind_address: SocketAddr,
    ) -> Self {
        Self {
            server_state,
            cancellation_token,
            task_tracker,
            tls_acceptor,
            bind_address,
        }
    }

    /// Start an initialized `Listener`. This should be spawned as a [`tokio`] task: `tokio::spawn(listener)`.
    #[instrument(skip_all, fields(address = %self.bind_address), parent = None)]
    pub async fn start(self) -> io::Result<()> {
        let address = self.bind_address;

        let listener = TcpListener::bind(address).await?;

        info!("TCP listener bound and accepting connections");

        loop {
            tokio::select! {
                conn = listener.accept() => match conn {
                    Ok((stream, peer_addr)) => {
                        debug!(%peer_addr, "Accepted incoming TCP connection");

                        self.task_tracker.spawn(Connection::start(
                            self.server_state.clone(),
                            self.tls_acceptor.clone(),
                            stream,
                            peer_addr,
                            self.cancellation_token.clone(),
                        ));
                    }

                    Err(e) => {
                        warn!(error = %e, "Failed to accept incoming TCP connection");
                        continue;
                    }
                },

                () = self.cancellation_token.cancelled() => {
                    info!("Listener task received cancellation signal, shutting down...");
                    break;
                }
            }
        }

        Ok(())
    }
}
