// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Branches as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Show available topic branches.
#[derive(Debug, Clone, Parser)]
pub struct BranchesArgs {
    /// Projects to query.
    pub projects: Vec<String>,
}

#[async_trait::async_trait]
impl Command for BranchesArgs {
    const NAME: &'static str = "branches";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let projects = ctx.client.projects.keys().cloned().collect::<Vec<_>>();
        let engine = repo_rs_engine::DefaultBranches;
        let output = engine.branches(ctx, projects).await?;
        tracing::info!("{}", output);

        Ok(ExitCode::SUCCESS)
    }
}
