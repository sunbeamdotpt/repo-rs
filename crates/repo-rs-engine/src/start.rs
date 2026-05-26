// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use crate::{Context, Error};

/// Options controlling start behavior.
#[derive(Debug, Clone, Default)]
pub struct StartOptions {
    /// The git revision (branch, tag, or commit SHA).
    pub revision: Option<String>,
    /// The current HEAD commit.
    pub head: bool,
}

/// Trait for start logic.
#[async_trait::async_trait]
pub trait Start {
    /// Start a new topic branch in the given projects.

    async fn start(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        branch: String,
        opts: StartOptions,
    ) -> Result<(), Error>;
}

/// Default start implementation.
pub struct DefaultStart;

#[async_trait::async_trait]
impl Start for DefaultStart {
    async fn start(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        branch: String,
        opts: StartOptions,
    ) -> Result<(), Error> {
        for path in projects {
            let project = ctx.client.project_by_path(&path).ok_or_else(|| {
                Error::InvalidArguments(format!("project not found: {path}"))
            })?;

            let repo = ctx.git.open(&project.worktree).await?;

            let target = if opts.head {
                ctx.git.ref_resolve(&repo, "HEAD").await?
            } else {
                let rev = opts
                    .revision
                    .clone()
                    .unwrap_or_else(|| project.revision_expr.clone());
                ctx.git.ref_resolve(&repo, &rev).await?
            };

            let ref_name = format!("refs/heads/{branch}");
            ctx.git.ref_update(&repo, &ref_name, &target).await?;
            ctx.git.checkout(&repo, &branch).await?;
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
    use tempfile::tempdir;

    fn make_project(name: &str, relpath: &str) -> Project {
        Project {
            name: name.to_string(),
            relpath: Utf8PathBuf::from(relpath),
            worktree: Utf8PathBuf::from(format!("/tmp/repo/{relpath}")),
            gitdir: Utf8PathBuf::from(format!("/tmp/repo/.repo/projects/{relpath}.git")),
            objdir: Utf8PathBuf::from(format!("/tmp/repo/.repo/project-objects/{name}.git")),
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

    fn make_context(tmp: &tempfile::TempDir, projects: Vec<Project>) -> Context {
        let repo_root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let repo_dir = repo_root.join(".repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        let mut map = indexmap::IndexMap::new();
        for p in &projects {
            let mut proj = p.clone();
            proj.worktree = repo_root.join(&p.relpath);
            map.insert(p.relpath.clone(), proj);
        }
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
                projects: map,
                submanifests: indexmap::IndexMap::new(),
            },
            outer_client: None,
            progress: Box::new(()),
            color_choice: anstream::ColorChoice::Auto,
            git: Arc::new(MockGitBackend::new()),
        }
    }

    #[tokio::test]
    async fn test_start_from_manifest_revision() {
        let tmp = tempdir().unwrap();
        let mut ctx = make_context(&tmp, vec![make_project("foo", "foo")]);
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|path| {
                let _path = path.clone();
                Box::pin(async move {
                    let dir = tempdir().unwrap();
                    let p = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
                    Ok(git2::Repository::init(&p).unwrap())
                })
            });
        mock.expect_ref_resolve()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("abc123".to_string()) }));
        mock.expect_ref_update()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(()) }));
        mock.expect_checkout()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultStart;
        engine
            .start(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                "feature".to_string(),
                StartOptions::default(),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_start_from_specified_revision() {
        let tmp = tempdir().unwrap();
        let mut ctx = make_context(&tmp, vec![make_project("foo", "foo")]);
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|_| {
                Box::pin(async {
                    let dir = tempdir().unwrap();
                    let p = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
                    Ok(git2::Repository::init(&p).unwrap())
                })
            });
        mock.expect_ref_resolve()
            .times(1)
            .withf(|_, name| name == "v1.0")
            .returning(|_, _| Box::pin(async { Ok("def456".to_string()) }));
        mock.expect_ref_update()
            .times(1)
            .withf(|_, name, _| name == "refs/heads/feature")
            .returning(|_, _, _| Box::pin(async { Ok(()) }));
        mock.expect_checkout()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultStart;
        engine
            .start(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                "feature".to_string(),
                StartOptions {
                    revision: Some("v1.0".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_start_from_head() {
        let tmp = tempdir().unwrap();
        let mut ctx = make_context(&tmp, vec![make_project("foo", "foo")]);
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|_| {
                Box::pin(async {
                    let dir = tempdir().unwrap();
                    let p = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
                    Ok(git2::Repository::init(&p).unwrap())
                })
            });
        mock.expect_ref_resolve()
            .times(1)
            .withf(|_, name| name == "HEAD")
            .returning(|_, _| Box::pin(async { Ok("headsha".to_string()) }));
        mock.expect_ref_update()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(()) }));
        mock.expect_checkout()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultStart;
        engine
            .start(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                "feature".to_string(),
                StartOptions {
                    head: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_start_missing_project() {
        let tmp = tempdir().unwrap();
        let ctx = make_context(&tmp, vec![]);
        let engine = DefaultStart;
        let err = engine
            .start(
                &ctx,
                vec![Utf8PathBuf::from("missing")],
                "feature".to_string(),
                StartOptions::default(),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("project not found"));
    }

    #[tokio::test]
    async fn test_start_git_error() {
        let tmp = tempdir().unwrap();
        let mut ctx = make_context(&tmp, vec![make_project("foo", "foo")]);
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|_| Box::pin(async { Err(repo_rs_git::Error::Git2("open failed".to_string())) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultStart;
        let err = engine
            .start(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                "feature".to_string(),
                StartOptions::default(),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("open failed"));
    }

    #[tokio::test]
    async fn test_start_ref_resolve_error() {
        let tmp = tempdir().unwrap();
        let mut ctx = make_context(&tmp, vec![make_project("foo", "foo")]);
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|_| {
                Box::pin(async {
                    let dir = tempdir().unwrap();
                    let p = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
                    Ok(git2::Repository::init(&p).unwrap())
                })
            });
        mock.expect_ref_resolve()
            .times(1)
            .returning(|_, _| Box::pin(async { Err(repo_rs_git::Error::RefNotFound("main".to_string())) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultStart;
        let err = engine
            .start(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                "feature".to_string(),
                StartOptions::default(),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("ref not found"));
    }
}
