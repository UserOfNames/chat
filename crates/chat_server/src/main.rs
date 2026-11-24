mod default_paths;
mod init;
mod run;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

use default_paths::DefaultPaths;

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
    Init(init::Mode),

    /// Start the server
    Run(run::Args),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    let default_paths = DefaultPaths::defaults("server");

    match args.command {
        Commands::Init(mode) => mode.run(default_paths),
        Commands::Run(args) => args.run(default_paths).await,
    }
}
