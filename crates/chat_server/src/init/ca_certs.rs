use std::path::PathBuf;

use anyhow::Context;
use clap::Args;
use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair};
use serde::{Deserialize, Serialize};

use crate::{first_match, utils::get_tls_ca_dir};

use super::{WriteParams, write_with_params};

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

pub fn init_ca_certs(args: InitCACertsArgs) -> anyhow::Result<()> {
    let default_output_dir = get_tls_ca_dir();

    let output_cert_path = first_match! {
        Some(path) = args.output_cert_path => path,
        Some(dd) = &default_output_dir => dd.join("certificate.pem"),
    }
    .context("Resolving output path for certificate file")?;

    let output_key_path = first_match! {
        Some(path) = args.output_key_path => path,
        Some(dd) = &default_output_dir => dd.join("key.pem"),
    }
    .context("Resolving output path for private key file")?;

    if args.dry_run {
        todo!();
    }

    let signing_key = KeyPair::generate().context("Generating keypair")?;

    let mut cert_params = CertificateParams::new(vec![]).context("Generating certificate")?;
    cert_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);

    let cert_pem = cert_params
        .self_signed(&signing_key)
        .context("Signing new certificate")?
        .pem();

    let key_pem = signing_key.serialize_pem();

    let paramses = &[
        WriteParams {
            path: &output_key_path,
            contents: key_pem,
            force: args.force,
            mode: Some(0o400),
        },
        WriteParams {
            path: &output_cert_path,
            contents: cert_pem,
            force: args.force,
            mode: None,
        },
    ];

    write_with_params(paramses).context("Saving output files")
}
