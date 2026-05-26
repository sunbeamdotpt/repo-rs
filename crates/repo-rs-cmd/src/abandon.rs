// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Abandon as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Permanently abandon a development branch.
#[derive(Debug, Clone, Parser)]
pub struct AbandonArgs {
    /// Abandon all branches.
    #[arg(long)]
    pub all: bool,

    /// Branch to abandon.
    pub branch: Option<String>,

    /// Projects to affect.
    pub projects: Vec<String>,
}

#[async_trait::async_trait]
impl Command for AbandonArgs {
    const NAME: &'static str = "abandon";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let opts = repo_rs_engine::AbandonOptions {
            all: self.all,
            branch: self.branch.clone(),
        };

        let projects = ctx.client.projects.keys().cloned().collect::<Vec<_>>();
        let engine = repo_rs_engine::DefaultAbandon;
        engine.abandon(ctx, projects, opts).await?;

        Ok(ExitCode::SUCCESS)
    }
}
