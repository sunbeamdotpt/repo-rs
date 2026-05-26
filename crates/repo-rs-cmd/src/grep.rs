// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Grep as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Search for patterns across all projects.
#[derive(Debug, Clone, Parser)]
pub struct GrepArgs {
    /// Search index instead of working tree.
    #[arg(long)]
    pub cached: bool,

    /// Revision to search.
    #[arg(short = 'r', long)]
    pub revision: Option<String>,

    /// Ignore case.
    #[arg(short = 'i', long)]
    pub ignore_case: bool,

    /// Match whole words.
    #[arg(short = 'w', long)]
    pub word_regexp: bool,

    /// Invert match.
    #[arg(short = 'v', long)]
    pub invert_match: bool,

    /// Extended regexp.
    #[arg(short = 'E', long)]
    pub extended_regexp: bool,

    /// Fixed strings.
    #[arg(short = 'F', long)]
    pub fixed_strings: bool,

    /// Show line numbers.
    #[arg(short = 'n', long)]
    pub line_number: bool,

    /// Only show filenames with matches.
    #[arg(short = 'l', long)]
    pub files_with_matches: bool,

    /// Only show filenames without matches.
    #[arg(short = 'L', long)]
    pub files_without_match: bool,

    /// Pattern to search.
    pub pattern: String,

    /// Projects to search.
    pub projects: Vec<String>,
}

#[async_trait::async_trait]
impl Command for GrepArgs {
    const NAME: &'static str = "grep";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let opts = repo_rs_engine::GrepOptions {
            pattern: self.pattern.clone(),
            cached: self.cached,
            revision: self.revision.clone(),
            ignore_case: self.ignore_case,
            word_regexp: self.word_regexp,
            invert_match: self.invert_match,
            extended_regexp: self.extended_regexp,
            fixed_strings: self.fixed_strings,
            line_number: self.line_number,
            files_with_matches: self.files_with_matches,
            files_without_match: self.files_without_match,
        };

        let projects = ctx.client.projects.keys().cloned().collect::<Vec<_>>();
        let engine = repo_rs_engine::DefaultGrep;
        engine.grep(ctx, projects, opts).await?;

        Ok(ExitCode::SUCCESS)
    }
}
