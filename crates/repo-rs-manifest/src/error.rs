// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

/// Errors that can occur when parsing or validating a manifest.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error("XML parse error: {0}")]
    /// An XML parsing error.
    Xml(#[from] quick_xml::Error),

    #[error("XML encoding error: {0}")]
    /// An encoding error.
    Encoding(String),

    #[error("invalid manifest path: {0}")]
    /// The manifest path is invalid.
    InvalidPath(String),

    #[error("missing required attribute: {0}")]
    /// A required XML attribute is missing.
    MissingAttribute(String),

    #[error("unknown remote reference: {0}")]
    /// A project references an undefined remote.
    UnknownRemote(String),

    #[error("duplicate remote name: {0}")]
    /// A remote name is defined more than once.
    DuplicateRemote(String),

    #[error("duplicate submanifest name: {0}")]
    /// A submanifest name is defined more than once.
    DuplicateSubmanifest(String),

    #[error("circular include detected: {0}")]
    /// A circular manifest include was detected.
    CircularInclude(String),

    #[error("IO error: {0}")]
    /// An I/O error occurred.
    Io(#[from] std::io::Error),
}

impl From<quick_xml::encoding::EncodingError> for Error {
    fn from(e: quick_xml::encoding::EncodingError) -> Self {
        Self::Encoding(e.to_string())
    }
}
