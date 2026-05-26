// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use crate::{Context, Error};

/// Options controlling grep behavior.
#[derive(Debug, Clone, Default)]
pub struct GrepOptions {
    /// Pattern.
    pub pattern: String,
    /// Line number.
    pub line_number: bool,
    /// Files with matches.
    pub files_with_matches: bool,
    /// Files without match.
    pub files_without_match: bool,
    /// Cached.
    pub cached: bool,
    /// The git revision (branch, tag, or commit SHA).
    pub revision: Option<String>,
    /// Ignore case.
    pub ignore_case: bool,
    /// Word regexp.
    pub word_regexp: bool,
    /// Invert match.
    pub invert_match: bool,
    /// Extended regexp.
    pub extended_regexp: bool,
    /// Fixed strings.
    pub fixed_strings: bool,
}

/// Trait for grep logic.
#[async_trait::async_trait]
pub trait Grep {
    /// Search for patterns across project worktrees.
    async fn grep(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        opts: GrepOptions,
    ) -> Result<(), Error>;
}

/// Default grep implementation.
pub struct DefaultGrep;

#[async_trait::async_trait]
impl Grep for DefaultGrep {
    async fn grep(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        opts: GrepOptions,
    ) -> Result<(), Error> {
        let regex = regex::Regex::new(&opts.pattern).map_err(|e| {
            Error::InvalidArguments(format!("invalid regex: {e}"))
        })?;
        for path in projects {
            let project = match ctx.client.project_by_path(&path) {
                Some(p) => p,
                None => {
                    return Err(Error::InvalidArguments(format!(
                        "project not found: {path}"
                    )));
                }
            };
            let worktree = std::path::Path::new(project.worktree.as_str());
            if !worktree.exists() {
                continue;
            }
            let walker = walkdir::WalkDir::new(worktree)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file());
            for entry in walker {
                let file_path = entry.path();
                let content = match tokio::fs::read_to_string(file_path).await {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let mut matched = false;
                for (i, line) in content.lines().enumerate() {
                    if regex.is_match(line) {
                        matched = true;
                        if opts.files_with_matches {
                            tracing::info!("{}", file_path.display());
                            break;
                        }
                        if opts.files_without_match {
                            break;
                        }
                        let prefix = if opts.line_number {
                            format!("{}:{}:", file_path.display(), i + 1)
                        } else {
                            format!("{}:", file_path.display())
                        };
                        tracing::info!("{}{}", prefix, line);
                    }
                }
                if opts.files_without_match && !matched {
                    tracing::info!("{}", file_path.display());
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
    async fn test_grep_invalid_regex() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp);
        let engine = DefaultGrep;
        let err = engine
            .grep(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                GrepOptions {
                    pattern: "[invalid".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("invalid regex"));
    }

    #[tokio::test]
    async fn test_grep_missing_project() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp);
        let engine = DefaultGrep;
        let err = engine
            .grep(
                &ctx,
                vec![Utf8PathBuf::from("missing")],
                GrepOptions {
                    pattern: "test".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("project not found"));
    }
}
