// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use crate::{Context, Error};

/// Trait for stage logic.
#[async_trait::async_trait]
pub trait Stage {
    /// Stage changes in the given projects.
    async fn stage(&self, ctx: &Context, projects: Vec<Utf8PathBuf>) -> Result<(), Error>;
}

/// Default stage implementation.
pub struct DefaultStage;

#[async_trait::async_trait]
impl Stage for DefaultStage {
    /// Stage changes in the given projects.
    async fn stage(&self, ctx: &Context, projects: Vec<Utf8PathBuf>) -> Result<(), Error> {
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
            ctx.git.stage_all(&repo).await.map_err(|e| {
                Error::Sync(format!("failed to stage {}: {e}", project.name))
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

    fn make_context() -> Context {
        let mut projects = indexmap::IndexMap::new();
        projects.insert(Utf8PathBuf::from("foo"), make_project("foo", "foo"));
        projects.insert(Utf8PathBuf::from("bar"), make_project("bar", "bar"));
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
    async fn test_stage_project_not_found() {
        let ctx = make_context();
        let engine = DefaultStage;
        let err = engine
            .stage(&ctx, vec![Utf8PathBuf::from("nonexistent")])
            .await
            .unwrap_err();
        assert!(err.to_string().contains("project not found"));
    }

    #[tokio::test]
    async fn test_stage_single_project() {
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
        mock.expect_stage_all()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultStage;
        engine.stage(&ctx, vec![Utf8PathBuf::from("foo")]).await.unwrap();
    }

    #[tokio::test]
    async fn test_stage_multiple_projects() {
        let mut ctx = make_context();
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(2)
            .returning(|_| {
                Box::pin(async {
                    let dir = tempfile::tempdir().unwrap();
                    let p = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
                    Ok(git2::Repository::init(&p).unwrap())
                })
            });
        mock.expect_stage_all()
            .times(2)
            .returning(|_| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultStage;
        engine
            .stage(&ctx, vec![Utf8PathBuf::from("foo"), Utf8PathBuf::from("bar")])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_stage_open_error() {
        let mut ctx = make_context();
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|_| Box::pin(async { Err(repo_rs_git::Error::Git2("open failed".to_string())) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultStage;
        let err = engine
            .stage(&ctx, vec![Utf8PathBuf::from("foo")])
            .await
            .unwrap_err();
        assert!(err.to_string().contains("failed to open"));
    }

    #[tokio::test]
    async fn test_stage_stage_all_error() {
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
        mock.expect_stage_all()
            .times(1)
            .returning(|_| Box::pin(async { Err(repo_rs_git::Error::Git2("stage failed".to_string())) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultStage;
        let err = engine
            .stage(&ctx, vec![Utf8PathBuf::from("foo")])
            .await
            .unwrap_err();
        assert!(err.to_string().contains("failed to stage"));
    }
}
