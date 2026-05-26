// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{Context, Error};
use std::collections::HashSet;

/// Options controlling list behavior.
#[derive(Debug, Clone, Default)]
pub struct ListOptions {
    /// Paths only.
    pub paths_only: bool,
    /// Names only.
    pub names_only: bool,
    /// The list of groups this item belongs to.
    pub groups: Vec<String>,
    /// Regex.
    pub regex: Option<String>,
    /// Name only.
    pub name_only: bool,
    /// Path only.
    pub path_only: bool,
    /// Fullpath.
    pub fullpath: bool,
}

/// Trait for list logic.
#[async_trait::async_trait]
pub trait List {
    /// List all projects in the manifest.
    async fn list(&self, ctx: &Context, opts: ListOptions) -> Result<String, Error>;
}

/// Default list implementation.
pub struct DefaultList;

#[async_trait::async_trait]
impl List for DefaultList {
    /// List all projects in the manifest.
    async fn list(&self, ctx: &Context, opts: ListOptions) -> Result<String, Error> {
        let mut lines = Vec::new();
        let group_filter: HashSet<String> = opts.groups.iter().cloned().collect();
        for (path, project) in &ctx.client.projects {
            if !group_filter.is_empty() && !project.has_any_group(&group_filter) {
                continue;
            }
            if opts.paths_only {
                lines.push(path.to_string());
            } else if opts.names_only {
                lines.push(project.name.clone());
            } else {
                lines.push(format!("{} : {}", path, project.name));
            }
        }
        Ok(lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use camino::Utf8PathBuf;
    use repo_rs_git::MockGitBackend;
    use repo_rs_model::{client::MetaProject, Project, RepoClient};
    use std::sync::Arc;

    fn make_project(name: &str, relpath: &str, groups: Vec<&str>) -> Project {
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
            groups: groups.into_iter().map(String::from).collect(),
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
        let repo_dir = Utf8PathBuf::from("/tmp/.repo");
        let mut projects = indexmap::IndexMap::new();
        projects.insert(
            Utf8PathBuf::from("foo"),
            make_project("foo", "foo", vec!["all"]),
        );
        projects.insert(
            Utf8PathBuf::from("bar"),
            make_project("bar", "bar", vec!["all", "test"]),
        );
        Context {
            repo_root: Utf8PathBuf::from("/tmp"),
            client: RepoClient {
                repo_dir: repo_dir.clone(),
                manifest_project: MetaProject {
                    name: "manifests".to_string(),
                    path: repo_dir.join("manifests"),
                    gitdir: repo_dir.join("manifests.git"),
                },
                repo_project: MetaProject {
                    name: "repo".to_string(),
                    path: repo_dir.join("repo"),
                    gitdir: repo_dir.join("repo"),
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
    async fn test_list_default() {
        let ctx = make_context();
        let engine = DefaultList;
        let text = engine.list(&ctx, ListOptions::default()).await.unwrap();
        assert!(text.contains("foo : foo"));
        assert!(text.contains("bar : bar"));
    }

    #[tokio::test]
    async fn test_list_paths_only() {
        let ctx = make_context();
        let engine = DefaultList;
        let text = engine
            .list(&ctx, ListOptions {
                paths_only: true,
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(text.contains("foo"));
        assert!(!text.contains("foo :"));
    }

    #[tokio::test]
    async fn test_list_names_only() {
        let ctx = make_context();
        let engine = DefaultList;
        let text = engine
            .list(&ctx, ListOptions {
                names_only: true,
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(text.contains("foo"));
        assert!(!text.contains("foo :"));
    }

    #[tokio::test]
    async fn test_list_group_filter() {
        let ctx = make_context();
        let engine = DefaultList;
        let text = engine
            .list(&ctx, ListOptions {
                groups: vec!["test".to_string()],
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!text.contains("foo : foo"));
        assert!(text.contains("bar : bar"));
    }

    #[tokio::test]
    async fn test_list_empty() {
        let mut ctx = make_context();
        ctx.client.projects.clear();
        let engine = DefaultList;
        let text = engine.list(&ctx, ListOptions::default()).await.unwrap();
        assert!(text.is_empty());
    }
}
