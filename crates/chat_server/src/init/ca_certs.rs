use std::path::PathBuf;

use anyhow::Context;
use clap::Args;
use my_fs_utils::WriteParams;
use my_macros::first_match;
use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair};
use serde::{Deserialize, Serialize};

use crate::default_paths::DefaultPaths;

use super::write_with_params;

#[derive(Debug, Args, Serialize, Deserialize)]
pub struct InitCACertsArgs {
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
}

pub fn init_ca_certs(
    default_paths: Option<DefaultPaths>,
    args: InitCACertsArgs,
) -> anyhow::Result<()> {
    let output_cert_path = first_match! {
        Some(path) = args.output_cert_path.as_deref() => path,
        Some(defaults) = &default_paths => defaults.ca_cert(),
    }
    .context("Resolving output path for certificate file")?;

    let output_key_path = first_match! {
        Some(path) = args.output_key_path.as_deref() => path,
        Some(defaults) = &default_paths => defaults.ca_key(),
    }
    .context("Resolving output path for private key file")?;

    if args.dry_run {
        println!("CA cert path: '{}'", output_cert_path.display());
        println!("CA key path: '{}'", output_key_path.display());
    }

    let signing_key = KeyPair::generate().context("Generating keypair")?;

    let mut cert_params = CertificateParams::new(vec![]).context("Generating certificate")?;
    cert_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);

    let cert_pem = cert_params
        .self_signed(&signing_key)
        .context("Signing new certificate")?
        .pem();

    let key_pem = signing_key.serialize_pem();

    write_with_params(
        WriteParams::new(&output_key_path, &key_pem).force(args.force),
        Some(0o600),
    )?;
    println!(
        "CA private key initialized at '{}'",
        output_key_path.display()
    );

    write_with_params(
        WriteParams::new(&output_cert_path, &cert_pem).force(args.force),
        None,
    )?;
    println!(
        "CA certificate initialized at '{}'",
        output_cert_path.display()
    );

    #[cfg(not(unix))]
    eprintln!(
        "WARNING: permissions cannot be set on non-unix platforms.\n\
         Ensure the key file's permissions are correct."
    );

    Ok(())
}
