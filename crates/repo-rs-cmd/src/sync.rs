// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Sync as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Update working tree to the latest revision.
#[derive(Debug, Clone, Parser)]
pub struct SyncArgs {
    /// Fetch submodules.
    #[arg(long)]
    pub fetch_submodules: bool,

    /// Only fetch current branch.
    #[arg(short = 'c', long)]
    pub current_branch_only: bool,

    /// Detach projects back to manifest revision.
    #[arg(short = 'd', long)]
    pub detach: bool,

    /// Perform a local sync without network access.
    #[arg(short = 'l', long)]
    pub local_only: bool,

    /// Auto rebase local changes.
    #[arg(long)]
    pub rebase: bool,

    /// Prune stale remote-tracking branches.
    #[arg(long)]
    pub prune: bool,

    /// Fetch tags.
    #[arg(long)]
    pub tags: bool,

    /// Projects to sync (default: all).
    pub projects: Vec<String>,
}

#[async_trait::async_trait]
impl Command for SyncArgs {
    const NAME: &'static str = "sync";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let opts = repo_rs_engine::SyncOptions {
            local_only: self.local_only,
            current_branch_only: self.current_branch_only,
            detach: self.detach,
            rebase: self.rebase,
            prune: self.prune,
            tags: self.tags,
            ..Default::default()
        };

        let projects = if self.projects.is_empty() {
            ctx.client.projects.keys().cloned().collect::<Vec<_>>()
        } else {
            // TODO: resolve projects from args
            ctx.client.projects.keys().cloned().collect::<Vec<_>>()
        };

        let engine = repo_rs_engine::DefaultSync;
        let _report = engine.sync(ctx, projects, opts).await?;

        Ok(ExitCode::SUCCESS)
    }
}
