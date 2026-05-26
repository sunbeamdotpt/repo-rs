// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Prune as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Delete already-merged topic branches.
#[derive(Debug, Clone, Parser)]
pub struct PruneArgs {
    /// Projects to prune.
    pub projects: Vec<String>,
}

#[async_trait::async_trait]
impl Command for PruneArgs {
    const NAME: &'static str = "prune";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let projects = ctx.client.projects.keys().cloned().collect::<Vec<_>>();
        let engine = repo_rs_engine::DefaultPrune;
        engine.prune(ctx, projects).await?;

        Ok(ExitCode::SUCCESS)
    }
}
