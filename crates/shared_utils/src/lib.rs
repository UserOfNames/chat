use std::path::{Path, PathBuf};

use directories::ProjectDirs;

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
