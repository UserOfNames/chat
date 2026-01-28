use std::fs::{File, create_dir_all};
use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::Args;
use rcgen::{CertifiedKey, generate_simple_self_signed};
use serde::{Deserialize, Serialize};

use crate::{first_match, get_project_dirs};

#[derive(Debug, Args, Serialize, Deserialize)]
pub struct InitCertsArgs {
    // Path to the directory where the key file and certificate will be placed
    #[arg(short = 'p', long)]
    output_path: Option<PathBuf>,

    /// Subject Alternative Names (domains/IPs). Defaults to "localhost" if empty
    #[arg(short, long)]
    domains: Vec<String>,

    /// Overwrite existing files in the target directory
    #[arg(short, long)]
    force: bool,

    /// Print the target directory without creating any files
    #[arg(long)]
    dry_run: bool,

    /// Name of the certificate file
    #[arg(long, default_value = "certificate.pem")]
    cert_name: String,

    /// Name of the private key file
    #[arg(long, default_value = "private_key.pem")]
    key_name: String,
}

pub fn init_certs(args: InitCertsArgs) -> anyhow::Result<()> {
    let project_dirs = get_project_dirs();

    let output_dir = first_match! {
        Some(path) = &args.output_path => path.as_path(),
        Some(pd) = &project_dirs => pd.data_dir(),
    };

    let Some(output_dir) = output_dir else {
        bail!("Output directory could not be resolved");
    };

    let cert_path = output_dir.join(&args.cert_name);
    let key_path = output_dir.join(&args.key_name);

    if args.dry_run {
        println!("{}", output_dir.display());
        return Ok(());
    }

    create_dir_all(output_dir).context("Creating output directory")?;

    let subject_alt_names = args.domains;
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(subject_alt_names).context("Generating certificates")?;

    let targets = [
        (cert_path, cert.pem()),
        (key_path, signing_key.serialize_pem()),
    ];

    for (path, contents) in targets {
        let file = if args.force {
            File::create(&path)
        } else {
            File::create_new(&path)
        };

        let mut file = match file {
            Ok(file) => file,

            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => bail!(
                "File already exists at path '{}'. Use --force to overwrite it.",
                path.display()
            ),

            Err(e) => {
                return Err(e)
                    .with_context(|| format!("Initializing file at path '{}'", path.display()));
            }
        };

        file.write_all(contents.as_bytes())
            .context("Writing default config file")?;

        println!("Initialized file at path '{}'", path.display());
    }

    println!("Generation complete.");

    Ok(())
}
