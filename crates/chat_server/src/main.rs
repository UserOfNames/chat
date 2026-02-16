mod init;
mod run;
mod utils;

use std::{net::{IpAddr, Ipv4Addr}, path::PathBuf};

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

use init::InitMode;
use run::RunArgs;

static ENV_VAR_PREFIX: &str = "MY_CHAT_";
static CONFIG_FILE_NAME: &str = "config.toml";

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

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    cert_path: PathBuf,
    listener_ip: IpAddr,
    listener_port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cert_path: PathBuf::default(),
            listener_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            listener_port: 12345,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let global_args = Cli::parse();

    match global_args.command {
        Commands::Run(args) => run::main(args).await,
        Commands::Init(mode) => init::main(mode),
    }
}
