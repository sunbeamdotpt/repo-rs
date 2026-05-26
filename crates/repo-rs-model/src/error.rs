// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

/// Errors that can occur in the repo data model.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error("manifest error: {0}")]
    /// A manifest parsing or validation error.
    Manifest(#[from] repo_rs_manifest::Error),

    #[error("project not found: {0}")]
    /// The project was not found.
    ProjectNotFound(String),

    #[error("invalid group filter: {0}")]
    /// The group filter is invalid.
    InvalidGroup(String),

    #[error("invalid regex: {0}")]
    /// The regular expression is invalid.
    InvalidRegex(#[from] regex::Error),

    #[error("IO error: {0}")]
    /// An I/O error occurred.
    Io(#[from] std::io::Error),

    #[error("invalid path: {0}")]
    /// The manifest path is invalid.
    InvalidPath(String),
}
