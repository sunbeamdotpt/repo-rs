// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::ManifestCmd as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Manifest inspection utility.
#[derive(Debug, Clone, Parser)]
pub struct ManifestArgs {
    /// Replace revisions with HEAD.
    #[arg(short = 'r', long)]
    pub revision_as_head: bool,

    /// Output format.
    #[arg(long, value_enum, default_value = "xml")]
    pub format: ManifestFormat,

    /// Pretty print JSON.
    #[arg(long)]
    pub pretty: bool,

    /// Ignore local manifests.
    #[arg(long)]
    pub no_local_manifests: bool,
}

/// Output format for the manifest command.
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum ManifestFormat {
    #[default]
    /// XML format.
    Xml,
    /// Json variant.
    Json,
}

#[async_trait::async_trait]
impl Command for ManifestArgs {
    const NAME: &'static str = "manifest";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let opts = repo_rs_engine::ManifestOptions {
            revision_as_head: self.revision_as_head,
            format: match self.format {
                ManifestFormat::Xml => repo_rs_engine::ManifestFormat::Xml,
                ManifestFormat::Json => repo_rs_engine::ManifestFormat::Json,
            },
            pretty: self.pretty,
            no_local_manifests: self.no_local_manifests,
            ..Default::default()
        };

        let engine = repo_rs_engine::DefaultManifestCmd;
        let output = engine.manifest(ctx, opts).await?;
        tracing::info!("{}", output);

        Ok(ExitCode::SUCCESS)
    }
}
