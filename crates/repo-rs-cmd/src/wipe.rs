// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Wipe as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Delete projects from the worktree.
#[derive(Debug, Clone, Parser)]
pub struct WipeArgs {
    /// Force.
    #[arg(short = 'f', long)]
    pub force: bool,

    /// Force uncommitted.
    #[arg(long)]
    pub force_uncommitted: bool,

    /// Force shared.
    #[arg(long)]
    pub force_shared: bool,

    /// Projects to wipe.
    pub projects: Vec<String>,
}

#[async_trait::async_trait]
impl Command for WipeArgs {
    const NAME: &'static str = "wipe";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let opts = repo_rs_engine::WipeOptions {
            force: self.force,
            force_uncommitted: self.force_uncommitted,
            force_shared: self.force_shared,
        };

        let projects = ctx.client.projects.keys().cloned().collect::<Vec<_>>();
        let engine = repo_rs_engine::DefaultWipe;
        engine.wipe(ctx, projects, opts).await?;

        Ok(ExitCode::SUCCESS)
    }
}
