// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Help as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Display detailed help.
#[derive(Debug, Clone, Parser)]
pub struct HelpArgs {
    /// Show all commands.
    #[arg(short = 'a', long)]
    pub all: bool,

    /// Command to get help for.
    pub command: Option<String>,
}

#[async_trait::async_trait]
impl Command for HelpArgs {
    const NAME: &'static str = "help";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let engine = repo_rs_engine::DefaultHelp;
        let output = engine.help(ctx, self.command.clone()).await?;
        println!("{}", output);

        Ok(ExitCode::SUCCESS)
    }
}
