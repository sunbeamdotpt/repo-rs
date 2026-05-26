// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

#![warn(missing_docs)]

//! The `repo` CLI binary.
//!
//! This crate provides the main `repo` command-line tool, dispatching
//! all subcommands to the appropriate `repo-rs-engine` and `repo-rs-cmd`
//! implementations.

use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use repo_rs_cmd::{Command, Context};
use std::process::ExitCode;
use std::sync::Arc;

#[derive(Debug, Parser)]
#[command(name = "repo")]
#[command(about = "A Rust implementation of the Android repo tool")]
#[command(disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Init(repo_rs_cmd::init::InitArgs),
    Sync(repo_rs_cmd::sync::SyncArgs),
    Upload(repo_rs_cmd::upload::UploadArgs),
    Download(repo_rs_cmd::download::DownloadArgs),
    Start(repo_rs_cmd::start::StartArgs),
    Status(repo_rs_cmd::status::StatusArgs),
    Diff(repo_rs_cmd::diff::DiffArgs),
    Stage(repo_rs_cmd::stage::StageArgs),
    Rebase(repo_rs_cmd::rebase::RebaseArgs),
    CherryPick(repo_rs_cmd::cherry_pick::CherryPickArgs),
    Abandon(repo_rs_cmd::abandon::AbandonArgs),
    Checkout(repo_rs_cmd::checkout::CheckoutArgs),
    Branches(repo_rs_cmd::branches::BranchesArgs),
    Forall(repo_rs_cmd::forall::ForallArgs),
    Grep(repo_rs_cmd::grep::GrepArgs),
    Manifest(repo_rs_cmd::manifest::ManifestArgs),
    Info(repo_rs_cmd::info::InfoArgs),
    List(repo_rs_cmd::list::ListArgs),
    Prune(repo_rs_cmd::prune::PruneArgs),
    Gc(repo_rs_cmd::gc::GcArgs),
    Diffmanifests(repo_rs_cmd::diffmanifests::DiffManifestsArgs),
    Wipe(repo_rs_cmd::wipe::WipeArgs),
    Selfupdate(repo_rs_cmd::selfupdate::SelfUpdateArgs),
    Smartsync(repo_rs_cmd::smartsync::SmartsyncArgs),
    Version(repo_rs_cmd::version::VersionArgs),
    Help(repo_rs_cmd::help::HelpArgs),
    Overview(repo_rs_cmd::overview::OverviewArgs),
}

fn make_minimal_context() -> Context {
    let cwd = std::env::current_dir().unwrap();
    let repo_root = Utf8PathBuf::from_path_buf(cwd).unwrap_or_else(|p| {
        Utf8PathBuf::from(p.to_string_lossy().to_string())
    });
    Context {
        repo_root,
        client: repo_rs_model::RepoClient {
            repo_dir: Utf8PathBuf::from(""),
            manifest_project: repo_rs_model::client::MetaProject {
                name: String::new(),
                path: Utf8PathBuf::from(""),
                gitdir: Utf8PathBuf::from(""),
            },
            repo_project: repo_rs_model::client::MetaProject {
                name: String::new(),
                path: Utf8PathBuf::from(""),
                gitdir: Utf8PathBuf::from(""),
            },
            projects: indexmap::IndexMap::new(),
            submanifests: indexmap::IndexMap::new(),
        },
        outer_client: None,
        progress: Box::new(()),
        color_choice: anstream::ColorChoice::Auto,
        git: Arc::new(repo_rs_git::DefaultBackend),
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_writer(std::io::stdout)
        .with_env_filter(env_filter)
        .without_time()
        .with_target(false)
        .init();

    let cli = Cli::parse();

    let result = match &cli.command {
        Commands::Init(args) => {
            let ctx = make_minimal_context();
            args.execute(&ctx).await
        }
        Commands::Version(args) => {
            let ctx = make_minimal_context();
            args.execute(&ctx).await
        }
        Commands::Help(args) => {
            let ctx = make_minimal_context();
            args.execute(&ctx).await
        }
        _ => {
            let cwd = std::env::current_dir().unwrap();
            let start = Utf8PathBuf::from_path_buf(cwd).unwrap_or_else(|p| {
                Utf8PathBuf::from(p.to_string_lossy().to_string())
            });
            match repo_rs_engine::Context::load(&start, Arc::new(repo_rs_git::DefaultBackend)) {
                Ok(ctx) => {
                    match &cli.command {
                        Commands::Sync(args) => args.execute(&ctx).await,
                        Commands::Upload(args) => args.execute(&ctx).await,
                        Commands::Download(args) => args.execute(&ctx).await,
                        Commands::Start(args) => args.execute(&ctx).await,
                        Commands::Status(args) => args.execute(&ctx).await,
                        Commands::Diff(args) => args.execute(&ctx).await,
                        Commands::Stage(args) => args.execute(&ctx).await,
                        Commands::Rebase(args) => args.execute(&ctx).await,
                        Commands::CherryPick(args) => args.execute(&ctx).await,
                        Commands::Abandon(args) => args.execute(&ctx).await,
                        Commands::Checkout(args) => args.execute(&ctx).await,
                        Commands::Branches(args) => args.execute(&ctx).await,
                        Commands::Forall(args) => args.execute(&ctx).await,
                        Commands::Grep(args) => args.execute(&ctx).await,
                        Commands::Manifest(args) => args.execute(&ctx).await,
                        Commands::Info(args) => args.execute(&ctx).await,
                        Commands::List(args) => args.execute(&ctx).await,
                        Commands::Prune(args) => args.execute(&ctx).await,
                        Commands::Gc(args) => args.execute(&ctx).await,
                        Commands::Diffmanifests(args) => args.execute(&ctx).await,
                        Commands::Wipe(args) => args.execute(&ctx).await,
                        Commands::Selfupdate(args) => args.execute(&ctx).await,
                        Commands::Smartsync(args) => args.execute(&ctx).await,
                        Commands::Overview(args) => args.execute(&ctx).await,
                        _ => unreachable!(),
                    }
                }
                Err(e) => {
                    tracing::error!("{e}");
                    return ExitCode::FAILURE;
                }
            }
        }
    };

    match result {
        Ok(code) => code,
        Err(e) => {
            tracing::error!("{e}");
            ExitCode::FAILURE
        }
    }
}
