mod connection;
mod listener;
mod server_state;

use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};

use anyhow::Context;
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
    fn new(config: Config) -> anyhow::Result<Self> {
        let bind_address = SocketAddr::new(config.listener_ip, config.listener_port);

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

            if let Err(e) = server_state.add_channel(channel_info.id, channel_info.name, tx) {
                todo!("Log error and return: {e}");
            }
        }

        Ok(Self {
            bind_address,
            tls_acceptor,
            server_state,
            task_tracker: TaskTracker::new(),
        })
    }

    #[allow(unused_mut)]
    async fn run(mut self) -> anyhow::Result<()> {
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

        cancellation_token.cancel();

        self.task_tracker.close();
        self.task_tracker.wait().await;

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

    if let Some(path) = config_path {
        figment = figment.merge(Toml::file(path));
    }

    if let Some(defaults) = &default_paths {
        figment = figment.merge(Serialized::default("tls_cert_path", &defaults.server_cert));
        figment = figment.merge(Serialized::default("tls_key_path", &defaults.server_key));
    }

    let config: Config = figment
        .merge(Env::prefixed(ENV_VAR_PREFIX))
        .merge(Serialized::defaults(args))
        .extract()
        .context("Resolving configuration")?;

    ChatServer::new(config)
        .context("Initializing server")?
        .run()
        .await
}
