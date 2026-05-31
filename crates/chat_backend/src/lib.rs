pub mod client_command;
pub mod client_event;
mod connection;
pub mod ui_server_state;

/// Convenience re-export of types from [`network_protocol`].
pub mod network_protocol {
    pub use network_protocol::*;
}

use std::fs::{create_dir_all, write};
use std::io;
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::Arc;

use ::network_protocol::{FetchChannels, FetchUsers, ServerHello};
use figment::{
    Figment,
    providers::{Format, Toml},
};
use rustls::{
    RootCertStore,
    pki_types::{
        CertificateDer,
        pem::{self, PemObject},
    },
};
use serde::{Deserialize, Serialize};
use shared_utils::{NamedProjectDirs, TildeRelativePathBuf, first_match};
use thiserror::Error;
use tokio::sync::mpsc::{self, Receiver, Sender};

use client_command::ClientCommand;
use client_event::ClientEvent;
use connection::Connection;
use network_protocol::{NetworkCommand, NetworkEvent};
use tokio_rustls::TlsConnector;

use crate::client_event::InitialSync;

const DEFAULT_CONFIG: &str = include_str!("../data/config.toml");

#[derive(Debug, Error)]
pub enum InitError {
    // This variant is massive, so we have to box it or the linter will complain
    /// Extracting the [`figment::Figment`] into a [`Config`] failed.
    #[error("Config resolution failed: {0}")]
    ConfigResolutionFailed(#[source] Box<figment::Error>),

    /// Reading a certificate file from [`Config::additional_root_ca_paths`] failed.
    #[error("Reading certificate file '{path}' failed: {source}")]
    CertFileReadFailed { path: PathBuf, source: pem::Error },

    /// A certificate file from [`Config::additional_root_ca_paths`] could not be added to the main
    /// [`rustls::RootCertStore`].
    #[error("Certificate validation failed: {0}")]
    CertValidationFailed(#[from] rustls::Error),

    /// An [`io::Error`] occurred.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

impl From<figment::Error> for InitError {
    fn from(value: figment::Error) -> Self {
        Self::ConfigResolutionFailed(Box::new(value))
    }
}

#[derive(Debug)]
struct DefaultPaths {
    config: PathBuf,
}

impl DefaultPaths {
    /// Initialize a `BackendPaths` instance with default paths.
    ///
    /// `config`: `NamedProjectDirs::config_dir()/config.toml`
    fn defaults(component: impl Into<PathBuf>) -> Option<Self> {
        let base = NamedProjectDirs::new(component)?;

        let config = base.config_dir().join("config.toml");

        Some(Self { config })
    }
}

/// Configuration for the client backend runtime.
#[derive(Debug, Serialize, Deserialize)]
struct Config {
    /// Whether to include common PKI root certificates (default: true)
    include_webpki_roots: bool,
    /// Paths to additional root certificates (default: empty)
    additional_root_ca_paths: Vec<TildeRelativePathBuf>,
}

/// Contains channels through which to send `ClientCommand`s to the backend and from which to
/// receive `ClientEvent`s.
#[derive(Debug)]
pub struct BackendHandle {
    /// Sender for `ClientCommand`s.
    pub cmd_tx: Sender<ClientCommand>,
    /// Receiver of `ClientEvent`s.
    pub event_rx: Receiver<client_event::Result>,
}

/// The backend for the chat client. Frontends communicate with this via tokio channels by sending
/// `ClientCommand`s and receiving `ClientEvent`s.
///
/// To use the backend, first create it with `ChatBackend::new()`. Then, call the `run()` method.
/// For more information, see the documentation for those respective functions.
pub struct ChatBackend {
    tls_connector: TlsConnector,
    connection: Option<Connection>,
    cmd_rx: Receiver<ClientCommand>,
    event_tx: Sender<client_event::Result>,
}

impl ChatBackend {
    /// Create a new `ChatBackend` and a `BackendHandle` holding the necessary channels to
    /// communicate with the backend.
    ///
    /// The backend will attempt to read a config file from a reasonable, OS-specific default
    /// location: [`NamedProjectDirs`]`::new("client").config_dir().join("config.toml)`. This may
    /// optionally be overridden by passing a different path.
    ///
    /// If no config file is found at the given path, a default config will be generated and placed
    /// there. This will also attempt to create all requisite parent directories.
    ///
    /// # Errors
    /// See [`InitError`] for all possible errors from this function.
    pub fn new(config_path_override: Option<PathBuf>) -> Result<(Self, BackendHandle), InitError> {
        let default_paths = DefaultPaths::defaults("client");

        let config_path = first_match! {
            Some(overr) = config_path_override => overr,
            Some(defaults) = default_paths => defaults.config,
        };

        let mut figment = Figment::new().merge(Toml::string(DEFAULT_CONFIG));

        if let Some(path) = config_path {
            if path.exists() {
                figment = figment.merge(Toml::file(&path));
            } else {
                // Writing the default config file is best-effort. No error paths here return or
                // diverge.
                #[expect(clippy::collapsible_if)]
                if let Some(parent) = path.parent() {
                    if let Err(e) = create_dir_all(parent) {
                        // TODO: Log error
                    }
                }

                if let Err(e) = write(path, DEFAULT_CONFIG) {
                    // TODO: Log error
                }
            }
        }

        let config: Config = figment.extract()?;

        let mut root_cert_store = RootCertStore::empty();

        if config.include_webpki_roots {
            root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.to_vec());
        }

        for path in config.additional_root_ca_paths {
            let cert = CertificateDer::from_pem_file(path.resolved()?).map_err(|e| {
                InitError::CertFileReadFailed {
                    path: path.original().to_owned(),
                    source: e,
                }
            })?;

            root_cert_store.add(cert)?;
        }

        let tls_config = rustls::ClientConfig::builder()
            .with_root_certificates(root_cert_store)
            .with_no_client_auth();

        let tls_connector = TlsConnector::from(Arc::new(tls_config));

        let (cmd_tx, cmd_rx) = mpsc::channel::<ClientCommand>(128); // TODO: Buffer size
        let (event_tx, event_rx) = mpsc::channel::<client_event::Result>(128); // TODO: Buffer size

        let handle = BackendHandle { cmd_tx, event_rx };

        let backend = Self {
            tls_connector,
            connection: None,
            cmd_rx,
            event_tx,
        };

        Ok((backend, handle))
    }

    /// Start the backend.
    ///
    /// Because this is an asynchronous function, it is recommended, if possible, to spawn it as a
    /// task. However, if this is not possible (for example, if the frontend expects a synchronous
    /// event loop), one approach is to spawn it in a separate thread using `block_on`, then use
    /// the channels' blocking methods when sending to/receiving from the backend.
    #[allow(clippy::missing_panics_doc)]
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                event = async {
                    match self.connection.as_mut() {
                        Some(conn) => conn.receive_event().await,
                        None => std::future::pending().await,
                    }
                } => {
                    // If the server event is None, the server disconnected from us.
                    let Some(event) = event else {
                        self.send_ui_event(ClientEvent::ServerShutDown).await;
                        self.connection = None;
                        continue;
                    };

                    match event {
                        Ok(event) => self.handle_event(event).await,
                        Err(e) => {
                            self.send_ui_error(client_event::Error::Io(e)).await;
                            self.connection = None;
                            continue;
                        }
                    }
                }

                command = self.cmd_rx.recv() => {
                    if let Some(cmd) = command {
                        if let ControlFlow::Break(()) = self.handle_command(cmd).await {
                            break;
                        }
                    } else {
                        self.handle_ui_crash().await;
                        break;
                    }
                }
            }
        }

