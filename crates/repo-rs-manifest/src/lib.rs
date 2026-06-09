// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

#![warn(missing_docs)]

//! Manifest XML parsing for the repo tool.
//!
//! This crate handles parsing of `manifest.xml` and related files,
//! including `<include>` resolution, local manifest merging,
//! validation, and serialization.

/// Error types for manifest operations.
pub mod error;
/// Data models and core types.
pub mod model;
/// Manifest XML parsing.
pub mod parser;
/// Manifest resolution and merging.
pub mod resolver;
/// Manifest XML serialization.
pub mod serialize;
/// Manifest validation utilities.
pub mod validate;

pub use error::Error;
pub use model::Manifest;
pub use parser::{parse_manifest, validate_manifest};
pub use resolver::{
    apply_extend_projects, apply_remove_projects, inject_auto_groups,
    inject_submanifest_groups, load_manifest, merge_local_manifests, normalize_url,
    resolve_fetch_url, resolve_remote_name, LOCAL_MANIFEST_GROUP_PREFIX,
    MAX_SUBMANIFEST_DEPTH, SUBMANIFEST_GROUP_PREFIX,
};
pub use serialize::manifest_to_xml;
