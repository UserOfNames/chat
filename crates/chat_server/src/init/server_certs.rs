use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use clap::Args;
use rcgen::{CertificateParams, IsCa, Issuer, KeyPair};
use serde::{Deserialize, Serialize};
use shared_utils::first_match;

use crate::DefaultPaths;

use super::{WriteParams, write_with_params};

#[derive(Debug, Args, Serialize, Deserialize)]
pub struct InitServerCertsArgs {
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

    /// Path to the certificate output file
    #[arg(long)]
    output_cert_path: Option<PathBuf>,

    /// Path to the private key output file
    #[arg(long)]
    output_key_path: Option<PathBuf>,

    /// Path to the signing (CA) certificate
    #[arg(long)]
    ca_cert_path: Option<PathBuf>,

    /// Path to the signing (CA) private key
    #[arg(long)]
    ca_key_path: Option<PathBuf>,
}

pub fn init_server_certs(
    default_paths: Option<DefaultPaths>,
    args: InitServerCertsArgs,
) -> anyhow::Result<()> {
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

    let output_cert_path = first_match! {
        Some(path) = &args.output_cert_path => path,
        Some(defaults) = &default_paths => &defaults.server_cert,
    }
    .context("Resolving output path for certificate file")?;

    let output_key_path = first_match! {
        Some(path) = &args.output_key_path => path,
        Some(defaults) = &default_paths => &defaults.server_key,
    }
    .context("Resolving output path for private key file")?;

    if args.dry_run {
        println!("CA cert path: '{}'", ca_cert_path.display());
        println!("CA key path: '{}'", ca_key_path.display());
        println!("Server cert path: '{}'", output_cert_path.display());
        println!("Server key path: '{}'", output_key_path.display());
        return Ok(());
    }

    let ca_cert_pem = fs::read_to_string(ca_cert_path).with_context(|| {
        format!(
            "Reading CA certificate file from {}",
            ca_cert_path.display()
        )
    })?;

    let ca_key_pem = fs::read_to_string(ca_key_path)
        .with_context(|| format!("Reading CA private key file from {}", ca_key_path.display()))?;

    let ca_keypair = KeyPair::from_pem(&ca_key_pem).with_context(|| {
        format!(
            "Resolving CA private key PEM from file {}",
            ca_key_path.display()
        )
    })?;

    let ca_issuer = Issuer::from_ca_cert_pem(&ca_cert_pem, ca_keypair)
        .context("Resolving CA credentials from certificate and key")?;

    let new_cert_keypair = KeyPair::generate().context("Generating keypair for new cert")?;

    let mut cert_params =
        CertificateParams::new(args.subject_alt_names).context("Generating certificate")?;
    cert_params.is_ca = IsCa::ExplicitNoCa;

    let new_cert_pem = cert_params
        .signed_by(&new_cert_keypair, &ca_issuer)
        .context("Signing new cert")?
        .pem();

    let new_keypair_pem = new_cert_keypair.serialize_pem();

    let paramses = &[
        WriteParams {
            path: output_key_path,
            contents: new_keypair_pem,
            force: args.force,
            mode: Some(0o400),
        },
        WriteParams {
            path: output_cert_path,
            contents: new_cert_pem,
            force: args.force,
            mode: None,
        },
    ];

    write_with_params(paramses).context("Writing new files")?;

    println!(
        "Server private key initialized at '{}'",
        output_key_path.display()
    );
    println!(
        "Server private key initialized at '{}'",
        output_cert_path.display()
    );

    Ok(())
}
