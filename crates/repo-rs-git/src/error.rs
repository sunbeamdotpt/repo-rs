// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

/// Errors originating from git operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error("git2 error: {0}")]
    /// An error from the underlying git2 library.
    Git2(String),

    #[error("IO error: {0}")]
    /// An I/O error occurred.
    Io(#[from] std::io::Error),

    #[error("authentication failed")]
    /// Authentication failed.
    Auth,

    #[error("ref not found: {0}")]
    /// The requested git ref was not found.
    RefNotFound(String),

    #[error("config error: {0}")]
    /// A configuration error.
    Config(String),

    #[error("invalid refspec: {0}")]
    /// The refspec is invalid.
    InvalidRefspec(String),
}

impl From<tokio::task::JoinError> for Error {
    fn from(e: tokio::task::JoinError) -> Self {
        Self::Git2(e.to_string())
    }
}

impl From<git2::Error> for Error {
    fn from(e: git2::Error) -> Self {
        Self::Git2(e.message().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::Git2("something".to_string());
        assert_eq!(err.to_string(), "git2 error: something");
    }

    #[test]
    fn test_from_io_error() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "oops");
        let err: Error = io.into();
        assert!(err.to_string().contains("oops"));
    }

    #[tokio::test]
    async fn test_from_join_error() {
        let handle = tokio::task::spawn_blocking(|| panic!("test panic"));
        let join_err = handle.await.unwrap_err();
        let err: Error = join_err.into();
        assert!(err.to_string().contains("test panic") || err.to_string().contains("panic"));
    }
}