        self.shutdown().await;
    }

    /// Handle any necessary logic after a UI crash, but before shutting down. Note that
    /// `self.shutdown()` is always called when the application is closing.
    async fn handle_ui_crash(&mut self) {
        todo!("Handle UI crash")
    }

    /// Handle a `ClientCommand` coming from the frontend.
    async fn handle_command(&mut self, command: ClientCommand) -> ControlFlow<()> {
        match command {
            ClientCommand::Connect(host, port) => self.connect(host, port).await,
            ClientCommand::Disconnect => self.disconnect().await,
            ClientCommand::Quit => return ControlFlow::Break(()),
            ClientCommand::NetworkCommand(net_cmd) => self.send_network_command(net_cmd).await,
        }

        ControlFlow::Continue(())
    }

    /// Handle a `NetworkEvent` coming from the server.
    async fn handle_event(&mut self, event: NetworkEvent) {
        let event = match event.try_into() {
            Ok(event) => event,
            Err(e) => todo!("Log error"),
        };

        self.send_ui_event(event).await;
    }

    /// Attempt to connect to the server at `host:port`. The UI will be notified about whether the
    /// connection is successful or not.
    async fn connect(&mut self, host: String, port: Option<u16>) {
        let mut connection = match Connection::connect(&host, port, &self.tls_connector).await {
            Ok(conn) => conn,
            Err(e) => {
                self.send_ui_error(e.into()).await;
                return;
            }
        };

        if let Err(e) = connection.send_command(NetworkCommand::ClientHello).await {
            todo!("Log failed connection");
        }

        // We expect the server to send its Hello immediately after we send ours. Otherwise, we
        // cannot establish necessary basic state.
        let ServerHello {
            your_id,
            default_channel_id,
        } = match connection.receive_event().await {
            Some(Ok(NetworkEvent::ServerHello(hello))) => hello,
            Some(Ok(other)) => todo!("Log error: missed HELLO {other:?}"),
            Some(Err(e)) => todo!("Log error: event error {e}"),
            None => todo!("Log error: connection closed unexpectedly"),
        };

        // Fetch the channel list and initial user list. Currently, we treat this as a full,
        // automatic state dump. In future versions, this may be paginated and done lazily to
        // minimize network traffic.
        if let Err(e) = connection
            .send_command(NetworkCommand::FetchChannels(FetchChannels))
            .await
        {
            todo!("Log error: failed to request channels {e}")
        }

        if let Err(e) = connection
            .send_command(NetworkCommand::FetchUsers(FetchUsers))
            .await
        {
            todo!("Log error: failed to request users {e}")
        }

        let addr = connection.addr();
        self.connection = Some(connection);
        self.send_ui_event(ClientEvent::InitialSync(InitialSync {
            your_id,
            default_channel_id,
            server_addr: addr,
        }))
        .await;
    }

    /// Disconnect from the server.
    async fn disconnect(&mut self) {
        let Some(connection) = self.connection.take() else {
            // Disconnecting while already disconnected is a NOP
            return;
        };

        if let Err(e) = connection.disconnect().await {
            // TODO: Log error
        }

        // Even if the disconnect was not clean, by now, the connection has been consumed and
        // closed. As such, we unconditionally report success and only internally log the possible
        // error.
        self.send_ui_event(ClientEvent::Disconnected).await;
    }

    /// Send a `NetworkCommand` to the server. The UI will be notified if this fails.
    async fn send_network_command(&mut self, command: NetworkCommand) {
        let Some(connection) = &mut self.connection else {
            // TODO: Log error
            let kind = io::ErrorKind::NotConnected;
            let error = io::Error::from(kind);
            self.send_ui_error(error.into()).await;
            return;
        };

        if let Err(e) = connection.send_command(command).await {
            // TODO: Log error
            self.send_ui_error(e.into()).await;
        }
    }

    /// Send a `ClientEvent` to the UI.
    async fn send_ui_event(&mut self, event: ClientEvent) {
        if self.event_tx.send(Ok(event)).await.is_err() {
            self.handle_ui_crash().await;
            // TODO: Log error
        }
    }

    /// Send a `client_event::Error` to the UI.
    async fn send_ui_error(&mut self, error: client_event::Error) {
        // TODO: Log error
        if self.event_tx.send(Err(error)).await.is_err() {
            self.handle_ui_crash().await;
            // TODO: Log error
        }
    }

    /// Attempt a clean shutdown of the backend.
    async fn shutdown(mut self) {
        self.disconnect().await;
    }
}
