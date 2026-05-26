// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use crate::{Context, Error};

/// Trait for overview logic.
#[async_trait::async_trait]
pub trait Overview {
    /// Show an overview of unmerged branches.
    async fn overview(&self, ctx: &Context, projects: Vec<Utf8PathBuf>) -> Result<String, Error>;
}

/// Default overview implementation.
pub struct DefaultOverview;

#[async_trait::async_trait]
impl Overview for DefaultOverview {
    async fn overview(&self, ctx: &Context, _projects: Vec<Utf8PathBuf>) -> Result<String, Error> {
        let total = ctx.client.projects.len();
        let mut lines = Vec::new();
        lines.push(format!("Projects: {total}"));
        for (path, project) in &ctx.client.projects {
            lines.push(format!("  {} -> {}", path, project.name));
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
        let repo_dir = Utf8PathBuf::from("/tmp/.repo");
        let mut projects = indexmap::IndexMap::new();
        projects.insert(Utf8PathBuf::from("foo"), make_project("foo", "foo"));
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
    async fn test_overview() {
        let ctx = make_context();
        let engine = DefaultOverview;
        let text = engine.overview(&ctx, vec![]).await.unwrap();
        assert!(text.contains("Projects: 1"));
        assert!(text.contains("foo -> foo"));
    }
}
