use std::path::PathBuf;

use anyhow::Context;
use clap::Args;
use serde::{Deserialize, Serialize};
use shared_utils::first_match;

use crate::{DEFAULT_CONFIG, DefaultPaths};

use super::{WriteParams, write_with_params};

#[derive(Debug, Args, Serialize, Deserialize)]
pub struct InitConfigArgs {
    /// Path to initialize the config file
    /// If not provided, it will default to the standard OS directory.
    #[arg(short, long)]
    path: Option<PathBuf>,

    /// Overwrite the existing config file at the path
    #[arg(short, long)]
    force: bool,

    /// Print the target path for the config file and exit without creating it
    #[arg(long)]
    dry_run: bool,
}

pub fn init_config(default_paths: Option<DefaultPaths>, args: InitConfigArgs) -> anyhow::Result<()> {
    let config_path = first_match! {
        Some(path) = &args.path => path,
        Some(defaults) = &default_paths => &defaults.config,
    }
    .context("Resolving config file path")?;

    if args.dry_run {
        println!("{}", config_path.display());
        return Ok(());
    }

    let paramses = &[WriteParams {
        path: &config_path,
        contents: DEFAULT_CONFIG.to_owned(),
        force: args.force,
        mode: None,
    }];

    write_with_params(paramses).context("Saving config file")?;

    println!("Default config initialized at '{}'", config_path.display());

    Ok(())
}
