// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use crate::{Context, Error};

/// Options controlling sync behavior.
#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
    /// Jobs network.
    pub jobs_network: Option<usize>,
    /// Jobs checkout.
    pub jobs_checkout: Option<usize>,
    /// Local only.
    pub local_only: bool,
    /// Force sync.
    pub force_sync: bool,
    /// Rebase.
    pub rebase: bool,
    /// Current branch only.
    pub current_branch_only: bool,
    /// Detach.
    pub detach: bool,
    /// Prune.
    pub prune: bool,
    /// Tags.
    pub tags: bool,
    /// Depth.
    pub depth: Option<u32>,
    /// Clone bundle.
    pub clone_bundle: bool,
    /// Optimized fetch.
    pub optimized_fetch: bool,
    /// Retry fetches.
    pub retry_fetches: u32,
}

/// Result of a sync operation.
#[derive(Debug, Clone, Default)]
pub struct SyncReport {
    /// Fetched.
    pub fetched: Vec<String>,
    /// Checked out.
    pub checked_out: Vec<String>,
    /// Errors.
    pub errors: Vec<String>,
}

/// Trait for sync logic.
#[async_trait::async_trait]
pub trait Sync {
    /// Synchronize the given projects.
    ///
    /// # Errors
    ///
    /// Returns an error if any project fails to sync and `--fail-fast`
    /// semantics apply.
    async fn sync(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        opts: SyncOptions,
    ) -> Result<SyncReport, Error>;
}

/// Default sync implementation.
pub struct DefaultSync;

