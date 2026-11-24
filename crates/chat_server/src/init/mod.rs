mod ca_certs;
mod config;
mod pki;
mod server_certs;

use std::fs::Permissions;
use std::io;

use clap::Subcommand;
use my_fs_utils::WriteParams;
use serde::{Deserialize, Serialize};

use crate::DefaultPaths;

#[derive(Debug, Subcommand, Serialize, Deserialize)]
pub enum Mode {
    /// Initialize a default config file
    Config(config::Args),

    /// Initialize a root CA certificate and key
    CaCerts(ca_certs::Args),

    /// Initialize your own PKI: root CA certificate and key file, and signed certificates to go
    /// with them
    Pki(pki::Args),

    /// Initialize a CA-signed private key and certificate for TLS
    ServerCerts(server_certs::Args),
}

impl Mode {
    pub fn run(self, default_paths: Option<DefaultPaths>) -> anyhow::Result<()> {
        match self {
            Self::CaCerts(args) => args.run(default_paths),
            Self::Config(args) => args.run(default_paths),
            Self::Pki(args) => args.run(default_paths),
            Self::ServerCerts(args) => args.run(default_paths),
        }
    }
}

/// Helper function to make using [`WriteParams`] a bit easier within the needs of this project
///
/// * Handles mode without having to manually write `#[cfg]`
/// * Automatically sets common parameters:
///     * `make_parents(true)`
///     * `atomic(true)`
///     * Everything else (besides mode) must be set manually
fn write_with_params(params: WriteParams, mode: Option<u32>) -> io::Result<()> {
    let mut params = params.make_parents(true).atomic(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        params = params.permissions(mode.map(Permissions::from_mode));
    }

    params.write()
}
