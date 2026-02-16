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

/// Return the default `ProjectDirs` cosntruction for the project.
pub fn get_project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("rs", "UserOfNames", "my_chat")
}
