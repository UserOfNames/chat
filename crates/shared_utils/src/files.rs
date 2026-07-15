use std::{
    env::home_dir,
    io,
    path::{Component, Path, PathBuf},
};

use directories::ProjectDirs;
use figment::value::magic::RelativePathBuf;
use serde::{Deserialize, Serialize};

/// Custom wrapper around [`ProjectDirs`](directories::ProjectDirs).
///
/// Because this project has several crates, all of which require their own config, data, etc.
/// directories, it is desireable for them all to share a common base directory, so as to not
/// pollute the filesystem. However, for organization, they should each have their own space within
/// the base directory.
///
/// This struct accepts a name for the individual binary, and automatically appends that name to
/// each `ProjectDirs` output.
#[derive(Debug)]
pub struct NamedProjectDirs {
    base: ProjectDirs,
    component: PathBuf,
    config_dir: PathBuf,
    data_dir: PathBuf,
    state_dir: PathBuf,
}

impl NamedProjectDirs {
    /// Create a new `NamedProjectDirs` instance.
    ///
    /// `component` will be appended to all output paths from the underlying `ProjectDirs`.
    pub fn new(component: impl Into<PathBuf>) -> Option<Self> {
        let base = ProjectDirs::from("rs", "UserOfNames", "my_chat")?;

        let component = component.into();

        let config_dir = base.config_dir().join(&component);

        let data_dir = base.data_dir().join(&component);

        let state_dir = base
            .state_dir()
            .unwrap_or_else(|| base.data_local_dir())
            .join(&component);

        Some(Self {
            base,
            component,
            config_dir,
            data_dir,
            state_dir,
        })
    }

    #[must_use]
    pub fn base(&self) -> &ProjectDirs {
        &self.base
    }

    #[must_use]
    pub fn component(&self) -> &Path {
        &self.component
    }

    #[must_use]
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    #[must_use]
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    #[must_use]
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }
}

/// Abstraction over [`RelativePathBuf`](figment::value::magic::RelativePathBuf) that expands
/// leading tildes (`~`s) into the user's home directory.
///
/// This does not handle any additional shell expansion features.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TildeRelativePathBuf(RelativePathBuf);

impl TildeRelativePathBuf {
    /// Return the resolved (tilde-expanded, relative) path.
    ///
    /// # Errors
    /// * [`io::ErrorKind::NotFound`]: could not find the user's home directory
    pub fn resolved(&self) -> io::Result<PathBuf> {
        match Self::expand_leading_tilde(self.0.original()) {
            Some(Ok(path)) => Ok(path),
            Some(Err(e)) => Err(e),
            None => Ok(self.0.relative()),
        }
    }

    /// Return the inner [`RelativePathBuf::original()`].
    #[must_use]
    pub fn original(&self) -> &Path {
        self.0.original()
    }

    /// Internal helper function to expand a leading tilde, if present. Returns `None` if no
    /// expansion occurred.
    ///
    /// # Errors
    /// * [`io::ErrorKind::NotFound`]: could not find the user's home directory
    fn expand_leading_tilde(path: impl AsRef<Path>) -> Option<io::Result<PathBuf>> {
        let path = path.as_ref();

        let mut components = path.components();

        if let Some(Component::Normal(first)) = components.next()
            && first == "~"
        {
            let Some(home_dir) = home_dir() else {
                return Some(Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "Home directory failed to resolve",
                )));
            };

            let remainder = components.as_path();

            return Some(Ok(home_dir.join(remainder)));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_named_project_dirs_appends_component() {
        let component = Path::new("test_component");
        let dirs = NamedProjectDirs::new(component)
            .expect("Failed to create NamedProjectDirs. This likely indicates an environment error, not a logic error.");

        assert!(
            dirs.config_dir().ends_with(component),
            "config_dir did not end with the component name"
        );
        assert!(
            dirs.data_dir().ends_with(component),
            "data_dir did not end with the component name"
        );
        assert!(
            dirs.state_dir().ends_with(component),
            "state_dir did not end with the component name"
        );
    }

    #[test]
    fn test_expand_leading_tilde_with_empty_path() {
        let path = Path::new("");

        let result = TildeRelativePathBuf::expand_leading_tilde(path);

        assert!(result.is_none());
    }

    #[test]
    fn test_expand_leading_tilde_with_remainder() {
        let path = Path::new("~/documents/config.toml");
        let expected = home_dir()
            .expect("Home directory not found for test execution")
            .join("documents/config.toml");

        let result = TildeRelativePathBuf::expand_leading_tilde(path)
            .expect("Expected Some(_) because path starts with ~")
            .expect("Expected Ok(_) because home_dir should resolve");

        assert_eq!(result, expected);
    }

    #[test]
    fn test_expand_leading_tilde_standalone() {
        let path = Path::new("~");
        let home = home_dir().expect("Home directory not found for test execution");

        let result = TildeRelativePathBuf::expand_leading_tilde(path)
            .expect("Expected Some(_) because path starts with ~")
            .expect("Expected Ok(_) because home_dir should resolve");

        assert_eq!(result, home);
    }

    #[test]
    fn test_expand_leading_tilde_ignores_non_leading_tildes() {
        let path1 = Path::new("documents/~/test.txt");
        let path2 = Path::new("documents/test.txt");
        let path3 = Path::new("/~/documents");

        assert!(
            TildeRelativePathBuf::expand_leading_tilde(path1).is_none(),
            "Expanded a tilde that was not the first component"
        );
        assert!(
            TildeRelativePathBuf::expand_leading_tilde(path2).is_none(),
            "Attempted to expand a path with no tilde"
        );
        assert!(
            TildeRelativePathBuf::expand_leading_tilde(path3).is_none(),
            "Expanded an absolute path containing a tilde"
        );
    }
}
