// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Overview as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Display overview of unmerged branches.
#[derive(Debug, Clone, Parser)]
pub struct OverviewArgs {
    /// Current branch only.
    #[arg(short = 'c', long)]
    pub current_branch: bool,

    /// Projects to query.
    pub projects: Vec<String>,
}

#[async_trait::async_trait]
impl Command for OverviewArgs {
    const NAME: &'static str = "overview";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let projects = ctx.client.projects.keys().cloned().collect::<Vec<_>>();
        let engine = repo_rs_engine::DefaultOverview;
        let output = engine.overview(ctx, projects).await?;
        tracing::info!("{}", output);

        Ok(ExitCode::SUCCESS)
    }
}
