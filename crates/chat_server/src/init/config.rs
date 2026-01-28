use std::fs::{File, create_dir_all};
use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::Args;
use serde::{Deserialize, Serialize};

use crate::{CONFIG_FILE_NAME, Config, first_match, get_project_dirs};

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

pub fn init_config(args: InitConfigArgs) -> anyhow::Result<()> {
    let project_dirs = get_project_dirs();

    let config_path = first_match! {
        Some(path) = args.path => path,
        Some(pd) = &project_dirs => pd.config_dir().join(CONFIG_FILE_NAME),
    };

    let Some(config_path) = config_path else {
        bail!("Config path could not be resolved");
    };

    if args.dry_run {
        println!("{}", config_path.display());
        return Ok(());
    }

    if let Some(parent) = config_path.parent() {
        create_dir_all(parent)
            .with_context(|| format!("Creating output directory: '{}'", parent.display()))?;
    }

    let file = if args.force {
        File::create(&config_path)
    } else {
        File::create_new(&config_path)
    };

    let mut file = match file {
        Ok(file) => file,

        // This error can only arise if File::create_new was used, so there's no need to check
        // args.force here.
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => bail!(
            "File already exists at path '{}'. Use --force to overwrite it.",
            config_path.display()
        ),

        Err(e) => {
            return Err(e).with_context(|| {
                format!("Initializing config at path '{}'", config_path.display())
            });
        }
    };

    let ser =
        toml::to_string_pretty(&Config::default()).context("Resolving TOML for default config")?;

    file.write_all(ser.as_bytes())
        .context("Writing default config file")?;

    println!("Default config initialized at '{}'", config_path.display());

    Ok(())
}
