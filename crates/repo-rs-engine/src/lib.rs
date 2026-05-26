// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

#![warn(missing_docs)]

//! Business logic engine for repo-rs.
//!
//! Each subcommand's behavior is defined as a trait in this crate, with
//! a default implementation provided. Consumers can replace individual
//! behaviors by implementing the trait themselves.

#![doc = include_str!("../README.md")]

/// Command execution context.
pub mod context;
/// Error types for this crate.
pub mod error;

/// The `repo abandon` command implementation.
pub mod abandon;
/// The `repo branches` command implementation.
pub mod branches;
/// The `repo checkout` command implementation.
pub mod checkout;
/// The `repo diff` command implementation.
pub mod diff;
/// The `repo diffmanifests` command implementation.
pub mod diffmanifests;
/// The `repo download` command implementation.
pub mod download;
/// The `repo forall` command implementation.
pub mod forall;
/// The `repo gc` command implementation.
pub mod gc;
/// The `repo grep` command implementation.
pub mod grep;
/// The `repo help` command implementation.
pub mod help;
/// The `repo info` command implementation.
pub mod info;
/// The `repo init` command implementation.
pub mod init;
/// The `repo list` command implementation.
pub mod list;
/// Manifest command models.
pub mod manifest;
/// The `repo overview` command implementation.
pub mod overview;
/// The `repo prune` command implementation.
pub mod prune;
/// The `repo rebase` command implementation.
pub mod rebase;
/// The `repo selfupdate` command implementation.
pub mod selfupdate;
/// The `repo smartsync` command implementation.
pub mod smartsync;
/// The `repo stage` command implementation.
pub mod stage;
/// The `repo start` command implementation.
pub mod start;
/// The `repo status` command implementation.
pub mod status;
/// The `repo sync` command implementation.
pub mod sync;
/// The `repo upload` command implementation.
pub mod upload;
/// The `repo version` command implementation.
pub mod version;
/// The `repo wipe` command implementation.
pub mod wipe;
/// The `repo cherry-pick` command implementation.
pub mod cherry_pick;

pub use context::Context;
pub use error::Error;

// Re-export common types for convenience
pub use abandon::{Abandon, AbandonOptions, DefaultAbandon};
pub use branches::{Branches, DefaultBranches};
pub use checkout::{Checkout, DefaultCheckout};
pub use diff::{DefaultDiff, Diff, DiffOptions};
pub use diffmanifests::{DefaultDiffManifests, DiffManifests, DiffManifestsOptions};
pub use download::{DefaultDownload, Download, DownloadOptions};
pub use forall::{DefaultForall, Forall, ForallOptions};
pub use gc::{DefaultGc, Gc, GcOptions};
pub use grep::{DefaultGrep, Grep, GrepOptions};
pub use help::{DefaultHelp, Help};
pub use info::{DefaultInfo, Info, InfoFormat, InfoOptions};
pub use init::{DefaultInit, Init, InitOptions};
pub use list::{DefaultList, List, ListOptions};
pub use manifest::{DefaultManifestCmd, ManifestCmd, ManifestFormat, ManifestOptions};
pub use overview::{DefaultOverview, Overview};
pub use prune::{DefaultPrune, Prune};
pub use rebase::{DefaultRebase, Rebase, RebaseOptions};
pub use selfupdate::{DefaultSelfUpdate, SelfUpdate, SelfUpdateOptions};
pub use smartsync::{DefaultSmartsync, Smartsync};
pub use stage::{DefaultStage, Stage};
pub use start::{DefaultStart, Start, StartOptions};
pub use status::{DefaultStatus, Status, StatusOptions};
pub use sync::{DefaultSync, Sync, SyncOptions, SyncReport};
pub use upload::{DefaultUpload, Upload, UploadOptions, UploadReport};
pub use version::{DefaultVersion, Version, VersionInfo};
pub use wipe::{DefaultWipe, Wipe, WipeOptions};
pub use cherry_pick::{CherryPick, CherryPickOptions, DefaultCherryPick};
