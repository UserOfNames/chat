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
use std::path::{Path, PathBuf};
use std::sync::Arc;

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
use thiserror::Error;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio_rustls::TlsConnector;

use client_command::{ClientCommand, ConnectParams};
use client_event::ClientEvent;
use connection::Connection;
use network_protocol::{
    ClientHello, FetchChannels, FetchUsers, NetworkCommand, NetworkEvent, ServerHello,
};
use shared_utils::{NamedProjectDirs, TildeRelativePathBuf, first_match};
use tracing::{debug, error, info, instrument, warn};

use crate::client_event::InitialSync;

const DEFAULT_CONFIG: &str = include_str!("../data/config.toml");

#[derive(Debug, Error)]
pub enum InitError {
    /// The config path was overridden, but the target path does not exist.
    #[error("Overridden config path does not exist: '{0}'")]
    OverridePathDoesNotExist(PathBuf),

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
    #[instrument(skip_all, err)]
    pub fn new(config_path_override: Option<PathBuf>) -> Result<(Self, BackendHandle), InitError> {
        debug!("Initializing client backend");

        let mut figment = Figment::new().merge(Toml::string(DEFAULT_CONFIG));

        let default_paths = DefaultPaths::defaults("client");

        first_match! {
            Some(override_path) = config_path_override => {
                if !override_path.exists() {
                    return Err(InitError::OverridePathDoesNotExist(override_path));
                }

                figment = figment.merge(Toml::file(&override_path));
                debug!(config_path = %override_path.display(), "Overriden config path resolved");
            },

            Some(defaults) = default_paths => {
                let default_path = defaults.config;

                if default_path.exists() {
                    figment = figment.merge(Toml::file(&default_path));
                    debug!(config_path = %default_path.display(), "Default config path resolved");
                } else {
                    Self::try_to_write_config_file(&default_path);
                    debug!(config_path = %default_path.display(), "Wrote default config to disk");
                }
            },
        };

        let config: Config = figment.extract()?;
        info!("Config resolved");

        let mut root_cert_store = RootCertStore::empty();

        if config.include_webpki_roots {
            root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.to_vec());
        }

        for path in config.additional_root_ca_paths {
            debug!(path = %path.original().display(), "Loading additional root CA cert");

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

    /// Attempt to write the default config file. This is best-effort; if an error occurs, we log
    /// and swallow it.
    #[instrument(skip_all, fields(path = %path.display()))]
    fn try_to_write_config_file(path: &Path) {
        if let Some(parent) = path.parent()
            && let Err(e) = create_dir_all(parent)
        {
            warn!(
                error = %e,
                parent_path = %parent.display(),
                "Could not create parent directory for default config file"
            );
        }

        if let Err(e) = write(path, DEFAULT_CONFIG) {
            warn!(
                error = %e,
                "Could not write default config file"
            );
        }
    }

    /// Start the backend.
    ///
    /// Because this is an asynchronous function, it is recommended, if possible, to spawn it as a
    /// task. However, if this is not possible (for example, if the frontend expects a synchronous
    /// event loop), one approach is to spawn it in a separate thread using `block_on`, then use
    /// the channels' blocking methods when sending to/receiving from the backend.
    pub async fn run(mut self) {
        'backend: loop {
            tokio::select! {
                // This structure is a bit odd, but it's necessary. We need this `select!` arm to
                // listen for an event only if we're connected to the server. If we aren't
                // connected, we want to skip the arm entirely. Therefore, if `self.connection` is
                // `None`, we trigger a future that never resolves.
                event = async {
                    match self.connection.as_mut() {
                        Some(conn) => conn.receive_event().await,
                        None => std::future::pending().await,
                    }
                } => {
                    // If the server event is None, the server disconnected from us.
                    let Some(event) = event else {
                        info!("Server disconnected unexpectedly");
                        self.send_ui_event(ClientEvent::ServerShutDown).await;
                        self.connection = None;
                        continue 'backend;
                    };

                    match event {
                        Ok(event) => {
                            self.handle_event(event).await;
                        }

                        Err(e) => {
                            warn!(error = %e, "Error reading event from server");
                            self.send_ui_error(client_event::Error::Io(e)).await;
                            self.connection = None;
                            continue 'backend;
                        }
                    }
                }

                command = self.cmd_rx.recv() => {
                    let Some(command) = command else {
                        self.handle_ui_crash();
                        break 'backend;
                    };

                    if self.handle_command(command).await.is_break() {
                        break 'backend;
                    }
                }
            }
        }

        self.shutdown().await;
    }

    /// Handle any necessary logic after a UI crash, but before shutting down. Note that
    /// `self.shutdown()` is always called when the application is closing.
    #[expect(clippy::unused_self)]
    fn handle_ui_crash(&mut self) {
        error!("UI crashed");
    }

    /// Handle a `ClientCommand` coming from the frontend.
    async fn handle_command(&mut self, command: ClientCommand) -> ControlFlow<()> {
        match command {
            ClientCommand::Connect(params) => {
                info!(
                    host = %params.host,
                    port = ?params.port,
                    initial_username = %params.initial_username,
                    "Command received: connecting to server"
                );

                self.connect(params).await;
            }

            ClientCommand::Disconnect => {
                info!("Command received: disconnecting from server");

                self.disconnect().await;
            }

            ClientCommand::Quit => {
                info!("Command received: quitting");

                return ControlFlow::Break(());
            }

            ClientCommand::NetworkCommand(net_cmd) => self.send_network_command(net_cmd).await,
        }

        ControlFlow::Continue(())
    }

