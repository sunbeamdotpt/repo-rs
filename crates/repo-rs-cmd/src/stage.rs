// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Stage as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Stage file(s) for commit.
#[derive(Debug, Clone, Parser)]
pub struct StageArgs {
    /// Projects to stage.
    pub projects: Vec<String>,
}

#[async_trait::async_trait]
impl Command for StageArgs {
    const NAME: &'static str = "stage";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let projects = ctx.client.projects.keys().cloned().collect::<Vec<_>>();
        let engine = repo_rs_engine::DefaultStage;
        engine.stage(ctx, projects).await?;

        Ok(ExitCode::SUCCESS)
    }
}
