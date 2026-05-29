use std::path::PathBuf;

use anyhow::Context;
use clap::Args;
use rcgen::{BasicConstraints, CertificateParams, IsCa, Issuer, KeyPair};
use serde::{Deserialize, Serialize};
use shared_utils::first_match;

use crate::{
    DefaultPaths,
    init::{WriteParams, write_with_params},
};

#[derive(Debug, Args, Serialize, Deserialize)]
pub struct InitPkiArgs {
    /// Subject Alternative Names. Defaults to `["localhost", "127.0.0.1", "::1"]` if empty
    #[arg(
        short,
        long,
        value_delimiter = ',',
        default_values = ["localhost", "127.0.0.1", "::1"]
    )]
    subject_alt_names: Vec<String>,

    /// Overwrite existing files at output paths
    #[arg(short, long)]
    force: bool,

    /// Print relevant output information without writing files
    #[arg(long)]
    dry_run: bool,

    /// Path to the server certificate output file
    #[arg(long)]
    server_cert_path: Option<PathBuf>,

    /// Path to the server private key output file
    #[arg(long)]
    server_key_path: Option<PathBuf>,

    /// Path to the signing (CA) certificate output file
    #[arg(long)]
    ca_cert_path: Option<PathBuf>,

    /// Path to the signing (CA) private key output file
    #[arg(long)]
    ca_key_path: Option<PathBuf>,
}

pub fn init_pki(default_paths: Option<DefaultPaths>, args: InitPkiArgs) -> anyhow::Result<()> {
    let ca_cert_path = first_match! {
        Some(path) = &args.ca_cert_path => path,
        Some(defaults) = &default_paths => &defaults.ca_cert,
    }
    .context("Resolving path for CA certificate file")?;

    let ca_key_path = first_match! {
        Some(path) = &args.ca_key_path => path,
        Some(defaults) = &default_paths => &defaults.ca_key,
    }
    .context("Resolving path for CA key file")?;

    let server_cert_path = first_match! {
        Some(path) = &args.server_cert_path => path,
        Some(defaults) = &default_paths => &defaults.server_cert,
    }
    .context("Resolving server path for certificate file")?;

    let server_key_path = first_match! {
        Some(path) = &args.server_key_path => path,
        Some(defaults) = &default_paths => &defaults.server_key,
    }
    .context("Resolving output path for private key file")?;

    if args.dry_run {
        println!("CA cert path: '{}'", ca_cert_path.display());
        println!("CA key path: '{}'", ca_key_path.display());
        println!("Server cert path: '{}'", server_cert_path.display());
        println!("Server key path: '{}'", server_key_path.display());
        return Ok(());
    }

    let ca_keypair = KeyPair::generate().context("Generating keypair for CA")?;
    let mut ca_cert_params = CertificateParams::new(vec![]).context("Generating certificate")?;
    ca_cert_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let ca_cert = ca_cert_params
        .self_signed(&ca_keypair)
        .context("Signing new certificate")?;

    let server_keypair = KeyPair::generate().context("Generating keypair for server")?;
    let ca_issuer = Issuer::new(ca_cert_params, &ca_keypair);
    let mut server_cert_params =
        CertificateParams::new(args.subject_alt_names).context("Generating server certificate")?;
    server_cert_params.is_ca = IsCa::ExplicitNoCa;
    let server_cert = server_cert_params
        .signed_by(&server_keypair, &ca_issuer)
        .context("Signing server certificate")?;

    let ca_cert_pem = ca_cert.pem();
    let ca_key_pem = ca_keypair.serialize_pem();
    let server_cert_pem = server_cert.pem();
    let server_key_pem = server_keypair.serialize_pem();

    let paramses = &[
        WriteParams {
            path: ca_key_path,
            contents: ca_key_pem,
            force: args.force,
            mode: Some(0o400),
        },
        WriteParams {
            path: ca_cert_path,
            contents: ca_cert_pem,
            force: args.force,
            mode: None,
        },
        WriteParams {
            path: server_key_path,
            contents: server_key_pem,
            force: args.force,
            mode: None,
        },
        WriteParams {
            path: server_cert_path,
            contents: server_cert_pem,
            force: args.force,
            mode: None,
        },
    ];

    write_with_params(paramses).context("Writing new files")
}
