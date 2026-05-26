// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

#![warn(missing_docs)]

//! Core data model for the repo tool.
//!
//! Defines [`Project`], [`RemoteSpec`], [`RepoClient`], and related
//! types, plus project discovery, group filtering, and `.repo/`
//! layout introspection.

pub mod builder;
/// Repo client state management.
pub mod client;
/// Error types for this crate.
pub mod error;
/// Repo directory layout definitions.
pub mod layout;
/// Project model definitions.
pub mod project;
/// resolve module.
pub mod resolve;

pub use builder::ManifestBuilder;
pub use client::RepoClient;
pub use error::Error;
pub use project::{Project, RemoteSpec};
