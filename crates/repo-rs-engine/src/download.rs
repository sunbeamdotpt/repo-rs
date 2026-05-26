// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use crate::{Context, Error};

/// Options controlling download behavior.
#[derive(Debug, Clone, Default)]
pub struct DownloadOptions {
    /// The name of the currently checked-out branch.
    pub current_branch: bool,
    /// Tags.
    pub tags: bool,
    /// The branch name.
    pub branch: Option<String>,
    /// Cherry pick.
    pub cherry_pick: bool,
    /// Record origin.
    pub record_origin: bool,
    /// Revert.
    pub revert: bool,
    /// Ff only.
    pub ff_only: bool,
    /// Changes.
    pub changes: Vec<String>,
    /// Current working directory, used to infer the default project.
    pub cwd: Option<Utf8PathBuf>,
}

/// Trait for download logic.
#[async_trait::async_trait]
pub trait Download {
    /// Download a change from the code review server.
    async fn download(&self, ctx: &Context, opts: DownloadOptions) -> Result<(), Error>;
}

/// Default download implementation.
pub struct DefaultDownload;

#[async_trait::async_trait]
impl Download for DefaultDownload {
    /// Download a change from the code review server.
    async fn download(&self, ctx: &Context, opts: DownloadOptions) -> Result<(), Error> {
        if opts.changes.is_empty() {
            return Err(Error::InvalidArguments(
                "no changes specified".to_string(),
            ));
        }

        let mut current_project: Option<repo_rs_model::Project> = None;

        for arg in &opts.changes {
            if let Some((chg_id, ps_id)) = parse_change_id(arg) {
                let project = match &current_project {
                    Some(p) => p.clone(),
                    None => {
                        find_project_by_cwd(ctx, opts.cwd.as_ref())
                            .ok_or_else(|| {
                                Error::InvalidArguments(
                                    "no project specified and cannot determine project from cwd"
                                        .to_string(),
                                )
                            })?
                    }
                };

                let patchset = match ps_id {
                    Some(id) => id,
                    None => {
                        find_latest_patchset(ctx, &project, chg_id).await?
                    }
                };

                let repo = ctx
                    .git
                    .open(&project.worktree)
                    .await
                    .map_err(|e| Error::Sync(format!("failed to open {}: {e}", project.name)))?;

                let remote_ref = format!(
                    "refs/changes/{:02}/{}/{}",
                    chg_id % 100,
                    chg_id,
                    patchset
                );
                let local_ref = format!("refs/repo-rs/download/{}/{}", chg_id, patchset);
                let refspec = format!("+{}:{}", remote_ref, local_ref);

                ctx.git
                    .fetch_refspec(&repo, "origin", &refspec, &())
                    .await
                    .map_err(|e| {
                        Error::Sync(format!(
                            "failed to fetch change {} for {}: {e}",
                            chg_id, project.name
                        ))
                    })?;

                let commit = ctx
                    .git
                    .ref_resolve(&repo, &local_ref)
                    .await
                    .map_err(|e| Error::Sync(format!("failed to resolve downloaded ref: {e}")))?;

                if opts.cherry_pick {
                    ctx.git
                        .cherry_pick(&repo, &commit)
                        .await
                        .map_err(|e| Error::Sync(format!("failed to cherry-pick {}: {e}", project.name)))?;
                } else if opts.revert {
                    ctx.git
                        .checkout(&repo, &local_ref)
                        .await
                        .map_err(|e| Error::Sync(format!("failed to checkout {}: {e}", project.name)))?;
                } else if opts.ff_only {
                    ctx.git
                        .ref_update(&repo, "HEAD", &commit)
                        .await
                        .map_err(|e| Error::Sync(format!("failed to fast-forward {}: {e}", project.name)))?;
                } else {
                    ctx.git
                        .checkout(&repo, &local_ref)
                        .await
                        .map_err(|e| Error::Sync(format!("failed to checkout {}: {e}", project.name)))?;
                }
            } else {
                // Treat as a project name or path.
                let path = Utf8PathBuf::from(arg);
                current_project = ctx
                    .client
                    .project_by_path(&path)
                    .cloned()
                    .or_else(|| {
                        ctx.client.projects.values().find(|p| p.name == *arg).cloned()
                    });
            }
        }

        Ok(())
    }
}

