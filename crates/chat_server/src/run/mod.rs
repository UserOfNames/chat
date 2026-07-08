mod connection;
mod listener;
mod server_state;

use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};

use anyhow::{Context, bail};
use clap::Args;
use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use network_protocol::{ChannelInfo, NetworkEvent, UserInfo};
use rustls::{
    ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
};
use serde::{Deserialize, Serialize};
use shared_utils::{TildeRelativePathBuf, first_match};
use tokio::sync::{broadcast, mpsc};
use tokio_rustls::TlsAcceptor;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use listener::Listener;
use server_state::ServerState;
use tracing::{debug, info, instrument};
use tracing_appender::{non_blocking::WorkerGuard, rolling};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{DEFAULT_CONFIG, DefaultPaths, ENV_VAR_PREFIX};

#[derive(Debug, Args, Serialize, Deserialize)]
pub struct RunArgs {
    /// Path to the TOML config file for the server
    #[arg(long, value_name = "PATH")]
    config_file: Option<PathBuf>,

    /// The address the TCP listener binds to
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    listener_ip: Option<IpAddr>,

    /// The port the TCP listener binds to
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    listener_port: Option<u16>,

    /// Path to the server's TLS certificate
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    tls_cert_path: Option<PathBuf>,

    /// Path to the server's private TLS key
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    tls_key_path: Option<PathBuf>,

    /// Maximum allowed length of users' display names
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    max_username_length: Option<usize>,

    /// Whether to write logs to standard output
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    log_to_stdout: Option<bool>,

    /// Whether to write logs to a file
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    log_to_file: Option<bool>,

    /// Directory to store the log file if `log_to_file` is true
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    log_dir: Option<PathBuf>,
}

/// Configuration for the server runtime.
#[derive(Debug, Serialize, Deserialize)]
struct Config {
    /// Host address the listener task binds to.
    listener_ip: IpAddr,

    /// Port the listener task binds to.
    listener_port: u16,

    /// Path to the TLS certificate file.
    tls_cert_path: TildeRelativePathBuf,

    /// Path to the TLS private key file associated with the certificate.
    tls_key_path: TildeRelativePathBuf,

    /// Maximum allowed length of users' display names.
    max_username_length: usize,

    /// Whether to write logs to standard output.
    log_to_stdout: bool,

    /// Whether to write logs to a file.
    log_to_file: bool,

    /// Directory to store the log file if `log_to_file` is true.
    log_dir: PathBuf,

    /// List of all the channels on the server. Includes channels' IDs and names.
    channels: Vec<ChannelInfo>,
}

/// Represents a connected user.
#[derive(Debug, Clone)]
struct User {
    pub info: UserInfo,
    pub sender: mpsc::Sender<NetworkEvent>,
}

/// Represents a channel.
#[derive(Debug, Clone)]
struct Channel {
    pub info: ChannelInfo,
    pub broadcast: broadcast::Sender<NetworkEvent>,
}

/// A chat server. To start the server, first initialize it with `new()`. Then, call `run()`.
struct ChatServer {
    bind_address: SocketAddr,
    tls_acceptor: TlsAcceptor,
    server_state: Arc<ServerState>,
    task_tracker: TaskTracker,
}

