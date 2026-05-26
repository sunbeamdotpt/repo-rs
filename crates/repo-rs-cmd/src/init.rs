// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Init as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Initialize a repo client checkout.
#[derive(Debug, Clone, Parser)]
pub struct InitArgs {
    /// Manifest repository URL.
    #[arg(long)]
    pub manifest_url: String,

    /// Manifest branch.
    #[arg(short = 'b', long)]
    pub manifest_branch: Option<String>,

    /// Manifest file name.
    #[arg(short = 'm', long)]
    pub manifest_name: Option<String>,

    /// Create a mirror checkout.
    #[arg(long)]
    pub mirror: bool,

    /// Use git worktrees.
    #[arg(long)]
    pub worktree: bool,

    /// Reference repo for object sharing.
    #[arg(long)]
    pub reference: Option<String>,

    /// Create a shallow clone.
    #[arg(long)]
    pub depth: Option<u32>,

    /// Enable partial clone.
    #[arg(long)]
    pub partial_clone: bool,

    /// Repo tool URL.
    #[arg(long)]
    pub repo_url: Option<String>,

    /// Repo tool revision.
    #[arg(long)]
    pub repo_rev: Option<String>,

    /// Force init even if .repo/ already exists.
    #[arg(short, long)]
    pub force: bool,
}

#[async_trait::async_trait]
impl Command for InitArgs {
    const NAME: &'static str = "init";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let opts = repo_rs_engine::InitOptions {
            manifest_url: self.manifest_url.clone(),
            manifest_branch: self.manifest_branch.clone(),
            manifest_name: self.manifest_name.clone(),
            mirror: self.mirror,
            worktree: self.worktree,
            reference: self.reference.clone(),
            depth: self.depth,
            partial_clone: self.partial_clone,
            repo_url: self.repo_url.clone(),
            repo_rev: self.repo_rev.clone(),
            force: self.force,
            ..Default::default()
        };

        let engine = repo_rs_engine::DefaultInit;
        let _client = engine.init(ctx, opts).await?;

        Ok(ExitCode::SUCCESS)
    }
}
