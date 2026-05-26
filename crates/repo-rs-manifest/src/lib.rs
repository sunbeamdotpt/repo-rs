// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

#![warn(missing_docs)]

//! Manifest XML parsing for the repo tool.
//!
//! This crate handles parsing of `manifest.xml` and related files,
//! including `<include>` resolution, local manifest merging, and
//! validation.

/// Error types for manifest operations.
pub mod error;
/// Data models and core types.
pub mod model;
/// Manifest XML parsing.
pub mod parser;
/// Manifest resolution and merging.
pub mod resolver;
/// Manifest validation utilities.
pub mod validate;

pub use error::Error;
pub use model::Manifest;
pub use parser::{parse_manifest, validate_manifest};
pub use resolver::{
    apply_extend_projects, apply_remove_projects, load_manifest, merge_local_manifests,
};
