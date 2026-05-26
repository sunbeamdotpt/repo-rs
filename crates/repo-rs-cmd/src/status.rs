// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Status as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Show the working tree status.
#[derive(Debug, Clone, Parser)]
pub struct StatusArgs {
    /// Show orphan projects.
    #[arg(short = 'o', long)]
    pub orphans: bool,

    /// Projects to check.
    pub projects: Vec<String>,
}

#[async_trait::async_trait]
impl Command for StatusArgs {
    const NAME: &'static str = "status";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let opts = repo_rs_engine::StatusOptions {
            orphans: self.orphans,
        };

        let projects = ctx.client.projects.keys().cloned().collect::<Vec<_>>();
        let engine = repo_rs_engine::DefaultStatus;
        let report = engine.status(ctx, projects, opts).await?;
        for r in &report {
            let branch = r.branch.as_deref().unwrap_or("(detached)");
            let dirty = if r.dirty { " *" } else { "" };
            tracing::info!("{}: {}{}", r.project, branch, dirty);
        }

        Ok(ExitCode::SUCCESS)
    }
}
