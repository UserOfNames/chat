use std::{
    env::home_dir,
    io,
    path::{Component, Path, PathBuf},
};

use directories::ProjectDirs;
use figment::value::magic::RelativePathBuf;
use serde::{Deserialize, Serialize};

/// Attempt to match a series of bindings against patterns and return the first match.
#[macro_export]
macro_rules! first_match {
    // Base case: The final arm (no trailing comma required)
    ($pat:pat = $expr:expr => $body:expr $(,)?) => {
        if let $pat = $expr {
            ::core::option::Option::Some($body)
        } else {
            ::core::option::Option::None
        }
    };

    // Recursive case: Take the first arm, recurse on the rest
    ($pat:pat = $expr:expr => $body:expr, $($rest:tt)+) => {
        if let $pat = $expr {
            ::core::option::Option::Some($body)
        } else {
            $crate::first_match!($($rest)+)
        }
    };
}

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

        Some(Self {
            base,
            component,
            config_dir,
            data_dir,
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
