// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use crate::{Context, Error};

/// Options controlling abandon behavior.
#[derive(Debug, Clone, Default)]
pub struct AbandonOptions {
    /// The branch name.
    pub branch: Option<String>,
    /// Whether to apply to all projects.
    pub all: bool,
}

/// Trait for abandon logic.
#[async_trait::async_trait]
pub trait Abandon {
    /// Abandon a topic branch in the given projects.
    async fn abandon(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        opts: AbandonOptions,
    ) -> Result<(), Error>;
}

/// Default abandon implementation.
pub struct DefaultAbandon;

#[async_trait::async_trait]
impl Abandon for DefaultAbandon {
    /// Abandon a topic branch in the given projects.
    async fn abandon(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        opts: AbandonOptions,
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
            let branch = match &opts.branch {
                Some(b) => b.clone(),
                None => ctx
                    .git
                    .head_name(&repo)
                    .await
                    .ok()
                    .flatten()
                    .ok_or_else(|| {
                        Error::Sync(format!(
                            "cannot determine current branch for {}",
                            project.name
                        ))
                    })?,
            };
            let upstream = project
                .upstream
                .clone()
                .unwrap_or_else(|| project.revision_expr.clone());
            ctx.git.checkout(&repo, &upstream).await.map_err(|e| {
                Error::Sync(format!(
                    "failed to checkout upstream in {}: {e}",
                    project.name
                ))
            })?;
            ctx.git
                .ref_update(&repo, &format!("refs/heads/{branch}"), "")
                .await
                .map_err(|e| {
                    Error::Sync(format!(
                        "failed to delete branch {branch} in {}: {e}",
                        project.name
                    ))
                })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use repo_rs_git::MockGitBackend;
    use repo_rs_model::{client::MetaProject, Project, RepoClient};
    use std::collections::HashSet;
    use std::sync::Arc;

    fn make_project(name: &str, relpath: &str) -> Project {
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
            upstream: None,
            dest_branch: None,
        }
    }

    fn make_context(tmp: &tempfile::TempDir) -> Context {
        let repo_root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let repo_dir = repo_root.join(".repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        let mut projects = indexmap::IndexMap::new();
        projects.insert(Utf8PathBuf::from("foo"), make_project("foo", "foo"));
        Context {
            repo_root: repo_root.clone(),
            client: RepoClient {
                repo_dir,
                manifest_project: MetaProject {
                    name: "manifests".to_string(),
                    path: repo_root.join(".repo/manifests"),
                    gitdir: repo_root.join(".repo/manifests.git"),
                },
                repo_project: MetaProject {
                    name: "repo".to_string(),
                    path: repo_root.join(".repo/repo"),
                    gitdir: repo_root.join(".repo/repo"),
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
    async fn test_abandon_with_explicit_branch() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = make_context(&tmp);
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
        mock.expect_checkout()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        mock.expect_ref_update()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultAbandon;
        engine
            .abandon(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                AbandonOptions {
                    branch: Some("feature".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_abandon_missing_project() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp);
        let engine = DefaultAbandon;
        let err = engine
            .abandon(&ctx, vec![Utf8PathBuf::from("missing")], AbandonOptions::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("project not found"));
    }

    #[tokio::test]
    async fn test_abandon_infers_branch_from_head() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = make_context(&tmp);
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
        mock.expect_head_name()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some("feature".to_string())) }));
        mock.expect_checkout()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        mock.expect_ref_update()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultAbandon;
        engine
            .abandon(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                AbandonOptions::default(),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_abandon_head_name_error() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = make_context(&tmp);
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
        mock.expect_head_name()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultAbandon;
        let err = engine
            .abandon(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                AbandonOptions::default(),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("cannot determine current branch"));
    }

    #[tokio::test]
    async fn test_abandon_open_error() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = make_context(&tmp);
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|_| Box::pin(async { Err(repo_rs_git::Error::Git2("open failed".to_string())) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultAbandon;
        let err = engine
            .abandon(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                AbandonOptions {
                    branch: Some("feature".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("failed to open"));
    }

    #[tokio::test]
    async fn test_abandon_checkout_error() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = make_context(&tmp);
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
        mock.expect_checkout()
            .times(1)
            .returning(|_, _| Box::pin(async { Err(repo_rs_git::Error::Git2("checkout failed".to_string())) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultAbandon;
        let err = engine
            .abandon(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                AbandonOptions {
                    branch: Some("feature".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("failed to checkout"));
    }

    #[tokio::test]
    async fn test_abandon_ref_update_error() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = make_context(&tmp);
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
        mock.expect_checkout()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        mock.expect_ref_update()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Err(repo_rs_git::Error::Git2("delete failed".to_string())) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultAbandon;
        let err = engine
            .abandon(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                AbandonOptions {
                    branch: Some("feature".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("failed to delete branch"));
    }
}
