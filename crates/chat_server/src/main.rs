mod init;
mod run;

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

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
    Run(run::RunArgs),
}

#[derive(Debug, Subcommand, Serialize, Deserialize)]
enum InitMode {
    /// Initialize a default config file
    Config(init::ConfigArgs),
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
        Commands::Run(args) => run::main(args),
        Commands::Init(mode) => init::main(mode),
    }
}

// TODO: Relocate this?
fn get_project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("rs", "UserOfNames", "my_chat")
}

// TODO: Relocate this?
fn get_config_path(project_dirs: Option<&ProjectDirs>, other: &[Option<&Path>]) -> Option<PathBuf> {
    other
        .iter()
        .flatten()
        .next()
        .map(|&x| x.to_owned())
        .or_else(|| project_dirs.map(|dirs| dirs.config_dir().join(CONFIG_FILE_NAME)))
}
