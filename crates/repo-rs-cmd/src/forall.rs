// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Forall as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Run a shell command in each project.
#[derive(Debug, Clone, Parser)]
pub struct ForallArgs {
    /// Regex to filter projects.
    #[arg(short = 'r', long)]
    pub regex: Option<String>,

    /// Inverse regex to filter projects.
    #[arg(short = 'i', long)]
    pub inverse_regex: Option<String>,

    /// Group filter.
    #[arg(short = 'g', long)]
    pub groups: Vec<String>,

    /// Command to run.
    #[arg(short = 'c', long)]
    pub command: String,

    /// Abort on errors.
    #[arg(short = 'e', long)]
    pub abort_on_errors: bool,

    /// Show project header.
    #[arg(short = 'p', long)]
    pub project_header: bool,

    /// Interactive.
    #[arg(long)]
    pub interactive: bool,
}

#[async_trait::async_trait]
impl Command for ForallArgs {
    const NAME: &'static str = "forall";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let opts = repo_rs_engine::ForallOptions {
            command: self.command.clone(),
            regex: self.regex.clone(),
            inverse_regex: self.inverse_regex.clone(),
            groups: self.groups.clone(),
            abort_on_errors: self.abort_on_errors,
            project_header: self.project_header,
            interactive: self.interactive,
        };

        let projects = ctx.client.projects.keys().cloned().collect::<Vec<_>>();
        let engine = repo_rs_engine::DefaultForall;
        engine.forall(ctx, projects, opts).await?;

        Ok(ExitCode::SUCCESS)
    }
}