/// Parse a change-id argument.
///
/// Accepts `12345` or `12345/3` (change-id / patchset).
fn parse_change_id(arg: &str) -> Option<(u32, Option<u32>)> {
    let parts: Vec<&str> = arg.split('/').collect();
    let chg_id: u32 = parts.first()?.parse().ok()?;
    if chg_id == 0 {
        return None;
    }
    let ps_id = parts.get(1).and_then(|s| s.parse().ok());
    Some((chg_id, ps_id))
}

/// Find the project that contains the current working directory.
fn find_project_by_cwd(ctx: &Context, cwd: Option<&Utf8PathBuf>) -> Option<repo_rs_model::Project> {
    let cwd = cwd?;
    ctx.client
        .projects
        .values()
        .find(|p| cwd.as_str().starts_with(p.worktree.as_str()))
        .cloned()
}

/// Find the latest patchset for a change by listing remote refs.
async fn find_latest_patchset(
    ctx: &Context,
    project: &repo_rs_model::Project,
    chg_id: u32,
) -> Result<u32, Error> {
    let repo = ctx
        .git
        .open(&project.worktree)
        .await
        .map_err(|e| Error::Sync(format!("failed to open {}: {e}", project.name)))?;

    let pattern = format!("refs/changes/{:02}/{}/", chg_id % 100, chg_id);
    let refs = ctx
        .git
        .list_remote_refs(&repo, "origin", &pattern)
        .await
        .map_err(|e| Error::Sync(format!("failed to list remote refs: {e}")))?;

    let max_ps = refs
        .iter()
        .filter_map(|(name, _)| {
            name.rsplit('/').next()?.parse::<u32>().ok()
        })
        .max()
        .unwrap_or(1);

    Ok(max_ps)
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
    async fn test_download_no_changes() {
        let ctx = make_context();
        let engine = DefaultDownload;
        let err = engine
            .download(&ctx, DownloadOptions::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("no changes specified"));
    }

    #[tokio::test]
    async fn test_download_change_id_without_project() {
        let ctx = make_context();
        let engine = DefaultDownload;
        let err = engine
            .download(
                &ctx,
                DownloadOptions {
                    changes: vec!["12345".to_string()],
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("no project specified"));
    }

    #[tokio::test]
    async fn test_download_change_id_with_explicit_project() {
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
        mock.expect_list_remote_refs()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(vec![]) }));
        mock.expect_fetch_refspec()
            .times(1)
            .withf(|_, _, refspec, _| refspec.contains("refs/changes/"))
            .returning(|_, _, _, _| Box::pin(async { Ok(()) }));
        mock.expect_ref_resolve()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("abc123".to_string()) }));
        mock.expect_checkout()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultDownload;
        engine
            .download(
                &ctx,
                DownloadOptions {
                    changes: vec!["foo".to_string(), "12345".to_string()],
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_download_with_patchset() {
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
        mock.expect_fetch_refspec()
            .times(1)
            .withf(|_, _, refspec, _| refspec.contains("refs/changes/45/12345/2"))
            .returning(|_, _, _, _| Box::pin(async { Ok(()) }));
        mock.expect_ref_resolve()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("abc123".to_string()) }));
        mock.expect_checkout()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultDownload;
        engine
            .download(
                &ctx,
                DownloadOptions {
                    changes: vec!["foo".to_string(), "12345/2".to_string()],
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_download_cherry_pick() {
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
        mock.expect_list_remote_refs()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(vec![]) }));
        mock.expect_fetch_refspec()
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(()) }));
        mock.expect_ref_resolve()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("abc123".to_string()) }));
        mock.expect_cherry_pick()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultDownload;
        engine
            .download(
                &ctx,
                DownloadOptions {
                    changes: vec!["foo".to_string(), "12345".to_string()],
                    cherry_pick: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_download_fetch_error() {
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
        mock.expect_list_remote_refs()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(vec![]) }));
        mock.expect_fetch_refspec()
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Err(repo_rs_git::Error::Git2("fetch failed".to_string())) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultDownload;
        let err = engine
            .download(
                &ctx,
                DownloadOptions {
                    changes: vec!["foo".to_string(), "12345".to_string()],
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("failed to fetch"));
    }

    #[tokio::test]
    async fn test_download_ff_only() {
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
        mock.expect_list_remote_refs()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(vec![]) }));
        mock.expect_fetch_refspec()
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(()) }));
        mock.expect_ref_resolve()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("abc123".to_string()) }));
        mock.expect_ref_update()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultDownload;
        engine
            .download(
                &ctx,
                DownloadOptions {
                    changes: vec!["foo".to_string(), "12345".to_string()],
                    ff_only: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_download_revert() {
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
        mock.expect_list_remote_refs()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(vec![]) }));
        mock.expect_fetch_refspec()
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(()) }));
        mock.expect_ref_resolve()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("abc123".to_string()) }));
        mock.expect_checkout()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultDownload;
        engine
            .download(
                &ctx,
                DownloadOptions {
                    changes: vec!["foo".to_string(), "12345".to_string()],
                    revert: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_download_with_cwd() {
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
        mock.expect_list_remote_refs()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(vec![]) }));
        mock.expect_fetch_refspec()
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(()) }));
        mock.expect_ref_resolve()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("abc123".to_string()) }));
        mock.expect_checkout()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultDownload;
        engine
            .download(
                &ctx,
                DownloadOptions {
                    changes: vec!["12345".to_string()],
                    cwd: Some(Utf8PathBuf::from("/tmp/foo/src")),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_download_latest_patchset() {
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
        mock.expect_list_remote_refs()
            .times(1)
            .returning(|_, _, _| {
                Box::pin(async {
                    Ok(vec![
                        ("refs/changes/45/12345/1".to_string(), "a".to_string()),
                        ("refs/changes/45/12345/3".to_string(), "c".to_string()),
                        ("refs/changes/45/12345/2".to_string(), "b".to_string()),
                    ])
                })
            });
        mock.expect_fetch_refspec()
            .times(1)
            .withf(|_, _, refspec, _| refspec.contains("refs/changes/45/12345/3"))
            .returning(|_, _, _, _| Box::pin(async { Ok(()) }));
        mock.expect_ref_resolve()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("abc123".to_string()) }));
        mock.expect_checkout()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultDownload;
        engine
            .download(
                &ctx,
                DownloadOptions {
                    changes: vec!["foo".to_string(), "12345".to_string()],
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    #[test]
    fn test_parse_change_id_plain() {
        assert_eq!(parse_change_id("12345"), Some((12345, None)));
    }

    #[test]
    fn test_parse_change_id_with_patchset() {
        assert_eq!(parse_change_id("12345/3"), Some((12345, Some(3))));
    }

    #[test]
    fn test_parse_change_id_invalid() {
        assert_eq!(parse_change_id("abc"), None);
        assert_eq!(parse_change_id("0"), None);
    }

    #[test]
    fn test_find_project_by_cwd() {
        let ctx = make_context();
        let cwd = Utf8PathBuf::from("/tmp/foo/src");
        let project = find_project_by_cwd(&ctx, Some(&cwd));
        assert!(project.is_some());
        assert_eq!(project.unwrap().name, "foo");
    }

    #[test]
    fn test_find_project_by_cwd_no_match() {
        let ctx = make_context();
        let cwd = Utf8PathBuf::from("/other/path");
        let project = find_project_by_cwd(&ctx, Some(&cwd));
        assert!(project.is_none());
    }
}
