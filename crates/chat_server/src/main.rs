mod init;
mod run;
mod utils;

use clap::{Parser, Subcommand};
use directories::ProjectDirs;
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
    listener_ip: String,
    listener_port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listener_ip: String::from("localhost"),
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

// TODO: Relocate this?
fn get_project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("rs", "UserOfNames", "my_chat")
}
