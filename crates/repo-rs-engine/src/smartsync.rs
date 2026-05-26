// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use crate::{sync::Sync as _, Context, Error};

/// Trait for smartsync logic.
#[async_trait::async_trait]
pub trait Smartsync {
    /// Smart sync: update only projects with new commits.

    async fn smartsync(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
    ) -> Result<super::sync::SyncReport, Error>;
}

/// Default smartsync implementation.
///
/// Delegates to the standard sync engine with smart-sync flags enabled.
/// Smart manifest resolution (build-server integration) is not yet
/// implemented, so this currently performs a normal sync.
pub struct DefaultSmartsync;

#[async_trait::async_trait]
impl Smartsync for DefaultSmartsync {
    async fn smartsync(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
    ) -> Result<super::sync::SyncReport, Error> {
        let opts = super::sync::SyncOptions {
            // Smart sync would query a manifest server for the latest
            // known-good manifest. Until that infrastructure is wired in,
            // we fall back to a normal sync so the command is functional.
            ..Default::default()
        };
        let engine = super::sync::DefaultSync;
        engine.sync(ctx, projects, opts).await
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
    async fn test_smartsync_empty_projects() {
        let ctx = make_context();
        let engine = DefaultSmartsync;
        let report = engine.smartsync(&ctx, vec![]).await.unwrap();
        assert!(report.fetched.is_empty());
        assert!(report.checked_out.is_empty());
    }

    #[tokio::test]
    async fn test_smartsync_delegates_to_sync() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = make_context();
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

        let engine = DefaultSmartsync;
        let report = engine
            .smartsync(&ctx, vec![Utf8PathBuf::from("foo")])
            .await
            .unwrap();
        assert!(report.errors.is_empty());
    }
}
