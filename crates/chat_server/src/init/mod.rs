mod ca_certs;
mod config;
mod pki;
mod server_certs;

use std::fs::Permissions;
use std::io;

use clap::Subcommand;
use my_fs_utils::WriteParams;
use serde::{Deserialize, Serialize};

use ca_certs::{InitCACertsArgs, init_ca_certs};
use config::{InitConfigArgs, init_config};
use server_certs::{InitServerCertsArgs, init_server_certs};

use crate::DefaultPaths;
use crate::init::pki::{InitPkiArgs, init_pki};

#[derive(Debug, Subcommand, Serialize, Deserialize)]
pub enum InitMode {
    /// Initialize a default config file
    Config(InitConfigArgs),

    /// Initialize a root CA certificate and key
    CaCerts(InitCACertsArgs),

    /// Initialize your own PKI: root CA certificate and key file, and signed certificates to go
    /// with them
    Pki(InitPkiArgs),

    /// Initialize a CA-signed private key and certificate for TLS
    ServerCerts(InitServerCertsArgs),
}

/// Helper function to make using [`WriteParams`] a bit easier within the needs of this project
///
/// * Handles mode without having to manually write `#[cfg]`
/// * Automatically sets common parameters:
///     * `make_parents(true)`
///     * `atomic(true)`
///     * Everything else (besides mode) must be set manually
fn write_with_params(
    params: WriteParams,
    mode: Option<u32>,
) -> io::Result<()> {
    let mut params = params.make_parents(true).atomic(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        params = params.permissions(mode.map(Permissions::from_mode));
    }

    params.write()
}

pub fn main(default_paths: Option<DefaultPaths>, mode: InitMode) -> anyhow::Result<()> {
    match mode {
        InitMode::Config(args) => init_config(default_paths, args),
        InitMode::CaCerts(args) => init_ca_certs(default_paths, args),
        InitMode::Pki(args) => init_pki(default_paths, args),
        InitMode::ServerCerts(args) => init_server_certs(default_paths, args),
    }
}
