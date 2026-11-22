use std::path::PathBuf;

use anyhow::Context;
use clap::Args;
use my_fs_utils::WriteParams;
use my_macros::first_match;
use serde::{Deserialize, Serialize};

use crate::{DEFAULT_CONFIG, default_paths::DefaultPaths};

use super::write_with_params;

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

pub fn init_config(
    default_paths: Option<DefaultPaths>,
    args: InitConfigArgs,
) -> anyhow::Result<()> {
    let config_path = first_match! {
        Some(path) = args.path.as_deref() => path,
        Some(defaults) = &default_paths => defaults.config(),
    }
    .context("Resolving config file path")?;

    if args.dry_run {
        println!("Config path: {}", config_path.display());
        return Ok(());
    }

    write_with_params(
        WriteParams::new(&config_path, DEFAULT_CONFIG).force(args.force),
        None,
    )?;

    println!("Default config initialized at '{}'", config_path.display());

    Ok(())
}
