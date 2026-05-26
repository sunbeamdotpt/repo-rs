// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use crate::{Context, Error};

/// Options controlling info behavior.
#[derive(Debug, Clone, Default)]
pub struct InfoOptions {
    /// Json.
    pub json: bool,
    /// Diff.
    pub diff: bool,
    /// Overview.
    pub overview: bool,
    /// The name of the currently checked-out branch.
    pub current_branch: bool,
    /// Local only.
    pub local_only: bool,
    /// Format.
    pub format: InfoFormat,
    /// Include summary.
    pub include_summary: bool,
    /// Include projects.
    pub include_projects: bool,
}

/// Output format for info.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum InfoFormat {
    #[default]
    /// Plain text output.
    Text,
    /// Json variant.
    Json,
}

/// Trait for info logic.
#[async_trait::async_trait]
pub trait Info {
    /// Display information about projects.
    async fn info(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        opts: InfoOptions,
    ) -> Result<String, Error>;
}

/// Default info implementation.
pub struct DefaultInfo;

#[async_trait::async_trait]
impl Info for DefaultInfo {
    async fn info(
        &self,
        ctx: &Context,
        _projects: Vec<Utf8PathBuf>,
        _opts: InfoOptions,
    ) -> Result<String, Error> {
        let total = ctx.client.projects.len();
        let manifest_path = ctx.client.manifest_project.path.clone();
        let manifest_gitdir = ctx.client.manifest_project.gitdir.clone();

        let mut lines = Vec::new();
        lines.push(format!("Repo root: {}", ctx.repo_root));
        lines.push(format!("Manifest path: {manifest_path}"));
        lines.push(format!("Manifest gitdir: {manifest_gitdir}"));
        lines.push(format!("Total projects: {total}"));

        let manifest_repo = ctx.git.open(&manifest_gitdir).await.ok();
        if let Some(repo) = manifest_repo {
            if let Ok(Some(branch)) = ctx.git.head_name(&repo).await {
                lines.push(format!("Manifest branch: {branch}"));
            }
            if let Ok(refs) = ctx.git.ref_list(&repo, Some("HEAD")).await {
                if let Some((_, sha)) = refs.first() {
                    lines.push(format!("Manifest HEAD: {sha}"));
                }
            }
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
    async fn test_info_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = make_context(&tmp);
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .returning(|_| {
                Box::pin(async {
                    let dir = tempfile::tempdir().unwrap();
                    let p = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
                    Ok(git2::Repository::init(&p).unwrap())
                })
            });
        mock.expect_head_name()
            .returning(|_| Box::pin(async { Ok(Some("main".to_string())) }));
        mock.expect_ref_list()
            .returning(|_, _| Box::pin(async { Ok(vec![("HEAD".to_string(), "abc123".to_string())]) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultInfo;
        let text = engine.info(&ctx, vec![], InfoOptions::default()).await.unwrap();
        assert!(text.contains("Repo root:"));
        assert!(text.contains("Manifest path:"));
        assert!(text.contains("Total projects: 1"));
        assert!(text.contains("Manifest branch: main"));
        assert!(text.contains("Manifest HEAD: abc123"));
    }
}
