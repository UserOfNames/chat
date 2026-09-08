mod ca_certs;
mod config;
mod pki;
mod server_certs;

use std::fs::create_dir_all;
use std::io::{self, Write};
use std::path::Path;

use clap::Subcommand;
use serde::{Deserialize, Serialize};

use ca_certs::{InitCACertsArgs, init_ca_certs};
use config::{InitConfigArgs, init_config};
use server_certs::{InitServerCertsArgs, init_server_certs};
use tempfile::NamedTempFile;

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

#[derive(Debug)]
struct WriteParams<'a> {
    path: &'a Path,
    contents: String,
    mode: Option<u32>,
    force: bool,
}

fn write_with_params(paramses: &[WriteParams]) -> io::Result<()> {
    // Create new tempfiles
    let mut tempfiles: Vec<NamedTempFile> = Vec::with_capacity(paramses.len());
    for params in paramses {
        let mut builder = tempfile::Builder::new();

        if let Some(mode) = params.mode {
            #[cfg(unix)]
            {
                use std::fs::Permissions;
                use std::os::unix::fs::PermissionsExt;
                builder.permissions(Permissions::from_mode(mode));
            }

            #[cfg(not(unix))]
            eprintln!(
                "WARNING: attempt to set permissions for file '{}' failed, as you are not on a Unix platform.\n\
                 Default permissions may be insecure!",
                params.path.display()
            );
        }

        if let Some(parent) = params.path.parent() {
            create_dir_all(parent)?;
            let mut tempfile = builder.tempfile_in(parent)?;
            write!(tempfile, "{}", params.contents)?;
            tempfiles.push(tempfile);
        }
    }

    // Persist tempfiles
    for (tempfile, params) in tempfiles.into_iter().zip(paramses.iter()) {
        if params.force {
            tempfile.persist(params.path)?;
        } else {
            tempfile.persist_noclobber(params.path)?;
        }
    }

    Ok(())
}

pub fn main(default_paths: Option<DefaultPaths>, mode: InitMode) -> anyhow::Result<()> {
    match mode {
        InitMode::Config(args) => init_config(default_paths, args),
        InitMode::CaCerts(args) => init_ca_certs(default_paths, args),
        InitMode::Pki(args) => init_pki(default_paths, args),
        InitMode::ServerCerts(args) => init_server_certs(default_paths, args),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_write_creates_new_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_cert.pem");

        let params = WriteParams {
            path: &file_path,
            contents: "hello world".to_string(),
            mode: None,
            force: false,
        };

        assert!(write_with_params(&[params]).is_ok());
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "hello world");
    }

    #[test]
    fn test_write_respects_no_clobber() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("key.pem");

        // Write initial file
        fs::write(&file_path, "original key data").unwrap();

        let params = WriteParams {
            path: &file_path,
            contents: "new key data".to_string(),
            mode: None,
            force: false,
        };

        // Should fail due to noclobber
        let result = write_with_params(&[params]);
        assert!(result.is_err());
        
        // Ensure the original contents were preserved
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "original key data");
    }

    #[test]
    fn test_write_allows_force_overwrite() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("config.toml");

        // Write initial file
        fs::write(&file_path, "old config").unwrap();

        let params = WriteParams {
            path: &file_path,
            contents: "new config".to_string(),
            mode: None,
            force: true,
        };

        assert!(write_with_params(&[params]).is_ok());
        
        // Ensure the new contents replaced the old
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "new config");
    }

    #[test]
    fn test_write_creates_missing_parent_directories() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("deep").join("nested").join("test.txt");

        let params = WriteParams {
            path: &file_path,
            contents: "nested data".to_string(),
            mode: None,
            force: false,
        };

        assert!(write_with_params(&[params]).is_ok());
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "nested data");
    }

    #[cfg(unix)]
    #[test]
    fn test_unix_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("secret.key");

        let params = WriteParams {
            path: &file_path,
            contents: "secret".to_string(),
            mode: Some(0o400),
            force: false,
        };

        assert!(write_with_params(&[params]).is_ok());

        let metadata = fs::metadata(&file_path).unwrap();
        let permissions = metadata.permissions();
        
        // We only check the lowest 9 bits (0o777) to ignore file type bits
        assert_eq!(permissions.mode() & 0o777, 0o400);
    }
}
