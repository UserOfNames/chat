use std::path::PathBuf;

use anyhow::Context;
use clap::Args;
use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};

use crate::{Config, ENV_VAR_PREFIX, get_config_path, get_project_dirs};

#[derive(Debug, Args, Serialize, Deserialize)]
pub struct RunArgs {
    /// The address the TCP listener binds to
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    listener_address: Option<String>,

    /// Path to the TOML config file for the server.
    #[arg(long, value_name = "PATH", global = true)]
    config_file: Option<PathBuf>,
}

pub fn main(args: RunArgs) -> anyhow::Result<()> {
    let project_dirs = get_project_dirs();
    let args_conf_path = args.config_file.as_deref();
    let env_conf_path = std::env::var(format!("{ENV_VAR_PREFIX}CONFIG_FILE"))
        .ok()
        .map(PathBuf::from);

    let config_path = get_config_path(
        project_dirs.as_ref(),
        &[args_conf_path, env_conf_path.as_deref()],
    );

    let base_config = Config::default();

    let mut figment = Figment::new().merge(Serialized::defaults(base_config));

    if let Some(path) = config_path {
        figment = figment.merge(Toml::file(path));
    }

    let config: Config = figment
        .merge(Env::prefixed(ENV_VAR_PREFIX))
        .merge(Serialized::defaults(args))
        .extract()
        .context("Resolving configuration")?;

    Ok(())
}