    /// Handle a `NetworkEvent` coming from the server.
    #[instrument(skip_all, fields(event = %event.name()))]
    async fn handle_event(&mut self, event: NetworkEvent) {
        #[allow(clippy::single_match_else)]
        let event: ClientEvent = match event.try_into() {
            Ok(event) => event,
            Err(()) => {
                warn!("Received invalid event from server, could not convert to client event");
                return;
            }
        };

        debug!("Received event from server");
        self.send_ui_event(event).await;
    }

    /// Attempt to connect to the server at using the given `ConnectParams`. The UI will be notified
    /// about whether the connection is successful or not.
    #[instrument(skip_all, fields(
        host = %params.host,
        port = ?params.port,
    ))]
    async fn connect(&mut self, params: ConnectParams) {
        let mut connection =
            match Connection::connect(&params.host, params.port, &self.tls_connector).await {
                Ok(conn) => conn,
                Err(e) => {
                    warn!(error = %e, "Failed to establish TCP+TLS connection to server");
                    self.send_ui_error(e.into()).await;
                    return;
                }
            };

        debug!("Established TCP+TLS connection to server");

        let client_hello = ClientHello {
            requested_name: params.initial_username,
        };

        if let Err(e) = connection
            .send_command(NetworkCommand::ClientHello(client_hello))
            .await
        {
            warn!(error = %e, "Failed to send Hello to server");
        }

        // We expect the server to send its Hello immediately after we send ours. Otherwise, we
        // cannot establish necessary basic state.
        let ServerHello {
            your_id,
            default_channel_id,
        } = match connection.receive_event().await {
            Some(Ok(NetworkEvent::ServerHello(hello))) => hello,

            Some(Ok(other)) => {
                warn!(
                    ?other,
                    "Failed to connect to server - missed server Hello, got unexpected command"
                );
                return;
            }

            Some(Err(e)) => {
                warn!(error = %e, "Failed to connect to server - IO error");
                return;
            }

            None => {
                warn!("Failed to connect to server - connection closed unexpectedly");
                return;
            }
        };
        debug!(our_id = %your_id, "Received server Hello");

        // Fetch the channel list and initial user list. Currently, we treat this as a full,
        // automatic state dump. In future versions, this may be paginated and done lazily to
        // minimize network traffic.
        if let Err(e) = connection
            .send_command(NetworkCommand::FetchChannels(FetchChannels))
            .await
        {
            warn!(error = %e, "Failed to connect to server - could not fetch channels");
            return;
        }

        if let Err(e) = connection
            .send_command(NetworkCommand::FetchUsers(FetchUsers))
            .await
        {
            warn!(error = %e, "Failed to connect to server - could not fetch users");
            return;
        }

        debug!("Fetched channels and users");

        let server_addr = connection.addr();

        self.connection = Some(connection);
        // At this point, the connection has succeeded. While we may immediately experience a UI
        // crash and promptly disconnect, that's a subsequent event. At this point, the connection
        // is done.
        debug!("Connection succeeded");

        self.send_ui_event(ClientEvent::InitialSync(InitialSync {
            your_id,
            default_channel_id,
            server_addr,
        }))
        .await;
    }

    /// Disconnect from the server.
    #[instrument(skip_all, fields(
        connection_address = ?self.connection.as_ref().map(Connection::addr),
    ))]
    async fn disconnect(&mut self) {
        let Some(connection) = self.connection.take() else {
            // Disconnecting while already disconnected is a NOP
            return;
        };

        if let Err(e) = connection.disconnect().await {
            warn!(error = %e, "Failed to disconnect cleanly. Disconnection will still proceed");
        }

        // Even if the disconnect was not clean, by now, the connection has been consumed and
        // closed. As such, we unconditionally report success and only internally log the possible
        // error.
        self.send_ui_event(ClientEvent::Disconnected).await;
    }

    /// Send a `NetworkCommand` to the server. The UI will be notified if this fails.
    async fn send_network_command(&mut self, command: NetworkCommand) {
        let Some(connection) = &mut self.connection else {
            warn!("Tried to send a command, but there's no active connection");
            let error = io::Error::from(io::ErrorKind::NotConnected);
            self.send_ui_error(error.into()).await;
            return;
        };

        if let Err(e) = connection.send_command(command).await {
            warn!(error = %e, "Failed to send command to server");
            self.send_ui_error(e.into()).await;
        }
    }

    /// Send a `ClientEvent` to the UI.
    async fn send_ui_event(&mut self, event: ClientEvent) {
        if self.event_tx.send(Ok(event)).await.is_err() {
            self.handle_ui_crash();
        }
    }

    /// Send a `client_event::Error` to the UI.
    async fn send_ui_error(&mut self, error: client_event::Error) {
        if self.event_tx.send(Err(error)).await.is_err() {
            self.handle_ui_crash();
        }
    }

    /// Attempt a clean shutdown of the backend.
    async fn shutdown(mut self) {
        self.disconnect().await;
    }
}
