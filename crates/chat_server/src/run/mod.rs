mod connection;
mod listener;

use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};

use anyhow::Context;
use clap::Args;
use dashmap::DashMap;
use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use network_protocol::{ChannelId, NetworkEvent, UserId};
use rustls::{
    ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
};
use serde::{Deserialize, Serialize};
use shared_utils::first_match;
use tokio::sync::{broadcast, mpsc};
use tokio_rustls::TlsAcceptor;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use listener::Listener;

use crate::{Config, DEFAULT_CONFIG, DefaultPaths, ENV_VAR_PREFIX};

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

    /// List of all the channel IDs on the server
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long, value_name = "PATH")]
    channel_ids: Option<Vec<ChannelId>>,
}

/// State shared between all tasks.
#[derive(Debug)]
struct ServerState {
    default_channel_id: Option<ChannelId>,
    channels: DashMap<ChannelId, broadcast::Sender<NetworkEvent>>,
    users: DashMap<UserId, mpsc::Sender<NetworkEvent>>,
}

impl ServerState {
    /// Initialize a `ServerState` instance.
    fn new(default_channel_id: Option<ChannelId>) -> Self {
        Self {
            default_channel_id,
            channels: DashMap::new(),
            users: DashMap::new(),
        }
    }
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

        let cert_path_display = config.tls_cert_path.display();
        let certs = CertificateDer::pem_file_iter(&config.tls_cert_path)
            .with_context(|| format!("Opening TLS certificate file '{cert_path_display}'"))?
            .collect::<Result<Vec<_>, _>>()
            .with_context(|| format!("Reading TLS certificate file '{cert_path_display}'"))?;

        let key_path_display = config.tls_key_path.display();
        let key = PrivateKeyDer::from_pem_file(&config.tls_key_path)
            .with_context(|| format!("Reading TLS key file '{key_path_display}'"))?;

        let tls_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .context("Configuring TLS: bad certificate or key")?;

        let tls_acceptor = TlsAcceptor::from(Arc::new(tls_config));

        let default_channel_id = config.channel_ids.first().cloned();
        let server_state = Arc::new(ServerState::new(default_channel_id));

        for channel_id in config.channel_ids {
            let (tx, _rx) = broadcast::channel(128); // TODO: Buffer size
            server_state.channels.insert(channel_id, tx);
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

        self.task_tracker
            .spawn(async move { listener.start().await });

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
