mod connection;

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
use network_protocol::codecs::ServerCodec;
use rustls::{
    ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_util::codec::Framed;

use crate::{
    CONFIG_FILE_NAME, Config, DEFAULT_CONFIG, ENV_VAR_PREFIX, first_match,
    utils::{get_project_dirs, get_tls_server_dir},
};

use connection::Connection;

#[derive(Debug, Args, Serialize, Deserialize)]
pub struct RunArgs {
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

    /// Path to the TOML config file for the server
    #[arg(long, value_name = "PATH")]
    config_file: Option<PathBuf>,
}

struct ChatServer {
    config: Config,
    acceptor: TlsAcceptor,
    listener: TcpListener,
}

impl ChatServer {
    async fn new(config: Config) -> anyhow::Result<Self> {
        let address = SocketAddr::new(config.listener_ip, config.listener_port);

        let listener = TcpListener::bind(address)
            .await
            .with_context(|| format!("Binding TCP listener to address '{address}'"))?;

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

        let acceptor = TlsAcceptor::from(Arc::new(tls_config));

        Ok(Self {
            config,
            acceptor,
            listener,
        })
    }

    async fn run(mut self) -> anyhow::Result<()> {
        loop {
            // TODO: error handling
            let (stream, _addr) = self.listener.accept().await.unwrap();
            let stream = self.acceptor.accept(stream).await.unwrap();
            let stream = Framed::new(stream, ServerCodec);
            todo!();
        }

        Ok(())
    }
}

pub async fn main(args: RunArgs) -> anyhow::Result<()> {
    let project_dirs = get_project_dirs();
    let env_conf_path = std::env::var(format!("{ENV_VAR_PREFIX}CONFIG_FILE"))
        .ok()
        .map(PathBuf::from);

    let config_path = first_match! {
        Some(path) = &args.config_file => path.clone(),
        Some(path) = env_conf_path => path,
        Some(pd) = &project_dirs => pd.config_dir().join(CONFIG_FILE_NAME),
    };

    let mut figment = Figment::new().merge(Toml::string(DEFAULT_CONFIG));

    if let Some(path) = config_path {
        figment = figment.merge(Toml::file(path));
    }

    if let Some(default_tls_path) = &get_tls_server_dir() {
        let cert_path = default_tls_path.join("certificate.pem");
        let key_path = default_tls_path.join("key.pem");
        figment = figment.merge(Serialized::default("tls_cert_path", cert_path));
        figment = figment.merge(Serialized::default("tls_key_path", key_path));
    }

    let config: Config = figment
        .merge(Env::prefixed(ENV_VAR_PREFIX))
        .merge(Serialized::defaults(args))
        .extract()
        .context("Resolving configuration")?;

    ChatServer::new(config)
        .await
        .context("Initializing server")?
        .run()
        .await
}
