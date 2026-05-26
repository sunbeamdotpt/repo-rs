// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use crate::{Context, Error};

/// Trait for branches logic.
#[async_trait::async_trait]
pub trait Branches {
    /// List topic branches in the given projects.
    async fn branches(&self, ctx: &Context, projects: Vec<Utf8PathBuf>) -> Result<String, Error>;
}

/// Default branches implementation.
pub struct DefaultBranches;

#[async_trait::async_trait]
impl Branches for DefaultBranches {
    /// List topic branches in the given projects.
    async fn branches(&self, ctx: &Context, projects: Vec<Utf8PathBuf>) -> Result<String, Error> {
        let mut lines = Vec::new();
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
                    lines.push(format!("{}: (not checked out: {e})", project.name));
                    continue;
                }
            };
            let refs = ctx
                .git
                .ref_list(&repo, Some("refs/heads/"))
                .await
                .unwrap_or_default();
            let branch_names: Vec<String> = refs
                .into_iter()
                .map(|(name, _)| {
                    name.strip_prefix("refs/heads/")
                        .unwrap_or(&name)
                        .to_string()
                })
                .collect();
            lines.push(format!("{}: {}", project.name, branch_names.join(", ")));
        }
        Ok(lines.join("\n"))
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
    async fn test_branches_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp, vec![]);
        let engine = DefaultBranches;
        let text = engine.branches(&ctx, vec![]).await.unwrap();
        assert!(text.is_empty());
    }

    #[tokio::test]
    async fn test_branches_happy_path() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = make_context(&tmp, vec![make_project("foo", "foo")]);
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
                        ("refs/heads/main".to_string(), "abc123".to_string()),
                        ("refs/heads/feature".to_string(), "def456".to_string()),
                    ])
                })
            });
        ctx.git = Arc::new(mock);

        let engine = DefaultBranches;
        let text = engine
            .branches(&ctx, vec![Utf8PathBuf::from("foo")])
            .await
            .unwrap();
        assert!(text.contains("foo:"));
        assert!(text.contains("main"));
        assert!(text.contains("feature"));
    }

    #[tokio::test]
    async fn test_branches_missing_project() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp, vec![]);
        let engine = DefaultBranches;
        let err = engine
            .branches(&ctx, vec![Utf8PathBuf::from("missing")])
            .await
            .unwrap_err();
        assert!(err.to_string().contains("project not found"));
    }

    #[tokio::test]
    async fn test_branches_open_error() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = make_context(&tmp, vec![make_project("foo", "foo")]);
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|_| Box::pin(async { Err(repo_rs_git::Error::Git2("open failed".to_string())) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultBranches;
        let text = engine
            .branches(&ctx, vec![Utf8PathBuf::from("foo")])
            .await
            .unwrap();
        assert!(text.contains("foo:"));
        assert!(text.contains("not checked out"));
    }
}
