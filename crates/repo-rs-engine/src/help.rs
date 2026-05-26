// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{Context, Error};

/// Trait for help logic.
#[async_trait::async_trait]
pub trait Help {
    /// Display help for a command.
    async fn help(&self, ctx: &Context, cmd: Option<String>) -> Result<String, Error>;
}

/// Default help implementation.
pub struct DefaultHelp;

#[async_trait::async_trait]
impl Help for DefaultHelp {
    async fn help(&self, _ctx: &Context, cmd: Option<String>) -> Result<String, Error> {
        let text = match cmd.as_deref() {
            Some("init") => "repo init - Initialize a new repo client checkout",
            Some("sync") => "repo sync - Synchronize all projects",
            Some("start") => "repo start - Start a new topic branch",
            Some("status") => "repo status - Show status of projects",
            Some("upload") => "repo upload - Upload changes for review",
            Some("forall") => "repo forall - Run a command in each project",
            Some("diff") => "repo diff - Show differences",
            Some("info") => "repo info - Show repo summary",
            Some("list") => "repo list - List all projects",
            Some("branches") => "repo branches - Show current branches",
            Some("manifest") => "repo manifest - Export manifest",
            Some("checkout") => "repo checkout - Checkout a branch",
            Some("prune") => "repo prune - Prune merged branches",
            Some("abandon") => "repo abandon - Abandon a topic branch",
            Some("version") => "repo version - Show version information",
            Some(c) => return Ok(format!("No detailed help available for: {c}")),
            None => {
                "repo-rs - A repo tool implementation\n\n\
                 Available commands:\n\
                 init, sync, start, status, upload, forall,\n\
                 diff, info, list, branches, manifest,\n\
                 checkout, prune, abandon, version, help"
            }
        };
        Ok(text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use camino::Utf8PathBuf;
    use repo_rs_git::MockGitBackend;
    use repo_rs_model::{client::MetaProject, RepoClient};
    use std::sync::Arc;

    fn make_context() -> Context {
        Context {
            repo_root: Utf8PathBuf::from("/tmp"),
            client: RepoClient {
                repo_dir: Utf8PathBuf::from("/tmp/.repo"),
                manifest_project: MetaProject {
                    name: "manifests".to_string(),
                    path: Utf8PathBuf::from("/tmp/.repo/manifests"),
                    gitdir: Utf8PathBuf::from("/tmp/.repo/manifests.git"),
                },
                repo_project: MetaProject {
                    name: "repo".to_string(),
                    path: Utf8PathBuf::from("/tmp/.repo/repo"),
                    gitdir: Utf8PathBuf::from("/tmp/.repo/repo"),
                },
                projects: indexmap::IndexMap::new(),
                submanifests: indexmap::IndexMap::new(),
            },
            outer_client: None,
            progress: Box::new(()),
            color_choice: anstream::ColorChoice::Auto,
            git: Arc::new(MockGitBackend::new()),
        }
    }

    #[tokio::test]
    async fn test_help_general() {
        let ctx = make_context();
        let engine = DefaultHelp;
        let text = engine.help(&ctx, None).await.unwrap();
        assert!(text.contains("repo-rs"));
        assert!(text.contains("init"));
        assert!(text.contains("sync"));
    }

    #[tokio::test]
    async fn test_help_specific() {
        let ctx = make_context();
        let engine = DefaultHelp;
        let text = engine.help(&ctx, Some("sync".to_string())).await.unwrap();
        assert!(text.contains("sync"));
    }

    #[tokio::test]
    async fn test_help_unknown() {
        let ctx = make_context();
        let engine = DefaultHelp;
        let text = engine.help(&ctx, Some("xyzzy".to_string())).await.unwrap();
        assert!(text.contains("xyzzy"));
    }
}
