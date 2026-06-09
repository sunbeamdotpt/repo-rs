// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Info as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Get info on the manifest branch, current branch, or unmerged branches.
#[derive(Debug, Clone, Parser)]
pub struct InfoArgs {
    /// Show diff.
    #[arg(short = 'd', long)]
    pub diff: bool,

    /// Show overview.
    #[arg(short = 'o', long)]
    pub overview: bool,

    /// Show current branch.
    #[arg(short = 'c', long)]
    pub current_branch: bool,

    /// Local only.
    #[arg(short = 'l', long)]
    pub local_only: bool,

    /// Output format.
    #[arg(long, value_enum, default_value = "text")]
    pub format: InfoFormat,

    /// Include summary.
    #[arg(long)]
    pub include_summary: bool,

    /// Include projects.
    #[arg(long)]
    pub include_projects: bool,

    /// Projects to query.
    pub projects: Vec<String>,
}

/// Output format for the info command.
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum InfoFormat {
    #[default]
    /// Plain text format.
    Text,
    /// Json variant.
    Json,
}

#[async_trait::async_trait]
impl Command for InfoArgs {
    const NAME: &'static str = "info";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let opts = repo_rs_engine::InfoOptions {
            diff: self.diff,
            overview: self.overview,
            current_branch: self.current_branch,
            local_only: self.local_only,
            format: match self.format {
                InfoFormat::Text => repo_rs_engine::InfoFormat::Text,
                InfoFormat::Json => repo_rs_engine::InfoFormat::Json,
            },
            include_summary: self.include_summary,
            include_projects: self.include_projects,
            ..Default::default()
        };

        let projects = ctx.client.projects.keys().cloned().collect::<Vec<_>>();
        let engine = repo_rs_engine::DefaultInfo;
        let output = engine.info(ctx, projects, opts).await?;
        println!("{}", output);

        Ok(ExitCode::SUCCESS)
    }
}
