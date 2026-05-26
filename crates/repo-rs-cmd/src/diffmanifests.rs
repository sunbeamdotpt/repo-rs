// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::DiffManifests as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Compare two manifests.
#[derive(Debug, Clone, Parser)]
pub struct DiffManifestsArgs {
    /// Raw output.
    #[arg(long)]
    pub raw: bool,

    /// Pretty format string.
    #[arg(long)]
    pub pretty_format: Option<String>,

    /// First manifest.
    pub manifest1: String,

    /// Second manifest.
    pub manifest2: Option<String>,
}

#[async_trait::async_trait]
impl Command for DiffManifestsArgs {
    const NAME: &'static str = "diffmanifests";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let opts = repo_rs_engine::DiffManifestsOptions {
            raw: self.raw,
            pretty_format: self.pretty_format.clone(),
        };

        let engine = repo_rs_engine::DefaultDiffManifests;
        let output = engine.diff_manifests(
            ctx,
            self.manifest1.clone(),
            self.manifest2.clone(),
            opts,
        ).await?;
        tracing::info!("{}", output);

        Ok(ExitCode::SUCCESS)
    }
}
