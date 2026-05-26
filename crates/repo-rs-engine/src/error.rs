// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

/// Errors that can occur during engine execution.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error("git error: {0}")]
    /// An error from the git backend.
    Git(#[from] repo_rs_git::Error),

    #[error("model error: {0}")]
    /// A data model inconsistency error.
    Model(#[from] repo_rs_model::Error),

    #[error("manifest error: {0}")]
    /// A manifest parsing or validation error.
    Manifest(#[from] repo_rs_manifest::Error),

    #[error("invalid arguments: {0}")]
    /// The provided arguments are invalid.
    InvalidArguments(String),

    #[error("sync error: {0}")]
    /// The sync operation failed.
    Sync(String),

    #[error("upload error: {0}")]
    /// The upload operation failed.
    Upload(String),

    #[error("IO error: {0}")]
    /// An I/O error occurred.
    Io(#[from] std::io::Error),
}
