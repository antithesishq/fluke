use std::env;

use serde::{Deserialize, Serialize};

use crate::{AbsolutePath, RelativePath};

#[derive(Clone, Debug, Deserialize, Serialize, Hash, PartialEq, Eq)]
#[serde(tag = "source", rename_all = "camelCase", deny_unknown_fields)]
pub enum Impurity {
    Import {
        path: AbsolutePath,
    },
    ReadFile {
        path: AbsolutePath,
    },
    HashFile {
        path: AbsolutePath,
    },
    ReadFileType {
        path: AbsolutePath,
    },
    ReadDir {
        path: AbsolutePath,
    },
    #[serde(rename = "path/filter")]
    PathFilter {
        path: AbsolutePath,
    },
    FilterSource {
        path: AbsolutePath,
    },
    Path {
        path: AbsolutePath,
        filter: Option<bool>,
    },
    #[serde(rename = "getEnv")]
    EnvVar {
        name: String,
        value: String,
    },
}

impl Impurity {
    pub fn path(&self) -> Option<&AbsolutePath> {
        match self {
            Impurity::Import { path }
            | Impurity::ReadFile { path }
            | Impurity::HashFile { path }
            | Impurity::ReadFileType { path }
            | Impurity::ReadDir { path }
            | Impurity::PathFilter { path }
            | Impurity::FilterSource { path }
            | Impurity::Path { path, .. } => Some(path),
            Impurity::EnvVar { .. } => None,
        }
    }

    pub fn has_changed(&self, watches: &[RelativePath], root: &AbsolutePath) -> bool {
        let strip_root = |p: &AbsolutePath| -> RelativePath {
            RelativePath::new(p.abs().strip_prefix(root.abs()).expect("root to be parent"))
                .expect("Path::strip_prefix to return a relative path")
        };

        match self {
            Impurity::Import { path } // TODO: default.nix case lmao
            | Impurity::ReadFile { path }
            | Impurity::HashFile { path }
            | Impurity::ReadFileType { path }
            | Impurity::PathFilter { path } => {
                watches.iter().any(|e| e.rel() == strip_root(path).rel())
            }

            Impurity::ReadDir { path } => watches
                .iter()
                .any(|e| e.rel().parent() == Some(strip_root(path).rel()) && e.rel() != ".git"),

            // TODO: Replace with simply comparing the file tree(s).
            //
            // The thought with this branch is that if a path is filtered (and we make it to this point,
            // so that the expression used to filter didn't change), then we can rely on the `Impurity::PathFilter`s
            // for more granular caching.
            //
            // Given the filters are turing complete, this premise is not provably correct. Thus, for now, we simply
            // copy the heuristic from the unfiltered path case, and *only return false if we're getting the project
            // root*, assuming this is a fileset.
            //
            // TODO: this *REALLY* wants a test.
            Impurity::Path {
                path,
                filter: Some(true),
            }
            | Impurity::FilterSource { path } => watches.iter().any(|e| {
                e.rel().starts_with(strip_root(path).rel())
                    && e.rel() != ".git"
                    && e.rel().strip_prefix(root.abs()).is_ok_and(|p| p == "")
            }),

            Impurity::Path {
                path,
                filter: None | Some(false),
            } => watches
                .iter()
                .any(|e| e.rel().starts_with(strip_root(path).rel()) && e.rel() != ".git"),

            Impurity::EnvVar { name, value } => env::var(name).unwrap_or_default() != *value,
        }
    }
}
