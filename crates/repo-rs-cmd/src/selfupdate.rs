// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::SelfUpdate as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Update repo to the latest version.
#[derive(Debug, Clone, Parser)]
pub struct SelfUpdateArgs {
    /// Skip repo verification.
    #[arg(long)]
    pub no_repo_verify: bool,
}

#[async_trait::async_trait]
impl Command for SelfUpdateArgs {
    const NAME: &'static str = "selfupdate";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let opts = repo_rs_engine::SelfUpdateOptions {
            no_repo_verify: self.no_repo_verify,
            ..Default::default()
        };

        let engine = repo_rs_engine::DefaultSelfUpdate;
        engine.self_update(ctx, opts).await?;

        Ok(ExitCode::SUCCESS)
    }
}
