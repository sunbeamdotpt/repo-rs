// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use crate::{Context, Error};

/// Trait for checkout logic.
#[async_trait::async_trait]
pub trait Checkout {
    /// Checkout a branch in the given projects.
    async fn checkout(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        branch: String,
    ) -> Result<(), Error>;
}

/// Default checkout implementation.
pub struct DefaultCheckout;

#[async_trait::async_trait]
impl Checkout for DefaultCheckout {
    async fn checkout(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        branch: String,
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
            ctx.git.checkout(&repo, &branch).await.map_err(|e| {
                Error::Sync(format!(
                    "failed to checkout {branch} in {}: {e}",
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
    async fn test_checkout_happy_path() {
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
        ctx.git = Arc::new(mock);

        let engine = DefaultCheckout;
        engine
            .checkout(&ctx, vec![Utf8PathBuf::from("foo")], "main".to_string())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_checkout_missing_project() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp);
        let engine = DefaultCheckout;
        let err = engine
            .checkout(&ctx, vec![Utf8PathBuf::from("missing")], "main".to_string())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("project not found"));
    }

    #[tokio::test]
    async fn test_checkout_open_error() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = make_context(&tmp);
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|_| Box::pin(async { Err(repo_rs_git::Error::Git2("open failed".to_string())) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultCheckout;
        let err = engine
            .checkout(&ctx, vec![Utf8PathBuf::from("foo")], "main".to_string())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("open failed"));
    }
}
