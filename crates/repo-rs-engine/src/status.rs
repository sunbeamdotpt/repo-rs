// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use crate::{Context, Error};

/// Options controlling status behavior.
#[derive(Debug, Clone, Default)]
pub struct StatusOptions {
    /// Orphans.
    pub orphans: bool,
}

/// Per-project status report.
#[derive(Debug, Clone, Default)]
pub struct StatusReport {
    /// Project.
    pub project: String,
    /// The branch name.
    pub branch: Option<String>,
    /// Commits ahead.
    pub commits_ahead: usize,
    /// Commits behind.
    pub commits_behind: usize,
    /// Whether the worktree has uncommitted changes.
    pub dirty: bool,
}

/// Trait for status logic.
#[async_trait::async_trait]
pub trait Status {
    /// Show the working tree status for the given projects.
    async fn status(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        opts: StatusOptions,
    ) -> Result<Vec<StatusReport>, Error>;
}

/// Default status implementation.
pub struct DefaultStatus;

#[async_trait::async_trait]
impl Status for DefaultStatus {
    async fn status(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        _opts: StatusOptions,
    ) -> Result<Vec<StatusReport>, Error> {
        let mut reports = Vec::new();

        for path in projects {
            let project = match ctx.client.project_by_path(&path) {
                Some(p) => p,
                None => {
                    return Err(Error::InvalidArguments(format!(
                        "project not found: {path}"
                    )));
                }
            };

            let repo = match ctx.git.open(&project.worktree).await {
                Ok(r) => r,
                Err(e) => {
                    return Err(Error::Sync(format!(
                        "failed to open {}: {e}",
                        project.name
                    )));
                }
            };

            let branch = ctx.git.head_name(&repo).await.ok().flatten();
            let dirty = ctx.git.is_dirty(&repo).await.unwrap_or(false);

            let upstream = project
                .upstream
                .clone()
                .unwrap_or_else(|| project.revision_expr.clone());
            let (commits_ahead, commits_behind) = ctx
                .git
                .ahead_behind(&repo, &upstream)
                .await
                .unwrap_or((0, 0));

            reports.push(StatusReport {
                project: project.name.clone(),
                branch,
                commits_ahead,
                commits_behind,
                dirty,
            });
        }

        Ok(reports)
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
            upstream: Some("origin/main".to_string()),
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
    async fn test_status_empty() {
        let tmp = tempdir().unwrap();
        let ctx = make_context(&tmp, vec![]);
        let engine = DefaultStatus;
        let reports = engine.status(&ctx, vec![], StatusOptions::default()).await.unwrap();
        assert!(reports.is_empty());
    }

    #[tokio::test]
    async fn test_status_happy_path() {
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
        mock.expect_head_name()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some("feature".to_string())) }));
        mock.expect_is_dirty()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        mock.expect_ahead_behind()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok((2, 1)) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultStatus;
        let reports = engine
            .status(&ctx, vec![Utf8PathBuf::from("foo")], StatusOptions::default())
            .await
            .unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].project, "foo");
        assert_eq!(reports[0].branch, Some("feature".to_string()));
        assert!(reports[0].dirty);
        assert_eq!(reports[0].commits_ahead, 2);
        assert_eq!(reports[0].commits_behind, 1);
    }

    #[tokio::test]
    async fn test_status_detached_head() {
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
        mock.expect_head_name()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));
        mock.expect_is_dirty()
            .times(1)
            .returning(|_| Box::pin(async { Ok(false) }));
        mock.expect_ahead_behind()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok((0, 0)) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultStatus;
        let reports = engine
            .status(&ctx, vec![Utf8PathBuf::from("foo")], StatusOptions::default())
            .await
            .unwrap();
        assert_eq!(reports[0].branch, None);
        assert!(!reports[0].dirty);
    }

    #[tokio::test]
    async fn test_status_missing_project() {
        let tmp = tempdir().unwrap();
        let ctx = make_context(&tmp, vec![]);
        let engine = DefaultStatus;
        let err = engine
            .status(&ctx, vec![Utf8PathBuf::from("missing")], StatusOptions::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("project not found"));
    }

    #[tokio::test]
    async fn test_status_open_error() {
        let tmp = tempdir().unwrap();
        let mut ctx = make_context(&tmp, vec![make_project("foo", "foo")]);
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|_| Box::pin(async { Err(repo_rs_git::Error::Git2("open failed".to_string())) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultStatus;
        let err = engine
            .status(&ctx, vec![Utf8PathBuf::from("foo")], StatusOptions::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("open failed"));
    }

    #[tokio::test]
    async fn test_status_graceful_git_errors() {
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
        mock.expect_head_name()
            .times(1)
            .returning(|_| Box::pin(async { Err(repo_rs_git::Error::Git2("head failed".to_string())) }));
        mock.expect_is_dirty()
            .times(1)
            .returning(|_| Box::pin(async { Err(repo_rs_git::Error::Git2("dirty failed".to_string())) }));
        mock.expect_ahead_behind()
            .times(1)
            .returning(|_, _| Box::pin(async { Err(repo_rs_git::Error::Git2("ahead_behind failed".to_string())) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultStatus;
        let reports = engine
            .status(&ctx, vec![Utf8PathBuf::from("foo")], StatusOptions::default())
            .await
            .unwrap();
        assert_eq!(reports[0].branch, None);
        assert!(!reports[0].dirty);
        assert_eq!(reports[0].commits_ahead, 0);
        assert_eq!(reports[0].commits_behind, 0);
    }
}
