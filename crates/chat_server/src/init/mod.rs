use std::io::{Write, stdin, stdout};
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, bail};
use clap::Args;
use serde::{Deserialize, Serialize};

use crate::{Config, InitMode, get_config_path, get_project_dirs};

#[derive(Debug, Args, Serialize, Deserialize)]
pub struct ConfigArgs {
    /// Path to the TOML config file for the server
    path: Option<PathBuf>,
}

pub fn main(mode: InitMode) -> anyhow::Result<()> {
    match mode {
        InitMode::Config(args) => init_config(args),
    }
}

fn init_config(args: ConfigArgs) -> anyhow::Result<()> {
    let project_dirs = get_project_dirs();
    let args_conf_path = args.path.as_deref();

    let config_path = get_config_path(project_dirs.as_ref(), &[args_conf_path]);

    let Some(config_path) = config_path else {
        bail!("Config path could not be resolved");
    };

    if config_path.exists() {
        println!(
            "WARNING: A file already exists at path '{}'. Continuing will OVERWRITE this file.",
            config_path.display()
        );

        // Sleep for a moment so the server admin won't just blaze ahead
        // and clobber the existing config file
        sleep(Duration::from_secs(1));
    }

    print!(
        "Initialize default configuration at path '{}'? [y/n]: ",
        config_path.display()
    );
    stdout().flush().context("Printing prompt to stdout")?;

    let mut buffer = String::new();
    stdin().read_line(&mut buffer).context("Reading input")?;
    let answer = buffer.trim().to_lowercase();

    match answer.as_str() {
        "y" | "yes" => {}
        "n" | "no" | "" => return Ok(()),
        _ => bail!("Expected 'y' or 'n', got '{answer}'"),
    }

    let ser =
        toml::to_string_pretty(&Config::default()).context("Resolving TOML for default config")?;
    std::fs::write(config_path, ser).context("Writing default config file")?;

    Ok(())
}
