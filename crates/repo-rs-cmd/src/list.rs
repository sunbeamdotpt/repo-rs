// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::List as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// List projects and their associated directories.
#[derive(Debug, Clone, Parser)]
pub struct ListArgs {
    /// Regex filter.
    #[arg(short = 'r', long)]
    pub regex: Option<String>,

    /// Group filter.
    #[arg(short = 'g', long)]
    pub groups: Vec<String>,

    /// Only show names.
    #[arg(short = 'n', long)]
    pub name_only: bool,

    /// Only show paths.
    #[arg(short = 'p', long)]
    pub path_only: bool,

    /// Show full paths.
    #[arg(short = 'f', long)]
    pub fullpath: bool,
}

#[async_trait::async_trait]
impl Command for ListArgs {
    const NAME: &'static str = "list";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let opts = repo_rs_engine::ListOptions {
            regex: self.regex.clone(),
            groups: self.groups.clone(),
            name_only: self.name_only,
            path_only: self.path_only,
            fullpath: self.fullpath,
            ..Default::default()
        };

        let engine = repo_rs_engine::DefaultList;
        let output = engine.list(ctx, opts).await?;
        println!("{}", output);

        Ok(ExitCode::SUCCESS)
    }
}
