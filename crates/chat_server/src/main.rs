mod default_paths;
mod init;
mod run;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

use default_paths::DefaultPaths;
use init::InitMode;
use run::RunArgs;

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let global_args = Cli::parse();

    let default_paths = DefaultPaths::defaults("server");

    match global_args.command {
        Commands::Run(args) => run::main(default_paths, args).await,
        Commands::Init(mode) => init::main(default_paths, mode),
    }
}
