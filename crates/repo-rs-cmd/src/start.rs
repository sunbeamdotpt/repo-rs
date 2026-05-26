// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Start as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Start a new branch for development.
#[derive(Debug, Clone, Parser)]
pub struct StartArgs {
    /// Revision to base the branch on.
    #[arg(short = 'r', long)]
    pub revision: Option<String>,

    /// Use HEAD as the base.
    #[arg(long)]
    pub head: bool,

    /// Branch name.
    pub branch: String,

    /// Projects to start the branch in.
    pub projects: Vec<String>,
}

#[async_trait::async_trait]
impl Command for StartArgs {
    const NAME: &'static str = "start";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let opts = repo_rs_engine::StartOptions {
            revision: self.revision.clone(),
            head: self.head,
        };

        let projects = ctx.client.projects.keys().cloned().collect::<Vec<_>>();
        let engine = repo_rs_engine::DefaultStart;
        engine.start(ctx, projects, self.branch.clone(), opts).await?;

        Ok(ExitCode::SUCCESS)
    }
}
