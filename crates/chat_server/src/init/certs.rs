use std::fs::{File, create_dir_all};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use anyhow::{Context, bail};
use clap::Args;
use rcgen::{CertifiedKey, generate_simple_self_signed};
use serde::{Deserialize, Serialize};

use crate::{first_match, utils::get_project_dirs};

#[derive(Debug, Args, Serialize, Deserialize)]
pub struct InitCertsArgs {
    /// Subject Alternative Names (domains/IPs). Defaults to "localhost" if empty
    #[arg(short, long, default_values = ["localhost"])]
    domains: Vec<String>,

    /// Overwrite existing files in the target directory
    #[arg(short, long)]
    force: bool,

    /// Print the target directory without creating any files
    #[arg(long)]
    dry_run: bool,

    /// Path to the certificate output file
    #[arg(long)]
    cert_path: Option<PathBuf>,

    /// Path to the private key output file
    #[arg(long)]
    key_path: Option<PathBuf>,
}

pub fn init_certs(args: InitCertsArgs) -> anyhow::Result<()> {
    let project_dirs = get_project_dirs();

    let cert_path = first_match! {
        Some(path) = args.cert_path => path,
        Some(pd) = &project_dirs => pd.data_dir().join("certificate.pem"),
    };

    let Some(cert_path) = cert_path else {
        bail!("Output path for certificate file could not be resolved");
    };

    let key_path = first_match! {
        Some(path) = args.key_path => path,
        Some(pd) = &project_dirs => pd.data_dir().join("key.pem"),
    };

    let Some(key_path) = key_path else {
        bail!("Output path for key file could not be resolved");
    };

    if args.dry_run {
        println!("Certificate path: '{}'", cert_path.display());
        println!("Key path: '{}'", key_path.display());
        return Ok(());
    }

    if let Some(parent) = cert_path.parent() {
        create_dir_all(parent).with_context(|| {
            format!(
                "Creating parent directory for file: '{}'",
                cert_path.display()
            )
        })?;
    }

    if let Some(parent) = key_path.parent() {
        create_dir_all(parent).with_context(|| {
            format!(
                "Creating parent directory for file: '{}'",
                key_path.display()
            )
        })?;
    }

    let subject_alt_names = args.domains;
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(subject_alt_names).context("Generating certificates")?;

    write_file(&cert_path, cert.pem(), 0o644, args.force).context("Writing certificate file")?;
    println!("Initialized certificate at path '{}'", &cert_path.display());

    write_file(&key_path, signing_key.serialize_pem(), 0o400, args.force)
        .context("Writing key file")?;
    println!("Initialized key file at path '{}'", &cert_path.display());

    println!("Generation complete.");

    #[cfg(not(unix))]
    println!("WARNING: Default file permissions on non-unix platforms may not be secure.");

    Ok(())
}

fn write_file(path: &Path, contents: String, mode: u32, force: bool) -> anyhow::Result<()> {
    let mut file = File::options();
    file.write(true)
        .create(true)
        .truncate(true)
        .create_new(!force); // Ignores create() and truncate() if set

    #[cfg(unix)]
    file.mode(mode);

    let mut file = match file.open(path) {
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

    Ok(())
}
