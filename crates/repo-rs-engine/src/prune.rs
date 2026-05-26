// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use crate::{Context, Error};

/// Trait for prune logic.
#[async_trait::async_trait]
pub trait Prune {
    /// Prune merged topic branches.
    async fn prune(&self, ctx: &Context, projects: Vec<Utf8PathBuf>) -> Result<(), Error>;
}

/// Default prune implementation.
pub struct DefaultPrune;

#[async_trait::async_trait]
impl Prune for DefaultPrune {
    /// Prune merged topic branches.
    async fn prune(&self, ctx: &Context, projects: Vec<Utf8PathBuf>) -> Result<(), Error> {
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
            let refs = ctx
                .git
                .ref_list(&repo, Some("refs/heads/"))
                .await
                .unwrap_or_default();
            let _upstream = project
                .upstream
                .clone()
                .unwrap_or_else(|| project.revision_expr.clone());
            for (refname, _) in refs {
                if refname == format!("refs/heads/{}", project.revision_expr) {
                    continue;
                }
                let branch_name = refname
                    .strip_prefix("refs/heads/")
                    .unwrap_or(&refname)
                    .to_string();
                let (_, behind) = ctx
                    .git
                    .ahead_behind(&repo, &format!("refs/heads/{branch_name}"))
                    .await
                    .unwrap_or((0, 0));
                if behind > 0 {
                    ctx.git
                        .ref_update(&repo, &refname, "")
                        .await
                        .ok();
                }
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
    async fn test_prune_happy_path() {
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
        mock.expect_ref_list()
            .times(1)
            .returning(|_, _| {
                Box::pin(async {
                    Ok(vec![
                        ("refs/heads/main".to_string(), "abc".to_string()),
                        ("refs/heads/old".to_string(), "def".to_string()),
                    ])
                })
            });
        mock.expect_ahead_behind()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok((0, 1)) }));
        mock.expect_ref_update()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultPrune;
        engine.prune(&ctx, vec![Utf8PathBuf::from("foo")]).await.unwrap();
    }

    #[tokio::test]
    async fn test_prune_missing_project() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp);
        let engine = DefaultPrune;
        let err = engine
            .prune(&ctx, vec![Utf8PathBuf::from("missing")])
            .await
            .unwrap_err();
        assert!(err.to_string().contains("project not found"));
    }
}
