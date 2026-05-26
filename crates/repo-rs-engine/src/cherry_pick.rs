// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use crate::{Context, Error};

/// Options controlling cherry_pick behavior.
#[derive(Debug, Clone, Default)]
pub struct CherryPickOptions {
    /// Commit.
    pub commit: String,
    /// Sha.
    pub sha: String,
    /// Project path to operate on. If `None`, uses the manifest project.
    pub project: Option<Utf8PathBuf>,
}

/// Trait for cherry_pick logic.
#[async_trait::async_trait]
pub trait CherryPick {
    /// Cherry-pick a commit across projects.
    async fn cherry_pick(&self, ctx: &Context, opts: CherryPickOptions) -> Result<(), Error>;
}

/// Default cherry_pick implementation.
pub struct DefaultCherryPick;

#[async_trait::async_trait]
impl CherryPick for DefaultCherryPick {
    /// Cherry-pick a commit across projects.
    async fn cherry_pick(&self, ctx: &Context, opts: CherryPickOptions) -> Result<(), Error> {
        let path = opts
            .project
            .as_ref()
            .unwrap_or(&ctx.client.manifest_project.path)
            .clone();

        let repo = ctx.git.open(&path).await.map_err(|e| {
            Error::Sync(format!("failed to open repo at {path}: {e}"))
        })?;

        ctx.git.cherry_pick(&repo, &opts.sha).await.map_err(|e| {
            Error::Sync(format!("cherry-pick failed: {e}"))
        })?;

        Ok(())
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
    async fn test_cherry_pick_uses_manifest_project_by_default() {
        let mut ctx = make_context();
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .withf(|p| p.as_str() == "/tmp/.repo/manifests")
            .returning(|_| {
                Box::pin(async {
                    let dir = tempfile::tempdir().unwrap();
                    let p = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
                    Ok(git2::Repository::init(&p).unwrap())
                })
            });
        mock.expect_cherry_pick()
            .times(1)
            .withf(|_, sha| sha == "abc123")
            .returning(|_, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultCherryPick;
        engine
            .cherry_pick(
                &ctx,
                CherryPickOptions {
                    commit: "abc123".to_string(),
                    sha: "abc123".to_string(),
                    project: None,
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_cherry_pick_with_explicit_project() {
        let mut ctx = make_context();
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .withf(|p| p.as_str() == "/tmp/my-project")
            .returning(|_| {
                Box::pin(async {
                    let dir = tempfile::tempdir().unwrap();
                    let p = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
                    Ok(git2::Repository::init(&p).unwrap())
                })
            });
        mock.expect_cherry_pick()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultCherryPick;
        engine
            .cherry_pick(
                &ctx,
                CherryPickOptions {
                    commit: "abc123".to_string(),
                    sha: "abc123".to_string(),
                    project: Some(Utf8PathBuf::from("/tmp/my-project")),
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_cherry_pick_open_error() {
        let mut ctx = make_context();
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|_| Box::pin(async { Err(repo_rs_git::Error::Git2("open failed".to_string())) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultCherryPick;
        let err = engine
            .cherry_pick(
                &ctx,
                CherryPickOptions {
                    commit: "abc123".to_string(),
                    sha: "abc123".to_string(),
                    project: None,
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("failed to open"));
    }

    #[tokio::test]
    async fn test_cherry_pick_git_error() {
        let mut ctx = make_context();
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|_| {
                Box::pin(async {
                    let dir = tempfile::tempdir().unwrap();
                    let p = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
                    Ok(git2::Repository::init(&p).unwrap())
                })
            });
        mock.expect_cherry_pick()
            .times(1)
            .returning(|_, _| Box::pin(async { Err(repo_rs_git::Error::Git2("pick failed".to_string())) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultCherryPick;
        let err = engine
            .cherry_pick(
                &ctx,
                CherryPickOptions {
                    commit: "abc123".to_string(),
                    sha: "abc123".to_string(),
                    project: None,
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("cherry-pick failed"));
    }
}
