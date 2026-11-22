use std::path::{Path, PathBuf};

use shared_utils::files::NamedProjectDirs;

/// Collection of relevant paths for the server to read or initialize important files.
#[derive(Debug)]
pub struct DefaultPaths {
    config: PathBuf,
    ca_cert: PathBuf,
    ca_key: PathBuf,
    server_cert: PathBuf,
    server_key: PathBuf,
    log_dir: PathBuf,
}

impl DefaultPaths {
    /// Initialize a `DefaultPaths` instance.
    ///
    /// # Default paths
    /// `config`: `NamedProjectDirs::config_dir()/config.toml`
    /// `ca_cert`: `NamedProjectDirs::data_dir()/tls/ca/certificate.pem`
    /// `ca_key`: `NamedProjectDirs::data_dir()/tls/ca/key.pem`
    /// `server_cert`: `NamedProjectDirs::data_dir()/tls/server/certificate.pem`
    /// `server_key`: `NamedProjectDirs::data_dir()/tls/server/key.pem`
    /// `log_dir`: `NamedProjectDirs::state_dir()`
    pub fn defaults(component: impl Into<PathBuf>) -> Option<Self> {
        let base = NamedProjectDirs::new(component)?;

        let config = base.config_dir().join("config.toml");

        let ca_dir = base.data_dir().join("tls").join("ca");
        let ca_cert = ca_dir.join("certificate.pem");
        let ca_key = ca_dir.join("key.pem");

        let server_cert_dir = base.data_dir().join("tls").join("server");
        let server_cert = server_cert_dir.join("certificate.pem");
        let server_key = server_cert_dir.join("key.pem");

        let log_dir = base.state_dir().to_owned();

        Some(Self {
            config,
            ca_cert,
            ca_key,
            server_cert,
            server_key,
            log_dir,
        })
    }

    pub fn config(&self) -> &Path {
        self.config.as_path()
    }

    pub fn ca_cert(&self) -> &Path {
        self.ca_cert.as_path()
    }

    pub fn ca_key(&self) -> &Path {
        self.ca_key.as_path()
    }

    pub fn server_cert(&self) -> &Path {
        self.server_cert.as_path()
    }

    pub fn server_key(&self) -> &Path {
        self.server_key.as_path()
    }

    pub fn log_dir(&self) -> &Path {
        self.log_dir.as_path()
    }
}
