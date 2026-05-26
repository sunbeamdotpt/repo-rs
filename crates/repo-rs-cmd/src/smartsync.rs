// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Smartsync as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Update working tree to the latest known good revision.
#[derive(Debug, Clone, Parser)]
pub struct SmartsyncArgs {
    /// Projects to sync.
    pub projects: Vec<String>,
}

#[async_trait::async_trait]
impl Command for SmartsyncArgs {
    const NAME: &'static str = "smartsync";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let projects = ctx.client.projects.keys().cloned().collect::<Vec<_>>();
        let engine = repo_rs_engine::DefaultSmartsync;
        let _report = engine.smartsync(ctx, projects).await?;

        Ok(ExitCode::SUCCESS)
    }
}
