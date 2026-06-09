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
        let mut apply_list: Vec<Utf8PathBuf> = Vec::new();

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
                        apply_list.push(path);
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
                    apply_list.push(path);
                } else if let Err(e) = ctx.git.checkout(&repo, &target).await {
                    report.errors.push(format!(
                        "failed to checkout {}: {e}",
                        project.name
                    ));
                    continue;
                } else {
                    report.checked_out.push(project.name.clone());
                    apply_list.push(path);
                }
            }
        }

        // Apply copyfiles and linkfiles for all successfully synced projects
        // after all projects have been cloned/checked out. This ensures that
        // linkfiles or copyfiles targeting directories inside other projects'
        // worktrees don't create those directories early and block later clones.
        for path in apply_list {
            let project = match ctx.client.project_by_path(&path) {
                Some(p) => p,
                None => continue,
            };
            if let Err(e) = apply_copyfiles_and_linkfiles(ctx, project) {
                report.errors.push(format!(
                    "failed to apply copyfiles/linkfiles for {}: {e}",
                    project.name
                ));
            }
        }

        Ok(report)
    }
}

/// Apply copyfile and linkfile directives for a project.
///
/// * `src` paths are resolved relative to the project's worktree.
/// * `dest` paths are resolved relative to the repo root.
fn apply_copyfiles_and_linkfiles(ctx: &Context, project: &repo_rs_model::Project) -> Result<(), Error> {
    for cf in &project.copyfiles {
        let src = project.worktree.join(&cf.src);
        let dest = ctx.repo_root.join(&cf.dest);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&src, &dest)?;
    }

    for lf in &project.linkfiles {
        let src = project.worktree.join(&lf.src);
        let dest = ctx.repo_root.join(&lf.dest);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Remove existing file or symlink at dest before creating a new one.
        if dest.exists() || dest.symlink_metadata().is_ok() {
            if dest.is_dir() {
                std::fs::remove_dir_all(&dest)?;
            } else {
                std::fs::remove_file(&dest)?;
            }
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&src, &dest)?;
        }
        #[cfg(windows)]
        {
            if src.is_dir() {
                std::os::windows::fs::symlink_dir(&src, &dest)?;
            } else {
                std::os::windows::fs::symlink_file(&src, &dest)?;
            }
        }
    }

    Ok(())
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

    #[tokio::test]
    async fn test_sync_creates_copyfiles_after_clone() {
        let tmp = tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let mut project = make_project("foo", "foo", "https://example.com/foo.git");
        project.copyfiles = vec![repo_rs_model::project::CopyFile {
            src: Utf8PathBuf::from("src.txt"),
            dest: Utf8PathBuf::from("dest.txt"),
        }];
        let mut ctx = make_context(&tmp, vec![project.clone()]);

        // Create the worktree with a source file.
        let worktree = root.join("foo");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(worktree.join("src.txt"), "hello copy").unwrap();

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
        assert!(report.fetched.contains(&"foo".to_string()));
        assert!(report.checked_out.contains(&"foo".to_string()));
        assert!(report.errors.is_empty());

        let dest = root.join("dest.txt");
        assert!(dest.exists());
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "hello copy");
    }

    #[tokio::test]
    async fn test_sync_creates_linkfiles_after_clone() {
        let tmp = tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let mut project = make_project("foo", "foo", "https://example.com/foo.git");
        project.linkfiles = vec![repo_rs_model::project::LinkFile {
            src: Utf8PathBuf::from("src.txt"),
            dest: Utf8PathBuf::from("link.txt"),
        }];
        let mut ctx = make_context(&tmp, vec![project.clone()]);

        // Create the worktree with a source file.
        let worktree = root.join("foo");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(worktree.join("src.txt"), "hello link").unwrap();

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
        assert!(report.fetched.contains(&"foo".to_string()));
        assert!(report.checked_out.contains(&"foo".to_string()));
        assert!(report.errors.is_empty());

        let dest = root.join("link.txt");
        assert!(dest.symlink_metadata().is_ok());
        assert!(std::fs::symlink_metadata(&dest).unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "hello link");
    }

    #[tokio::test]
    async fn test_sync_linkfile_replaces_existing_file() {
        let tmp = tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let mut project = make_project("foo", "foo", "https://example.com/foo.git");
        project.linkfiles = vec![repo_rs_model::project::LinkFile {
            src: Utf8PathBuf::from("src.txt"),
            dest: Utf8PathBuf::from("link.txt"),
        }];
        let mut ctx = make_context(&tmp, vec![project.clone()]);

        // Create the worktree with a source file.
        let worktree = root.join("foo");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(worktree.join("src.txt"), "new content").unwrap();
        // Pre-create an existing regular file at the destination.
        std::fs::write(root.join("link.txt"), "old content").unwrap();

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
        assert!(report.errors.is_empty());

        let dest = root.join("link.txt");
        assert!(std::fs::symlink_metadata(&dest).unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "new content");
    }

    #[tokio::test]
    async fn test_sync_creates_nested_dest_directories() {
        let tmp = tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let mut project = make_project("foo", "foo", "https://example.com/foo.git");
        project.copyfiles = vec![repo_rs_model::project::CopyFile {
            src: Utf8PathBuf::from("a.txt"),
            dest: Utf8PathBuf::from("sub/dir/a.txt"),
        }];
        let mut ctx = make_context(&tmp, vec![project.clone()]);

        let worktree = root.join("foo");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(worktree.join("a.txt"), "nested").unwrap();

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
        assert!(report.errors.is_empty());

        let dest = root.join("sub/dir/a.txt");
        assert!(dest.exists());
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "nested");
    }

    #[tokio::test]
    async fn test_sync_linkfiles_after_checkout() {
        let tmp = tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let mut project = make_project("foo", "foo", "https://example.com/foo.git");
        project.linkfiles = vec![repo_rs_model::project::LinkFile {
            src: Utf8PathBuf::from("src.txt"),
            dest: Utf8PathBuf::from("link.txt"),
        }];
        let mut ctx = make_context(&tmp, vec![project.clone()]);

        let worktree = root.join("foo");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(worktree.join("src.txt"), "checkout link").unwrap();
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
        assert!(report.errors.is_empty());

        let dest = root.join("link.txt");
        assert!(std::fs::symlink_metadata(&dest).unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "checkout link");
    }

    #[tokio::test]
    async fn test_sync_defers_linkfiles_for_later_project() {
        let tmp = tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();

        let mut skills = make_project("skills", "skills", "https://example.com/skills.git");
        skills.linkfiles = vec![repo_rs_model::project::LinkFile {
            src: Utf8PathBuf::from("agents/skills"),
            dest: Utf8PathBuf::from("platform/sso/.agents/skills"),
        }];

        let sso = make_project("sso", "platform/sso", "https://example.com/sso.git");

        let mut ctx = make_context(&tmp, vec![skills.clone(), sso.clone()]);

        let mut mock = MockGitBackend::new();
        mock.expect_clone()
            .times(2)
            .returning(|_, dst, _, _, _| {
                let dst = dst.clone();
                Box::pin(async move {
                    // Simulate git clone failing on non-empty directories for the later project.
                    if dst.as_str().ends_with("platform/sso") && dst.exists() {
                        let entries: Vec<_> = std::fs::read_dir(&dst)
                            .map_err(|e| repo_rs_git::Error::Git2(e.to_string()))?
                            .filter_map(|e| e.ok())
                            .collect();
                        if !entries.is_empty() {
                            return Err(repo_rs_git::Error::Git2(format!(
                                "'{dst}' exists and is not an empty directory"
                            )));
                        }
                    }
                    std::fs::create_dir_all(&dst).unwrap();
                    // Create the source file inside the skills worktree so the linkfile has something to link.
                    if dst.file_name() == Some("skills") {
                        std::fs::create_dir_all(dst.join("agents")).unwrap();
                        std::fs::write(dst.join("agents/skills"), "skills content").unwrap();
                    }
                    Ok(git2::Repository::init(&dst).unwrap())
                })
            });
        ctx.git = Arc::new(mock);

        let engine = DefaultSync;
        let report = engine
            .sync(
                &ctx,
                vec![Utf8PathBuf::from("skills"), Utf8PathBuf::from("platform/sso")],
                SyncOptions::default(),
            )
            .await
            .unwrap();

        // Both projects should be cloned successfully.
        assert!(report.fetched.contains(&"skills".to_string()));
        assert!(report.fetched.contains(&"sso".to_string()));
        assert!(report.checked_out.contains(&"skills".to_string()));
        assert!(report.checked_out.contains(&"sso".to_string()));
        assert!(report.errors.is_empty());

        // Linkfile should have been created after both clones.
        let link_dest = root.join("platform/sso/.agents/skills");
        assert!(std::fs::symlink_metadata(&link_dest).unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&link_dest).unwrap(), "skills content");
    }

    #[tokio::test]
    async fn test_sync_defers_copyfiles_for_later_project() {
        let tmp = tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();

        let mut config = make_project("config", "config", "https://example.com/config.git");
        config.copyfiles = vec![repo_rs_model::project::CopyFile {
            src: Utf8PathBuf::from("settings.json"),
            dest: Utf8PathBuf::from("platform/sso/.agents/settings.json"),
        }];

        let sso = make_project("sso", "platform/sso", "https://example.com/sso.git");

        let mut ctx = make_context(&tmp, vec![config.clone(), sso.clone()]);

        let mut mock = MockGitBackend::new();
        mock.expect_clone()
            .times(2)
            .returning(|_, dst, _, _, _| {
                let dst = dst.clone();
                Box::pin(async move {
                    // Simulate git clone failing on non-empty directories for the later project.
                    if dst.as_str().ends_with("platform/sso") && dst.exists() {
                        let entries: Vec<_> = std::fs::read_dir(&dst)
                            .map_err(|e| repo_rs_git::Error::Git2(e.to_string()))?
                            .filter_map(|e| e.ok())
                            .collect();
                        if !entries.is_empty() {
                            return Err(repo_rs_git::Error::Git2(format!(
                                "'{dst}' exists and is not an empty directory"
                            )));
                        }
                    }
                    std::fs::create_dir_all(&dst).unwrap();
                    // Create the source file inside the config worktree so the copyfile has something to copy.
                    if dst.file_name() == Some("config") {
                        std::fs::write(dst.join("settings.json"), "settings content").unwrap();
                    }
                    Ok(git2::Repository::init(&dst).unwrap())
                })
            });
        ctx.git = Arc::new(mock);

        let engine = DefaultSync;
        let report = engine
            .sync(
                &ctx,
                vec![Utf8PathBuf::from("config"), Utf8PathBuf::from("platform/sso")],
                SyncOptions::default(),
            )
            .await
            .unwrap();

        // Both projects should be cloned successfully.
        assert!(report.fetched.contains(&"config".to_string()));
        assert!(report.fetched.contains(&"sso".to_string()));
        assert!(report.checked_out.contains(&"config".to_string()));
        assert!(report.checked_out.contains(&"sso".to_string()));
        assert!(report.errors.is_empty());

        // Copyfile should have been created after both clones.
        let copy_dest = root.join("platform/sso/.agents/settings.json");
        assert!(copy_dest.exists());
        assert_eq!(std::fs::read_to_string(&copy_dest).unwrap(), "settings content");
    }

    #[tokio::test]
    async fn test_sync_rebase_applies_copyfiles_and_linkfiles() {
        let tmp = tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let mut project = make_project("foo", "foo", "https://example.com/foo.git");
        project.copyfiles = vec![repo_rs_model::project::CopyFile {
            src: Utf8PathBuf::from("src.txt"),
            dest: Utf8PathBuf::from("dest.txt"),
        }];
        project.linkfiles = vec![repo_rs_model::project::LinkFile {
            src: Utf8PathBuf::from("link_src.txt"),
            dest: Utf8PathBuf::from("link.txt"),
        }];
        let mut ctx = make_context(&tmp, vec![project]);
        let worktree = ctx
            .client
            .project_by_path(&Utf8PathBuf::from("foo"))
            .unwrap()
            .worktree
            .clone();
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(worktree.join("src.txt"), "copy content").unwrap();
        std::fs::write(worktree.join("link_src.txt"), "link content").unwrap();
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

        let copy_dest = root.join("dest.txt");
        assert!(copy_dest.exists());
        assert_eq!(std::fs::read_to_string(&copy_dest).unwrap(), "copy content");

        let link_dest = root.join("link.txt");
        assert!(std::fs::symlink_metadata(&link_dest).unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&link_dest).unwrap(), "link content");
    }
}
