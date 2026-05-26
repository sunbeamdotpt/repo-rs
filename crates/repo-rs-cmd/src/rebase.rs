// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Rebase as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Rebase local branches on the upstream branch.
#[derive(Debug, Clone, Parser)]
pub struct RebaseArgs {
    /// Interactive rebase.
    #[arg(short = 'i', long)]
    pub interactive: bool,

    /// Force rebase.
    #[arg(short = 'f', long)]
    pub force_rebase: bool,

    /// Autosquash.
    #[arg(long)]
    pub autosquash: bool,

    /// Auto stash.
    #[arg(long)]
    pub auto_stash: bool,

    /// Rebase onto manifest revision.
    #[arg(short = 'm', long)]
    pub onto_manifest: bool,

    /// Projects to rebase.
    pub projects: Vec<String>,
}

#[async_trait::async_trait]
impl Command for RebaseArgs {
    const NAME: &'static str = "rebase";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let opts = repo_rs_engine::RebaseOptions {
            interactive: self.interactive,
            force_rebase: self.force_rebase,
            autosquash: self.autosquash,
            auto_stash: self.auto_stash,
            onto_manifest: self.onto_manifest,
        };

        let projects = ctx.client.projects.keys().cloned().collect::<Vec<_>>();
        let engine = repo_rs_engine::DefaultRebase;
        engine.rebase(ctx, projects, opts).await?;

        Ok(ExitCode::SUCCESS)
    }
}
