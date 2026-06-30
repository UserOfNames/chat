mod init;
mod run;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

use init::InitMode;
use run::RunArgs;
use shared_utils::NamedProjectDirs;

const ENV_VAR_PREFIX: &str = "MY_CHAT_";
const DEFAULT_CONFIG: &str = include_str!("../data/config.toml");

#[allow(clippy::option_option)]
#[derive(Debug, Parser, Serialize, Deserialize)]
#[command(author = "UserOfNames", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand, Serialize, Deserialize)]
enum Commands {
    /// Initialize essential state for the server
    #[command(subcommand)]
    Init(InitMode),

    /// Start the server
    Run(RunArgs),
}

/// Collection of relevant paths for the server to read or initialize important files.
#[derive(Debug)]
struct DefaultPaths {
    config: PathBuf,
    ca_cert: PathBuf,
    ca_key: PathBuf,
    server_cert: PathBuf,
    server_key: PathBuf,
    log_dir: PathBuf,
}

impl DefaultPaths {
    /// Initialize a `ServerPaths` instance with default paths.
    ///
    /// `config`: `NamedProjectDirs::config_dir()/config.toml`
    /// `ca_cert`: `NamedProjectDirs::data_dir()/tls/ca/certificate.pem`
    /// `ca_key`: `NamedProjectDirs::data_dir()/tls/ca/key.pem`
    /// `server_cert`: `NamedProjectDirs::data_dir()/tls/server/certificate.pem`
    /// `server_key`: `NamedProjectDirs::data_dir()/tls/server/key.pem`
    /// `log_file`: `NamedProjectDirs::state_dir()/server.log`
    fn defaults(component: impl Into<PathBuf>) -> Option<Self> {
        let base = NamedProjectDirs::new(component)?;

        let config = base.config_dir().join("config.toml");

        let ca_dir = base.data_dir().join("tls").join("ca");
        let ca_cert = ca_dir.join("certificate.pem");
        let ca_key = ca_dir.join("key.pem");

        let server_cert_dir = base.data_dir().join("tls").join("server");
        let server_cert = server_cert_dir.join("certificate.pem");
        let server_key = server_cert_dir.join("key.pem");

        let log_dir = base.state_dir().to_owned();

        Some(Self {
            config,
            ca_cert,
            ca_key,
            server_cert,
            server_key,
            log_dir,
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let global_args = Cli::parse();

    let default_paths = DefaultPaths::defaults("server");

    match global_args.command {
        Commands::Run(args) => run::main(default_paths, args).await,
        Commands::Init(mode) => init::main(default_paths, mode),
    }
}