impl ChatServer {
    #[instrument(skip_all, err)]
    // TODO: `async fn new` is an antipattern. This whole function is getting bloated in general;
    // refactor the whole thing (and make it synchronous).
    async fn new(config: Config) -> anyhow::Result<Self> {
        let bind_address = SocketAddr::new(config.listener_ip, config.listener_port);
        debug!(ip = %config.listener_ip, port = %config.listener_port, "Resolved bind address");

        let cert_path_err_display = config.tls_cert_path.original().display();
        let tls_cert_path = &config
            .tls_cert_path
            .resolved()
            .context("Resolving TLS key path")?;
        let certs = CertificateDer::pem_file_iter(tls_cert_path)
            .with_context(|| format!("Opening TLS certificate file '{cert_path_err_display}'"))?
            .collect::<Result<Vec<_>, _>>()
            .with_context(|| format!("Reading TLS certificate file '{cert_path_err_display}'"))?;

        let key_path_err_display = config.tls_key_path.original().display();
        let tls_key_path = &config
            .tls_key_path
            .resolved()
            .context("Resolving TLS key path")?;
        let key = PrivateKeyDer::from_pem_file(tls_key_path)
            .with_context(|| format!("Reading TLS key file '{key_path_err_display}'"))?;

        debug!(
            cert_path = %tls_cert_path.display(),
            key_path = %tls_key_path.display(),
            "Loaded TLS keypair"
        );

        let tls_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .context("Configuring TLS: bad certificate or key")?;

        let tls_acceptor = TlsAcceptor::from(Arc::new(tls_config));

        let default_channel_id = config.channels.first().map(|inner| inner.id);
        let server_state = Arc::new(ServerState::new(
            default_channel_id,
            config.max_username_length,
        ));

        for channel_info in config.channels {
            let (tx, _rx) = broadcast::channel(128); // TODO: Buffer size

            debug!(
                channel_id = %channel_info.id,
                channel_name = %channel_info.name,
                "Registering channel"
            );

            if let Err(e) = server_state
                .add_channel(channel_info.id, channel_info.name, tx)
                .await
            {
                bail!("Failed to initialize channels - {e}");
            }
        }

        info!("Initialized server state");
        Ok(Self {
            bind_address,
            tls_acceptor,
            server_state,
            task_tracker: TaskTracker::new(),
        })
    }

    #[instrument(skip_all, err)]
    async fn run(self) -> anyhow::Result<()> {
        let cancellation_token = CancellationToken::new();

        let listener = Listener::new(
            self.server_state.clone(),
            cancellation_token.clone(),
            self.task_tracker.clone(),
            self.tls_acceptor.clone(),
            self.bind_address,
        );

        self.task_tracker.spawn(listener.start());

        tokio::signal::ctrl_c()
            .await
            .context("Failed to listen for 'Ctrl-C' signal")?;

        info!("Interruption signal received, shutting down...");
        cancellation_token.cancel();

        self.task_tracker.close();

        debug!("Waiting for tasks to finish...");
        self.task_tracker.wait().await;

        info!("Server shut down gracefully");
        Ok(())
    }
}

pub async fn main(default_paths: Option<DefaultPaths>, args: RunArgs) -> anyhow::Result<()> {
    let env_conf_path = std::env::var(format!("{ENV_VAR_PREFIX}CONFIG_FILE"))
        .ok()
        .map(PathBuf::from);

    let config_path = first_match! {
        Some(path) = &args.config_file => path.clone(),
        Some(path) = env_conf_path => path,
        Some(defaults) = &default_paths => defaults.config.clone(),
    };

    let mut figment = Figment::new().merge(Toml::string(DEFAULT_CONFIG));

    if let Some(path) = &config_path {
        figment = figment.merge(Toml::file(path));
    }

    if let Some(defaults) = &default_paths {
        figment = figment.merge(Serialized::default("tls_cert_path", &defaults.server_cert));
        figment = figment.merge(Serialized::default("tls_key_path", &defaults.server_key));
        figment = figment.merge(Serialized::default("log_dir", &defaults.log_dir));
    }

    let config: Config = figment
        .merge(Env::prefixed(ENV_VAR_PREFIX))
        .merge(Serialized::defaults(args))
        .extract()
        .context("Resolving configuration")?;

    let _log_file_guard = init_logging(&config);

    debug!(config_path = ?config_path, "Configuration resolved");

    if config.log_to_file {
        info!(log_dir = %config.log_dir.display(), "Background file logging enabled");
    }

    info!("Starting server");
    ChatServer::new(config)
        .await
        .context("Initializing server")?
        .run()
        .await
}

fn init_logging(config: &Config) -> Option<WorkerGuard> {
    let stdout_layer = config.log_to_stdout.then(tracing_subscriber::fmt::layer);

    let (file_layer, file_guard) = if config.log_to_file {
        let appender = rolling::daily(&config.log_dir, "server.log");
        let (appender, guard) = tracing_appender::non_blocking(appender);

        let layer = tracing_subscriber::fmt::layer()
            .with_writer(appender)
            .with_ansi(false);

        (Some(layer), Some(guard))
    } else {
        (None, None)
    };

    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(file_layer)
        .init();

    file_guard
}
