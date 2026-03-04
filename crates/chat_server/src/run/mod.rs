mod connection;

use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

use anyhow::Context;
use clap::Args;
use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use rustls::pki_types::{CertificateDer, pem::PemObject};
use serde::{Deserialize, Serialize};

use crate::{
    CONFIG_FILE_NAME, Config, DEFAULT_CONFIG, ENV_VAR_PREFIX, first_match, utils::get_project_dirs,
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

    /// Path to the TOML config file for the server.
    #[arg(long, value_name = "PATH", global = true)]
    config_file: Option<PathBuf>,
}

#[derive(Debug)]
struct ChatServer {
    config: Config,
}

impl ChatServer {
    fn new(config: Config) -> anyhow::Result<Self> {
        let address = SocketAddr::new(config.listener_ip, config.listener_port);

        let certs = CertificateDer::pem_file_iter(&config.cert_path)
            .context("ay")?
            .collect::<Result<Vec<_>, _>>()
            .context("uh")?;

        Ok(Self { config })
    }

    async fn run(mut self) -> anyhow::Result<()> {
        loop {
            break;
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

    if let Some(pd) = &project_dirs {
        figment = figment.merge(Serialized::default("cert_path", pd.data_dir()));
    }

    let config: Config = figment
        .merge(Env::prefixed(ENV_VAR_PREFIX))
        .merge(Serialized::defaults(args))
        .extract()
        .context("Resolving configuration")?;

    ChatServer::new(config)?.run().await
}
