use std::path::{Path, PathBuf};

use shared_utils::files::NamedProjectDirs;

#[derive(Debug)]
pub struct DefaultPaths {
    config: PathBuf,
    log_dir: PathBuf,
}

impl DefaultPaths {
    /// Initialize a `DefaultPaths` instance.
    ///
    /// # Default paths
    /// `config`: `NamedProjectDirs::config_dir()/config.toml`
    /// `log_dir`: `NamedProjectDirs::state_dir()`
    pub fn defaults(component: impl Into<PathBuf>) -> Option<Self> {
        let base = NamedProjectDirs::new(component)?;

        let config = base.config_dir().join("config.toml");

        let log_dir = base.state_dir().to_owned();

        Some(Self { config, log_dir })
    }

    pub fn config(&self) -> &Path {
        self.config.as_path()
    }

    pub fn log_dir(&self) -> &Path {
        self.log_dir.as_path()
    }
}
