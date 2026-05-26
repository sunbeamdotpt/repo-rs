// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

/// Errors that can occur during command execution.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error("engine error: {0}")]
    /// An error from the engine layer.
    Engine(#[from] repo_rs_engine::Error),

    #[error("invalid arguments: {0}")]
    /// The provided arguments are invalid.
    InvalidArguments(String),

    #[error("IO error: {0}")]
    /// An I/O error occurred.
    Io(#[from] std::io::Error),
}
