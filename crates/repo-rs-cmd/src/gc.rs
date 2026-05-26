// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Gc as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Clean up internal repo and Git state.
#[derive(Debug, Clone, Parser)]
pub struct GcArgs {
    /// Dry run.
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Repack.
    #[arg(long)]
    pub repack: bool,
}

#[async_trait::async_trait]
impl Command for GcArgs {
    const NAME: &'static str = "gc";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let opts = repo_rs_engine::GcOptions {
            dry_run: self.dry_run,
            repack: self.repack,
            ..Default::default()
        };

        let engine = repo_rs_engine::DefaultGc;
        engine.gc(ctx, opts).await?;

        Ok(ExitCode::SUCCESS)
    }
}
