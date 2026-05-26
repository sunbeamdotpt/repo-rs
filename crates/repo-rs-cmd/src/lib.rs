// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

#![warn(missing_docs)]

//! Command traits and default CLI implementations for repo-rs.
//!
//! This crate defines the [`Command`] trait that every subcommand must
//! implement, and provides default `clap`-derived structs for all
//! standard repo subcommands. Consumers can replace either the CLI
//! struct, the engine implementation, or both.

#![doc = include_str!("../README.md")]

/// Command execution context.
pub mod context;
/// Error types for this crate.
pub mod error;
/// traits module.
pub mod traits;

/// The `repo sync` command implementation.
pub mod sync;
/// The `repo init` command implementation.
pub mod init;
/// The `repo upload` command implementation.
pub mod upload;
/// The `repo start` command implementation.
pub mod start;
/// The `repo status` command implementation.
pub mod status;
/// The `repo diff` command implementation.
pub mod diff;
/// The `repo rebase` command implementation.
pub mod rebase;
/// The `repo forall` command implementation.
pub mod forall;
/// Manifest command models.
pub mod manifest;
/// The `repo info` command implementation.
pub mod info;
/// The `repo download` command implementation.
pub mod download;
/// The `repo grep` command implementation.
pub mod grep;
/// The `repo prune` command implementation.
pub mod prune;
/// The `repo abandon` command implementation.
pub mod abandon;
/// The `repo checkout` command implementation.
pub mod checkout;
/// The `repo branches` command implementation.
pub mod branches;
/// The `repo gc` command implementation.
pub mod gc;
/// The `repo list` command implementation.
pub mod list;
/// The `repo diffmanifests` command implementation.
pub mod diffmanifests;
/// The `repo version` command implementation.
pub mod version;
/// The `repo help` command implementation.
pub mod help;
/// The `repo selfupdate` command implementation.
pub mod selfupdate;
/// The `repo smartsync` command implementation.
pub mod smartsync;
/// The `repo stage` command implementation.
pub mod stage;
/// The `repo overview` command implementation.
pub mod overview;
/// The `repo wipe` command implementation.
pub mod wipe;
/// The `repo cherry-pick` command implementation.
pub mod cherry_pick;

pub use context::Context;
pub use error::Error;
pub use traits::Command;
