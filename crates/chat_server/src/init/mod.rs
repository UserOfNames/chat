mod certs;
mod config;

use clap::Subcommand;
use serde::{Deserialize, Serialize};

use certs::{InitCertsArgs, init_certs};
use config::{InitConfigArgs, init_config};

#[derive(Debug, Subcommand, Serialize, Deserialize)]
pub enum InitMode {
    /// Initialize certificates and keys for TLS
    Certs(InitCertsArgs),
    /// Initialize a default config file
    Config(InitConfigArgs),
}

pub fn main(mode: InitMode) -> anyhow::Result<()> {
    match mode {
        InitMode::Certs(args) => init_certs(args),
        InitMode::Config(args) => init_config(args),
    }
}
