// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use clap::Parser;
use repo_rs_engine::CherryPick as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Cherry-pick a change.
#[derive(Debug, Clone, Parser)]
pub struct CherryPickArgs {
    /// Commit SHA to cherry-pick.
    pub sha: String,
}

#[async_trait::async_trait]
impl Command for CherryPickArgs {
    const NAME: &'static str = "cherry-pick";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let cwd = std::env::current_dir().map_err(Error::Io)?;
        let project = Utf8PathBuf::from_path_buf(cwd)
            .map_err(|_| Error::InvalidArguments("current directory is not valid UTF-8".to_string()))?;

        let opts = repo_rs_engine::CherryPickOptions {
            sha: self.sha.clone(),
            commit: self.sha.clone(),
            project: Some(project),
        };

        let engine = repo_rs_engine::DefaultCherryPick;
        engine.cherry_pick(ctx, opts).await?;

        Ok(ExitCode::SUCCESS)
    }
}
