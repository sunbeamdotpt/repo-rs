// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Upload as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Upload changes for code review.
#[derive(Debug, Clone, Parser)]
pub struct UploadArgs {
    /// Topic branch name.
    #[arg(short = 't', long)]
    pub topic: Option<String>,

    /// Hashtags.
    #[arg(long)]
    pub hashtag: Vec<String>,

    /// Labels.
    #[arg(short = 'l', long)]
    pub label: Vec<String>,

    /// Reviewers.
    #[arg(long)]
    pub reviewers: Vec<String>,

    /// CC recipients.
    #[arg(long)]
    pub cc: Vec<String>,

    /// Upload as private.
    #[arg(short = 'p', long)]
    pub private: bool,

    /// Upload as WIP.
    #[arg(short = 'w', long)]
    pub wip: bool,

    /// Mark as ready.
    #[arg(short = 'r', long)]
    pub ready: bool,

    /// Dry run.
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Projects to upload.
    pub projects: Vec<String>,
}

#[async_trait::async_trait]
impl Command for UploadArgs {
    const NAME: &'static str = "upload";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let opts = repo_rs_engine::UploadOptions {
            topic: self.topic.clone(),
            hashtags: self.hashtag.clone(),
            labels: self.label.clone(),
            reviewers: self.reviewers.clone(),
            cc: self.cc.clone(),
            private: self.private,
            wip: self.wip,
            ready: self.ready,
            dry_run: self.dry_run,
            ..Default::default()
        };

        let projects = ctx.client.projects.keys().cloned().collect::<Vec<_>>();
        let engine = repo_rs_engine::DefaultUpload;
        let report = engine.upload(ctx, projects, opts).await?;
        for pushed in &report.pushed {
            tracing::info!("{}", pushed);
        }
        for error in &report.errors {
            tracing::error!("{}", error);
        }

        Ok(ExitCode::SUCCESS)
    }
}
