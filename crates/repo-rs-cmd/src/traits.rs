// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::error::Error;
use crate::Context;
use std::process::ExitCode;

/// Core trait for all repo commands.
///
/// Every subcommand implements this trait. The [`crate::Context`] is
/// passed to [`Command::execute`] so that commands can access the
/// repo client, manifest, progress callbacks, etc.
///
/// # Example
///
/// ```rust,ignore
/// use repo_rs_cmd::{Command, Context, Error};
/// use std::process::ExitCode;
///
/// struct MySync;
/// #[async_trait::async_trait]
/// impl Command for MySync {
///     const NAME: &'static str = "sync";
///     const COMMON: bool = true;
///
///     async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error> {
///         // custom sync logic
///         Ok(ExitCode::SUCCESS)
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait Command {
    /// Command name used for dispatch and help text.
    const NAME: &'static str;

    /// Whether this command is shown in short help listings.
    const COMMON: bool = false;

    /// Execute the command.
    ///
    /// # Errors
    ///
    /// Returns an error if the command fails. The caller is responsible
    /// for mapping errors to appropriate exit codes.
    async fn execute(&self, ctx: &Context) -> Result<ExitCode, Error>;
}
