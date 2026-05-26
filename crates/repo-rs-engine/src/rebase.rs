// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use crate::{Context, Error};

/// Options controlling rebase behavior.
#[derive(Debug, Clone, Default)]
pub struct RebaseOptions {
    /// Interactive.
    pub interactive: bool,
    /// Autosquash.
    pub autosquash: bool,
    /// Force rebase.
    pub force_rebase: bool,
    /// Auto stash.
    pub auto_stash: bool,
    /// Onto manifest.
    pub onto_manifest: bool,
}

/// Trait for rebase logic.
#[async_trait::async_trait]
pub trait Rebase {
    /// Rebase topic branches onto their tracking branches.

    async fn rebase(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        opts: RebaseOptions,
    ) -> Result<(), Error>;
}

/// Default rebase implementation.
pub struct DefaultRebase;

#[async_trait::async_trait]
impl Rebase for DefaultRebase {
    async fn rebase(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        _opts: RebaseOptions,
    ) -> Result<(), Error> {
        for path in projects {
            let project = match ctx.client.project_by_path(&path) {
                Some(p) => p,
                None => {
                    return Err(Error::InvalidArguments(format!(
                        "project not found: {path}"
                    )));
                }
            };
            let repo = ctx.git.open(&project.worktree).await.map_err(|e| {
                Error::Sync(format!("failed to open {}: {e}", project.name))
            })?;

            let upstream = project
                .upstream
                .as_deref()
                .unwrap_or(&project.revision_expr);

            ctx.git.rebase(&repo, upstream).await.map_err(|e| {
                Error::Sync(format!("failed to rebase {}: {e}", project.name))
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use camino::Utf8PathBuf;
    use repo_rs_git::MockGitBackend;
    use repo_rs_model::{client::MetaProject, Project, RepoClient};
    use std::collections::HashSet;
    use std::sync::Arc;

    fn make_project(name: &str, relpath: &str, upstream: Option<&str>) -> Project {
        Project {
            name: name.to_string(),
            relpath: Utf8PathBuf::from(relpath),
            worktree: Utf8PathBuf::from(format!("/tmp/{relpath}")),
            gitdir: Utf8PathBuf::from(format!("/tmp/.repo/projects/{relpath}.git")),
            objdir: Utf8PathBuf::from(format!("/tmp/.repo/project-objects/{name}.git")),
            remote: repo_rs_model::RemoteSpec {
                name: "origin".to_string(),
                fetch_url: "https://example.com".to_string(),
                push_url: None,
                review_url: None,
                alias: None,
            },
            revision_expr: "main".to_string(),
            revision_id: None,
            groups: {
                let mut g = HashSet::new();
                g.insert("all".to_string());
                g
            },
            parent: None,
            subprojects: Vec::new(),
            copyfiles: Vec::new(),
            linkfiles: Vec::new(),
            sync_c: false,
            sync_s: false,
            sync_tags: true,
            clone_depth: None,
            upstream: upstream.map(|s| s.to_string()),
            dest_branch: None,
        }
    }

    fn make_context() -> Context {
        let mut projects = indexmap::IndexMap::new();
        projects.insert(
            Utf8PathBuf::from("foo"),
            make_project("foo", "foo", Some("origin/main")),
        );
        projects.insert(
            Utf8PathBuf::from("bar"),
            make_project("bar", "bar", None),
        );
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
                projects,
                submanifests: indexmap::IndexMap::new(),
            },
            outer_client: None,
            progress: Box::new(()),
            color_choice: anstream::ColorChoice::Auto,
            git: Arc::new(MockGitBackend::new()),
        }
    }

    #[tokio::test]
    async fn test_rebase_project_not_found() {
        let ctx = make_context();
        let engine = DefaultRebase;
        let err = engine
            .rebase(&ctx, vec![Utf8PathBuf::from("nonexistent")], RebaseOptions::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("project not found"));
    }

    #[tokio::test]
    async fn test_rebase_with_explicit_upstream() {
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
        mock.expect_rebase()
            .times(1)
            .withf(|_, upstream| upstream == "origin/main")
            .returning(|_, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultRebase;
        engine
            .rebase(&ctx, vec![Utf8PathBuf::from("foo")], RebaseOptions::default())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_rebase_falls_back_to_revision_expr() {
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
        mock.expect_rebase()
            .times(1)
            .withf(|_, upstream| upstream == "main")
            .returning(|_, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultRebase;
        engine
            .rebase(&ctx, vec![Utf8PathBuf::from("bar")], RebaseOptions::default())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_rebase_open_error() {
        let mut ctx = make_context();
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|_| Box::pin(async { Err(repo_rs_git::Error::Git2("open failed".to_string())) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultRebase;
        let err = engine
            .rebase(&ctx, vec![Utf8PathBuf::from("foo")], RebaseOptions::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("failed to open"));
    }

    #[tokio::test]
    async fn test_rebase_git_error() {
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
        mock.expect_rebase()
            .times(1)
            .returning(|_, _| Box::pin(async { Err(repo_rs_git::Error::Git2("rebase failed".to_string())) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultRebase;
        let err = engine
            .rebase(&ctx, vec![Utf8PathBuf::from("foo")], RebaseOptions::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("failed to rebase"));
    }
}
