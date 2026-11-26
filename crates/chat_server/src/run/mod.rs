mod channels;
mod connection;
mod listener;
mod server_state;
mod users;

use std::{
    borrow::Cow,
    env,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, ensure};
use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use network_protocol::ChannelInfo;
use rustls::{
    ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
};
use serde::{Deserialize, Serialize};
use shared_utils::files::TildeRelativePathBuf;
use tokio_rustls::TlsAcceptor;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use listener::Listener;
use server_state::ServerState;
use tracing::{debug, info, instrument};
use tracing_appender::{non_blocking::WorkerGuard, rolling};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::{DEFAULT_CONFIG, ENV_VAR_PREFIX, default_paths::DefaultPaths};

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

    /// Log to the given directory. If unset, don't use file logging.
    log_dir: Option<TildeRelativePathBuf>,

    /// List of all the channels on the server. Includes channels' IDs and names.
    channels: Vec<ChannelInfo>,
}

impl Config {
    fn resolve_path<'a>(
        args: &'a Args,
        default_paths: Option<&'a DefaultPaths>,
    ) -> anyhow::Result<Option<Cow<'a, Path>>> {
        if let Some(path) = args.config_file.as_deref() {
            ensure!(
                path.exists(),
                "Path '{}' was given via config file, but did not exist",
                path.display()
            );
            return Ok(Some(Cow::Borrowed(path)));
        }

        if let Ok(path) = env::var(format!("{ENV_VAR_PREFIX}CONFIG_FILE")).map(PathBuf::from) {
            ensure!(
                path.exists(),
                "Path '{}' was given via environment variable, but did not exist",
                path.display()
            );
            return Ok(Some(Cow::Owned(path)));
        }

        if let Some(defaults) = default_paths {
            return Ok(Some(Cow::Borrowed(defaults.config())));
        }

        Ok(None)
    }

    fn load(
        args: &Args,
        default_paths: Option<&DefaultPaths>,
        config_path: Option<impl AsRef<Path>>,
    ) -> anyhow::Result<Config> {
        let mut figment = Figment::new().merge(Toml::string(DEFAULT_CONFIG));

        if let Some(defaults) = &default_paths {
            figment = figment.merge(Serialized::default("tls_cert_path", defaults.server_cert()));
            figment = figment.merge(Serialized::default("tls_key_path", defaults.server_key()));
        }

        if let Some(path) = config_path {
            figment = figment.merge(Toml::file(path));
        }

        figment
            .merge(Env::prefixed(ENV_VAR_PREFIX))
            .merge(Serialized::defaults(args))
            .extract()
            .context("Resolving configuration")
    }
}

/// A chat server. To start the server, first initialize it with `try_from_config()`. Then, call
/// `start()`.
struct ChatServer {
    bind_address: SocketAddr,
    tls_acceptor: TlsAcceptor,
    server_state: Arc<ServerState>,
    task_tracker: TaskTracker,
}

impl ChatServer {
    #[instrument(skip_all)]
    fn try_from_config(config: Config) -> anyhow::Result<Self> {
        let bind_address = SocketAddr::new(config.listener_ip, config.listener_port);
        debug!(ip = %config.listener_ip, port = %config.listener_port, "Resolved bind address");

        let (certs, key) = Self::get_tls_keypair(&config.tls_cert_path, &config.tls_key_path)?;

        let tls_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .context("Configuring TLS: bad certificate or key")?;

        let tls_acceptor = TlsAcceptor::from(Arc::new(tls_config));

        let server_state =
            ServerState::new_with_channels(config.max_username_length, config.channels)
                .context("Initializing server state")?;
        let server_state = Arc::new(server_state);

        Ok(Self {
            bind_address,
            tls_acceptor,
            server_state,
            task_tracker: TaskTracker::new(),
        })
    }

    fn get_tls_keypair(
        cert_path: &TildeRelativePathBuf,
        key_path: &TildeRelativePathBuf,
    ) -> anyhow::Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
        let cert_path_err_display = cert_path.original().display();
        let cert_path = cert_path
            .resolved()
            .context("Resolving TLS certificate path")?;
        let certs = CertificateDer::pem_file_iter(&cert_path)
            .with_context(|| format!("Opening TLS certificate file '{cert_path_err_display}'"))?
            .collect::<Result<Vec<_>, _>>()
            .with_context(|| format!("Reading TLS certificate file '{cert_path_err_display}'"))?;

        let key_path_err_display = key_path.original().display();
        let key_path = key_path.resolved().context("Resolving TLS key path")?;
        let key = PrivateKeyDer::from_pem_file(&key_path)
            .with_context(|| format!("Reading TLS key file '{key_path_err_display}'"))?;

        debug!(
            cert_path = %cert_path.display(),
            key_path = %key_path.display(),
            "Loaded TLS keypair"
        );

        Ok((certs, key))
    }

    #[instrument(skip_all)]
    async fn start(self) -> anyhow::Result<()> {
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

#[derive(Debug, clap::Args, Serialize, Deserialize)]
pub struct Args {
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

    /// Log to the given directory. If unset, don't use file logging.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    log_dir: Option<PathBuf>,
}

impl Args {
    pub async fn run(self, default_paths: Option<DefaultPaths>) -> anyhow::Result<()> {
        let config_path =
            Config::resolve_path(&self, default_paths.as_ref()).context("Resolving config path")?;

        let config = Config::load(&self, default_paths.as_ref(), config_path.as_ref())
            .context("Loading config")?;

        let _log_file_guard = init_logging(&config)?;

        debug!(?config, "Loaded config");

        let server = ChatServer::try_from_config(config).context("Initializing server")?;
        info!("Server initialized; starting");
        server.start().await
    }
}

fn init_logging(config: &Config) -> anyhow::Result<Option<WorkerGuard>> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let stdout_layer = config.log_to_stdout.then(tracing_subscriber::fmt::layer);

    let (file_layer, file_guard) = if let Some(log_dir) = config.log_dir.as_ref() {
        let path = log_dir
            .resolved()
            .context("Resolving relative log dir path")?;

        let appender = rolling::daily(path, "server.log");
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
        .with(filter)
        .init();

    Ok(file_guard)
}