#[async_trait::async_trait]
impl Sync for DefaultSync {
    async fn sync(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        opts: SyncOptions,
    ) -> Result<SyncReport, Error> {
        let mut report = SyncReport::default();

        for path in projects {
            let project = match ctx.client.project_by_path(&path) {
                Some(p) => p,
                None => {
                    report.errors.push(format!("project not found: {path}"));
                    continue;
                }
            };

            let worktree = &project.worktree;
            let gitdir = worktree.join(".git");

            let exists = gitdir.exists();

            if !exists {
                let url = match url::Url::parse(&project.remote.fetch_url) {
                    Ok(u) => u,
                    Err(_) => {
                        report
                            .errors
                            .push(format!("invalid url for {}", project.name));
                        continue;
                    }
                };
                match repo_rs_git::GitBackend::clone(
                    &*ctx.git,
                    &url,
                    worktree,
                    project.clone_depth.or(opts.depth),
                    None,
                    ctx.progress.as_ref(),
                )
                .await
                {
                    Ok(_) => {
                        report.fetched.push(project.name.clone());
                        report.checked_out.push(project.name.clone());
                    }
                    Err(e) => {
                        report.errors.push(format!(
                            "failed to clone {}: {e}",
                            project.name
                        ));
                        continue;
                    }
                }
            } else {
                let repo = match ctx.git.open(worktree).await {
                    Ok(r) => r,
                    Err(e) => {
                        report.errors.push(format!(
                            "failed to open {}: {e}",
                            project.name
                        ));
                        continue;
                    }
                };

                if !opts.local_only {
                    match ctx
                        .git
                        .fetch(
                            &repo,
                            &project.remote.name,
                            opts.prune,
                            opts.tags,
                            project.clone_depth.or(opts.depth),
                            ctx.progress.as_ref(),
                        )
                        .await
                    {
                        Ok(_) => {
                            report.fetched.push(project.name.clone());
                        }
                        Err(e) => {
                            report.errors.push(format!(
                                "failed to fetch {}: {e}",
                                project.name
                            ));
                            continue;
                        }
                    }
                }

                let target = project.revision_expr.clone();
                if opts.rebase {
                    let upstream = project
                        .upstream
                        .clone()
                        .unwrap_or_else(|| target.clone());
                    if let Err(e) = ctx.git.rebase(&repo, &upstream).await {
                        report.errors.push(format!(
                            "failed to rebase {}: {e}",
                            project.name
                        ));
                        continue;
                    }
                    report.checked_out.push(project.name.clone());
                } else if let Err(e) = ctx.git.checkout(&repo, &target).await {
                    report.errors.push(format!(
                        "failed to checkout {}: {e}",
                        project.name
                    ));
                    continue;
                } else {
                    report.checked_out.push(project.name.clone());
                }
            }
        }

        Ok(report)
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

    fn make_project(name: &str, relpath: &str, fetch_url: &str) -> Project {
        Project {
            name: name.to_string(),
            relpath: Utf8PathBuf::from(relpath),
            worktree: Utf8PathBuf::from(format!("/tmp/repo/{relpath}")),
            gitdir: Utf8PathBuf::from(format!("/tmp/repo/.repo/projects/{relpath}.git")),
            objdir: Utf8PathBuf::from(format!("/tmp/repo/.repo/project-objects/{name}.git")),
            remote: repo_rs_model::RemoteSpec {
                name: "origin".to_string(),
                fetch_url: fetch_url.to_string(),
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
    async fn test_sync_empty_projects() {
        let tmp = tempdir().unwrap();
        let ctx = make_context(&tmp, vec![]);
        let engine = DefaultSync;
        let report = engine.sync(&ctx, vec![], SyncOptions::default()).await.unwrap();
        assert!(report.fetched.is_empty());
        assert!(report.checked_out.is_empty());
        assert!(report.errors.is_empty());
    }

    #[tokio::test]
    async fn test_sync_clone_new_project() {
        let tmp = tempdir().unwrap();
        let mut ctx = make_context(&tmp, vec![make_project("foo", "foo", "https://example.com/foo.git")]);
        let mut mock = MockGitBackend::new();
        mock.expect_clone()
            .times(1)
            .returning(|_, _, _, _, _| {
                Box::pin(async {
                    let dir = tempdir().unwrap();
                    let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
                    Ok(git2::Repository::init(&path).unwrap())
                })
            });
        ctx.git = Arc::new(mock);

        let engine = DefaultSync;
        let report = engine
            .sync(&ctx, vec![Utf8PathBuf::from("foo")], SyncOptions::default())
            .await
            .unwrap();
        assert_eq!(report.fetched, vec!["foo"]);
        assert_eq!(report.checked_out, vec!["foo"]);
        assert!(report.errors.is_empty());
    }

    #[tokio::test]
    async fn test_sync_missing_project() {
        let tmp = tempdir().unwrap();
        let ctx = make_context(&tmp, vec![]);
        let engine = DefaultSync;
        let report = engine
            .sync(&ctx, vec![Utf8PathBuf::from("missing")], SyncOptions::default())
            .await
            .unwrap();
        assert!(report.fetched.is_empty());
        assert!(report.checked_out.is_empty());
        assert_eq!(report.errors, vec!["project not found: missing"]);
    }

    #[tokio::test]
    async fn test_sync_fetch_and_checkout_existing() {
        let tmp = tempdir().unwrap();
        let project = make_project("foo", "foo", "https://example.com/foo.git");
        let mut ctx = make_context(&tmp, vec![project]);
        let worktree = ctx.client.project_by_path(&Utf8PathBuf::from("foo")).unwrap().worktree.clone();
        std::fs::create_dir_all(&worktree).unwrap();
        git2::Repository::init(&worktree).unwrap();
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|path| {
                let path = path.clone();
                Box::pin(async move { Ok(git2::Repository::open(&path).unwrap()) })
            });
        mock.expect_fetch()
            .times(1)
            .returning(|_, _, _, _, _, _| Box::pin(async { Ok(()) }));
        mock.expect_checkout()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultSync;
        let report = engine
            .sync(&ctx, vec![Utf8PathBuf::from("foo")], SyncOptions::default())
            .await
            .unwrap();
        assert_eq!(report.fetched, vec!["foo"]);
        assert_eq!(report.checked_out, vec!["foo"]);
        assert!(report.errors.is_empty());
    }

    #[tokio::test]
    async fn test_sync_local_only_skips_fetch() {
        let tmp = tempdir().unwrap();
        let project = make_project("foo", "foo", "https://example.com/foo.git");
        let mut ctx = make_context(&tmp, vec![project]);
        let worktree = ctx.client.project_by_path(&Utf8PathBuf::from("foo")).unwrap().worktree.clone();
        std::fs::create_dir_all(&worktree).unwrap();
        git2::Repository::init(&worktree).unwrap();
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|path| {
                let path = path.clone();
                Box::pin(async move { Ok(git2::Repository::open(&path).unwrap()) })
            });
        mock.expect_fetch().times(0);
        mock.expect_checkout()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultSync;
        let report = engine
            .sync(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                SyncOptions {
                    local_only: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(report.fetched.is_empty());
        assert_eq!(report.checked_out, vec!["foo"]);
        assert!(report.errors.is_empty());
    }

    #[tokio::test]
    async fn test_sync_rebase() {
        let tmp = tempdir().unwrap();
        let project = make_project("foo", "foo", "https://example.com/foo.git");
        let mut ctx = make_context(&tmp, vec![project]);
        let worktree = ctx.client.project_by_path(&Utf8PathBuf::from("foo")).unwrap().worktree.clone();
        std::fs::create_dir_all(&worktree).unwrap();
        git2::Repository::init(&worktree).unwrap();
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|path| {
                let path = path.clone();
                Box::pin(async move { Ok(git2::Repository::open(&path).unwrap()) })
            });
        mock.expect_fetch()
            .times(1)
            .returning(|_, _, _, _, _, _| Box::pin(async { Ok(()) }));
        mock.expect_rebase()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultSync;
        let report = engine
            .sync(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                SyncOptions {
                    rebase: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(report.fetched, vec!["foo"]);
        assert_eq!(report.checked_out, vec!["foo"]);
        assert!(report.errors.is_empty());
    }

    #[tokio::test]
    async fn test_sync_clone_error() {
        let tmp = tempdir().unwrap();
        let mut ctx = make_context(&tmp, vec![make_project("foo", "foo", "https://example.com")]);
        let mut mock = MockGitBackend::new();
        mock.expect_clone()
            .times(1)
            .returning(|_, _, _, _, _| Box::pin(async { Err(repo_rs_git::Error::Git2("clone failed".to_string())) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultSync;
        let report = engine
            .sync(&ctx, vec![Utf8PathBuf::from("foo")], SyncOptions::default())
            .await
            .unwrap();
        assert!(report.fetched.is_empty());
        assert!(report.checked_out.is_empty());
        assert!(report.errors[0].contains("failed to clone foo"));
    }

    #[tokio::test]
    async fn test_sync_invalid_url() {
        let tmp = tempdir().unwrap();
        let mut ctx = make_context(&tmp, vec![make_project("foo", "foo", "::invalid::")]);
        let mock = MockGitBackend::new();
        ctx.git = Arc::new(mock);

        let engine = DefaultSync;
        let report = engine
            .sync(&ctx, vec![Utf8PathBuf::from("foo")], SyncOptions::default())
            .await
            .unwrap();
        assert!(report.fetched.is_empty());
        assert!(report.checked_out.is_empty());
        assert!(report.errors[0].contains("invalid url"));
    }
}
