// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Version as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Display the version of repo.
#[derive(Debug, Clone, Parser)]
pub struct VersionArgs;

#[async_trait::async_trait]
impl Command for VersionArgs {
    const NAME: &'static str = "version";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let engine = repo_rs_engine::DefaultVersion;
        let info = engine.version(ctx).await?;
        tracing::info!("repo version {}", info.repo_version);

        Ok(ExitCode::SUCCESS)
    }
}
