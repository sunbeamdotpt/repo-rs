// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

#![warn(missing_docs)]

//! Git engine for repo-rs.
//!
//! This is the **only** crate that interacts with `git2`. It provides a
//! thin, repo-specific abstraction over git operations so that
//! `repo-rs-engine` never needs to depend on git2 directly.

#![doc = include_str!("../README.md")]

/// Git backend implementations.
pub mod backend;
/// The `repo checkout` command implementation.
pub mod checkout;
/// clone module.
pub mod clone;
/// config module.
pub mod config;
/// The `repo download` command implementation.
pub mod download;
/// Error types for this crate.
pub mod error;
/// fetch module.
pub mod fetch;
/// open module.
pub mod open;
/// progress module.
pub mod progress;
/// push module.
pub mod push;
/// The `repo cherry-pick` command implementation.
pub mod cherry_pick;
/// Git reference operations.
pub mod refs;
/// The `repo stage` command implementation.
pub mod stage;

pub use backend::{GitBackend, DefaultBackend};
pub use error::Error;
pub use progress::ProgressCallback;

#[cfg(any(test, feature = "test-util"))]
pub use backend::MockGitBackend;

/// Test helpers for git operations.
#[cfg(test)]
pub mod test_helpers {
    use camino::Utf8PathBuf;
    use std::process::Command;

    /// Create a bare repository with a single commit using the system `git` CLI.
    ///
    /// # Panics
    ///
    /// Panics if `path` is not valid UTF-8 or if any `git` command fails.
    #[must_use]
    pub fn git_init_bare_with_commit(path: &std::path::Path) -> Utf8PathBuf {
        let path = Utf8PathBuf::from_path_buf(path.to_path_buf()).unwrap();

        Command::new("git")
            .args(["init", "--bare", path.as_str()])
            .output()
            .expect("git init --bare failed");

        let tmp = tempfile::tempdir().unwrap();
        let tmp_path = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();

        Command::new("git")
            .args(["clone", path.as_str(), tmp_path.as_str()])
            .output()
            .expect("git clone failed");

        std::fs::write(tmp_path.join("README.md"), "hello").unwrap();

        Command::new("git")
            .args(["-C", tmp_path.as_str(), "add", "."])
            .output()
            .expect("git add failed");

        Command::new("git")
            .args(["-C", tmp_path.as_str(), "commit", "-m", "initial", "--no-gpg-sign"])
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .output()
            .expect("git commit failed");

        Command::new("git")
            .args(["-C", tmp_path.as_str(), "push", "origin", "HEAD"])
            .output()
            .expect("git push failed");

        path
    }

    /// Add an empty commit to an existing repository using the system `git` CLI.
    ///
    /// # Panics
    ///
    /// Panics if `repo_path` is not valid UTF-8 or if the `git` command fails.
    pub fn git_add_empty_commit(repo_path: &std::path::Path, message: &str) {
        let path = Utf8PathBuf::from_path_buf(repo_path.to_path_buf()).unwrap();
        Command::new("git")
            .args([
                "-C",
                path.as_str(),
                "commit",
                "--allow-empty",
                "-m",
                message,
                "--no-gpg-sign",
            ])
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .output()
            .expect("git commit failed");
    }
}
