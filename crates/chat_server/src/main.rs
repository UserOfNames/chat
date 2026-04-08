mod init;
mod run;
mod utils;

use std::net::IpAddr;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

use init::InitMode;
use run::RunArgs;

static ENV_VAR_PREFIX: &str = "MY_CHAT_";
static CONFIG_FILE_NAME: &str = "config.toml";
static DEFAULT_CONFIG: &str = include_str!("../data/config.toml");

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
    tls_cert_path: PathBuf,
    tls_key_path: PathBuf,
    listener_ip: IpAddr,
    listener_port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let global_args = Cli::parse();

    match global_args.command {
        Commands::Run(args) => run::main(args).await,
        Commands::Init(mode) => init::main(mode),
    }
}
