// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Download as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Download a specific change.
#[derive(Debug, Clone, Parser)]
pub struct DownloadArgs {
    /// Create a local branch.
    #[arg(short = 'b', long)]
    pub branch: Option<String>,

    /// Cherry-pick.
    #[arg(short = 'c', long)]
    pub cherry_pick: bool,

    /// Record origin.
    #[arg(short = 'x', long)]
    pub record_origin: bool,

    /// Revert.
    #[arg(short = 'r', long)]
    pub revert: bool,

    /// Fast-forward only.
    #[arg(short = 'f', long)]
    pub ff_only: bool,

    /// Changes to download.
    pub changes: Vec<String>,
}

#[async_trait::async_trait]
impl Command for DownloadArgs {
    const NAME: &'static str = "download";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let opts = repo_rs_engine::DownloadOptions {
            branch: self.branch.clone(),
            cherry_pick: self.cherry_pick,
            record_origin: self.record_origin,
            revert: self.revert,
            ff_only: self.ff_only,
            changes: self.changes.clone(),
            ..Default::default()
        };

        let engine = repo_rs_engine::DefaultDownload;
        engine.download(ctx, opts).await?;

        Ok(ExitCode::SUCCESS)
    }
}
