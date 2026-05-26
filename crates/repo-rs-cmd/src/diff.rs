// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Diff as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Show changes between commit and working tree.
#[derive(Debug, Clone, Parser)]
pub struct DiffArgs {
    /// Show absolute paths.
    #[arg(short = 'u', long)]
    pub absolute: bool,

    /// Projects to diff.
    pub projects: Vec<String>,
}

#[async_trait::async_trait]
impl Command for DiffArgs {
    const NAME: &'static str = "diff";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let opts = repo_rs_engine::DiffOptions {
            absolute: self.absolute,
            ..Default::default()
        };

        let projects = ctx.client.projects.keys().cloned().collect::<Vec<_>>();
        let engine = repo_rs_engine::DefaultDiff;
        let output = engine.diff(ctx, projects, opts).await?;
        tracing::info!("{}", output);

        Ok(ExitCode::SUCCESS)
    }
}
