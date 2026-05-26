// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use crate::{Context, Error};

/// Options controlling wipe behavior.
#[derive(Debug, Clone, Default)]
pub struct WipeOptions {
    /// Force.
    pub force: bool,
    /// Force uncommitted.
    pub force_uncommitted: bool,
    /// Force shared.
    pub force_shared: bool,
}

/// Trait for wipe logic.
#[async_trait::async_trait]
pub trait Wipe {
    /// Wipe and re-sync the given projects.

    async fn wipe(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        opts: WipeOptions,
    ) -> Result<(), Error>;
}

/// Default wipe implementation.
pub struct DefaultWipe;

#[async_trait::async_trait]
impl Wipe for DefaultWipe {
    async fn wipe(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        opts: WipeOptions,
    ) -> Result<(), Error> {
        if !opts.force {
            return Err(Error::InvalidArguments(
                "wipe requires --force".to_string(),
            ));
        }
        for path in projects {
            let project = match ctx.client.project_by_path(&path) {
                Some(p) => p,
                None => {
                    return Err(Error::InvalidArguments(format!(
                        "project not found: {path}"
                    )));
                }
            };
            if project.worktree.exists() {
                tokio::fs::remove_dir_all(&project.worktree).await?;
            }
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
    async fn test_wipe_requires_force() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp);
        let engine = DefaultWipe;
        let err = engine
            .wipe(&ctx, vec![Utf8PathBuf::from("foo")], WipeOptions::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("requires --force"));
    }

    #[tokio::test]
    async fn test_wipe_with_force() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = make_context(&tmp);
        let worktree = tmp.path().join("foo");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(worktree.join("file.txt"), "data").unwrap();
        ctx.client.projects.get_mut(&Utf8PathBuf::from("foo")).unwrap().worktree =
            Utf8PathBuf::from_path_buf(worktree.clone()).unwrap();

        let engine = DefaultWipe;
        engine
            .wipe(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                WipeOptions { force: true, ..Default::default() },
            )
            .await
            .unwrap();
        assert!(!worktree.exists());
    }

    #[tokio::test]
    async fn test_wipe_missing_project() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp);
        let engine = DefaultWipe;
        let err = engine
            .wipe(
                &ctx,
                vec![Utf8PathBuf::from("missing")],
                WipeOptions { force: true, ..Default::default() },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("project not found"));
    }
}
