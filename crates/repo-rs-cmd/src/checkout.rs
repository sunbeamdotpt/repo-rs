// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use repo_rs_engine::Checkout as _;
use std::process::ExitCode;

use crate::{Command, Context, Error};

/// Checkout an existing branch.
#[derive(Debug, Clone, Parser)]
pub struct CheckoutArgs {
    /// Branch to checkout.
    pub branch: String,

    /// Projects to checkout.
    pub projects: Vec<String>,
}

#[async_trait::async_trait]
impl Command for CheckoutArgs {
    const NAME: &'static str = "checkout";
    const COMMON: bool = true;

    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
        let projects = ctx.client.projects.keys().cloned().collect::<Vec<_>>();
        let engine = repo_rs_engine::DefaultCheckout;
        engine.checkout(ctx, projects, self.branch.clone()).await?;

        Ok(ExitCode::SUCCESS)
    }
}
