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

    #[error("duplicate notice element")]
    /// A notice element was defined more than once.
    DuplicateNotice,

    #[error("duplicate manifest-server element")]
    /// A manifest-server element was defined more than once.
    DuplicateManifestServer,

    #[error("duplicate repo-hooks element")]
    /// A repo-hooks element was defined more than once.
    DuplicateRepoHooks,

    #[error("duplicate superproject element")]
    /// A superproject element was defined more than once.
    DuplicateSuperproject,

    #[error("duplicate default element")]
    /// A default element was defined more than once.
    DuplicateDefault,

    #[error("circular include detected: {0}")]
    /// A circular manifest include was detected.
    CircularInclude(String),

    #[error("invalid boolean value for attribute {attr}: {value}")]
    /// An attribute expected a boolean but got an invalid value.
    InvalidBoolean {
        /// The name of the attribute.
        attr: String,
        /// The invalid value provided.
        value: String,
    },

    #[error("invalid integer value for attribute {attr}: {value}")]
    /// An attribute expected an integer but got an invalid value.
    InvalidInteger {
        /// The name of the attribute.
        attr: String,
        /// The invalid value provided.
        value: String,
    },

    #[error("invalid value for attribute {attr}: {value} must be greater than 0")]
    /// An integer attribute must be greater than zero.
    InvalidPositiveInteger {
        /// The name of the attribute.
        attr: String,
        /// The invalid value provided.
        value: String,
    },

    #[error("invalid project name: {0}")]
    /// A project name contains invalid characters or components.
    InvalidProjectName(String),

    #[error("invalid copyfile src: {0}")]
    /// A copyfile src path is invalid.
    InvalidCopyfileSrc(String),

    #[error("invalid copyfile dest: {0}")]
    /// A copyfile dest path is invalid.
    InvalidCopyfileDest(String),

    #[error("invalid linkfile src: {0}")]
    /// A linkfile src path is invalid.
    InvalidLinkfileSrc(String),

    #[error("invalid linkfile dest: {0}")]
    /// A linkfile dest path is invalid.
    InvalidLinkfileDest(String),

    #[error("invalid annotation keep value: {0}")]
    /// An annotation keep attribute is not "true" or "false".
    InvalidAnnotationKeep(String),

    #[error("remove-project specifies non-existent project: {0}")]
    /// A remove-project directive targeted a project that does not exist.
    RemoveProjectNotFound(String),

    #[error("extend-project base-rev mismatch for {name}: expected {expected}, found {found}")]
    /// An extend-project base-rev did not match the project's revision.
    ExtendProjectBaseRevMismatch {
        /// The project name.
        name: String,
        /// The expected base revision.
        expected: String,
        /// The revision found on the project.
        found: String,
    },

    #[error("extend-project cannot use dest-path when matching multiple projects: {0}")]
    /// dest-path was used on an extend-project matching multiple projects.
    ExtendProjectDestPathMultipleMatches(String),

    #[error("maximum submanifest depth {0} exceeded")]
    /// The submanifest nesting depth exceeded the limit.
    MaxSubmanifestDepthExceeded(usize),

    #[error("IO error: {0}")]
    /// An I/O error occurred.
    Io(#[from] std::io::Error),
}

impl From<quick_xml::encoding::EncodingError> for Error {
    fn from(e: quick_xml::encoding::EncodingError) -> Self {
        Self::Encoding(e.to_string())
    }
}
