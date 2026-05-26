// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{Context, Error};

/// Options controlling selfupdate behavior.
#[derive(Debug, Clone, Default)]
pub struct SelfUpdateOptions {
    /// Force.
    pub force: bool,
    /// No repo verify.
    pub no_repo_verify: bool,
}

/// Trait for selfupdate logic.
#[async_trait::async_trait]
pub trait SelfUpdate {
    /// Update the repo tool itself.
    async fn self_update(&self, ctx: &Context, opts: SelfUpdateOptions) -> Result<(), Error>;
}

/// Default selfupdate implementation.
///
/// Fetches the latest repo tool code from the repo project's remote.
pub struct DefaultSelfUpdate;

#[async_trait::async_trait]
impl SelfUpdate for DefaultSelfUpdate {
    async fn self_update(&self, ctx: &Context, _opts: SelfUpdateOptions) -> Result<(), Error> {
        let repo_project = &ctx.client.repo_project;
        let repo = ctx
            .git
            .open(&repo_project.path)
            .await
            .map_err(|e| Error::Sync(format!("failed to open repo project: {e}")))?;

        ctx.git
            .fetch(&repo, "origin", false, false, None, &())
            .await
            .map_err(|e| Error::Sync(format!("failed to fetch repo project: {e}")))?;

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
    async fn test_selfupdate_happy_path() {
        let mut ctx = make_context();
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .withf(|p| p.as_str() == "/tmp/.repo/repo")
            .returning(|_| {
                Box::pin(async {
                    let dir = tempfile::tempdir().unwrap();
                    let p = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
                    Ok(git2::Repository::init(&p).unwrap())
                })
            });
        mock.expect_fetch()
            .times(1)
            .returning(|_, _, _, _, _, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultSelfUpdate;
        engine
            .self_update(&ctx, SelfUpdateOptions::default())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_selfupdate_open_error() {
        let mut ctx = make_context();
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|_| Box::pin(async { Err(repo_rs_git::Error::Git2("open failed".to_string())) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultSelfUpdate;
        let err = engine
            .self_update(&ctx, SelfUpdateOptions::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("failed to open repo project"));
    }

    #[tokio::test]
    async fn test_selfupdate_fetch_error() {
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
        mock.expect_fetch()
            .times(1)
            .returning(|_, _, _, _, _, _| Box::pin(async { Err(repo_rs_git::Error::Git2("fetch failed".to_string())) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultSelfUpdate;
        let err = engine
            .self_update(&ctx, SelfUpdateOptions::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("failed to fetch repo project"));
    }
}
