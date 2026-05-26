// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use crate::{Context, Error};

/// Options controlling forall behavior.
#[derive(Debug, Clone, Default)]
pub struct ForallOptions {
    /// The shell command to execute.
    pub command: String,
    /// Regex.
    pub regex: Option<String>,
    /// Inverse regex.
    pub inverse_regex: Option<String>,
    /// The list of groups this item belongs to.
    pub groups: Vec<String>,
    /// Abort on errors.
    pub abort_on_errors: bool,
    /// Whether to print a project header.
    pub project_header: bool,
    /// Interactive.
    pub interactive: bool,
}

/// Trait for forall logic.
#[async_trait::async_trait]
pub trait Forall {
    /// Run a shell command in each project worktree.

    async fn forall(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        opts: ForallOptions,
    ) -> Result<(), Error>;
}

/// Default forall implementation.
pub struct DefaultForall;

#[async_trait::async_trait]
impl Forall for DefaultForall {
    async fn forall(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        opts: ForallOptions,
    ) -> Result<(), Error> {
        for path in projects {
            let project = match ctx.client.project_by_path(&path) {
                Some(p) => p,
                None => {
                    if opts.abort_on_errors {
                        return Err(Error::InvalidArguments(format!(
                            "project not found: {path}"
                        )));
                    }
                    continue;
                }
            };

            if !project.worktree.as_std_path().exists() {
                if opts.abort_on_errors {
                    return Err(Error::InvalidArguments(format!(
                        "project worktree does not exist: {}",
                        project.worktree
                    )));
                }
                continue;
            }

            let mut cmd = tokio::process::Command::new("sh");
            cmd.arg("-c")
                .arg(&opts.command)
                .current_dir(&project.worktree)
                .env("REPO_PROJECT", &project.name)
                .env("REPO_PATH", project.relpath.as_str())
                .env("REPO_LREV", &project.revision_expr)
                .env("REPO_REMOTE", &project.remote.name);

            if opts.project_header {
                tracing::info!("project {}/ {}", project.name, project.relpath);
            }

            let output = cmd.output().await?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.is_empty() {
                tracing::info!("{}", stdout.trim_end());
            }
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.is_empty() {
                tracing::warn!("{}", stderr.trim_end());
            }

            if !output.status.success() {
                let msg = format!(
                    "command failed in {}: {}",
                    project.name,
                    String::from_utf8_lossy(&output.stderr)
                );
                if opts.abort_on_errors {
                    return Err(Error::Sync(msg));
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
    use repo_rs_model::{client::MetaProject, Project, RepoClient};
    use std::collections::HashSet;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn make_project(name: &str, relpath: &str) -> Project {
        Project {
            name: name.to_string(),
            relpath: Utf8PathBuf::from(relpath),
            worktree: Utf8PathBuf::from(format!("/tmp/repo/{relpath}")),
            gitdir: Utf8PathBuf::from(format!("/tmp/repo/.repo/projects/{relpath}.git")),
            objdir: Utf8PathBuf::from(format!("/tmp/repo/.repo/project-objects/{name}.git")),
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
            // Adjust worktree to be inside tempdir for real filesystem tests
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
            git: Arc::new(repo_rs_git::MockGitBackend::default()),
        }
    }

    #[tokio::test]
    async fn test_forall_empty() {
        let tmp = tempdir().unwrap();
        let ctx = make_context(&tmp, vec![]);
        let engine = DefaultForall;
        engine
            .forall(
                &ctx,
                vec![],
                ForallOptions {
                    command: "echo hello".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_forall_happy_path() {
        let tmp = tempdir().unwrap();
        let ctx = make_context(&tmp, vec![make_project("foo", "foo")]);
        std::fs::create_dir_all(tmp.path().join("foo")).unwrap();

        let engine = DefaultForall;
        engine
            .forall(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                ForallOptions {
                    command: "echo $REPO_PROJECT > out.txt".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let out = std::fs::read_to_string(tmp.path().join("foo/out.txt")).unwrap();
        assert_eq!(out.trim(), "foo");
    }

    #[tokio::test]
    async fn test_forall_missing_project_abort() {
        let tmp = tempdir().unwrap();
        let ctx = make_context(&tmp, vec![]);
        let engine = DefaultForall;
        let err = engine
            .forall(
                &ctx,
                vec![Utf8PathBuf::from("missing")],
                ForallOptions {
                    command: "echo hello".to_string(),
                    abort_on_errors: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("project not found"));
    }

    #[tokio::test]
    async fn test_forall_missing_project_continue() {
        let tmp = tempdir().unwrap();
        let ctx = make_context(&tmp, vec![]);
        let engine = DefaultForall;
        engine
            .forall(
                &ctx,
                vec![Utf8PathBuf::from("missing")],
                ForallOptions {
                    command: "echo hello".to_string(),
                    abort_on_errors: false,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_forall_command_failure_abort() {
        let tmp = tempdir().unwrap();
        let ctx = make_context(&tmp, vec![make_project("foo", "foo")]);
        std::fs::create_dir_all(tmp.path().join("foo")).unwrap();

        let engine = DefaultForall;
        let err = engine
            .forall(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                ForallOptions {
                    command: "exit 1".to_string(),
                    abort_on_errors: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("command failed"));
    }

    #[tokio::test]
    async fn test_forall_command_failure_continue() {
        let tmp = tempdir().unwrap();
        let ctx = make_context(&tmp, vec![make_project("foo", "foo")]);
        std::fs::create_dir_all(tmp.path().join("foo")).unwrap();

        let engine = DefaultForall;
        engine
            .forall(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                ForallOptions {
                    command: "exit 1".to_string(),
                    abort_on_errors: false,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_forall_env_vars() {
        let tmp = tempdir().unwrap();
        let ctx = make_context(&tmp, vec![make_project("foo", "foo")]);
        std::fs::create_dir_all(tmp.path().join("foo")).unwrap();

        let engine = DefaultForall;
        engine
            .forall(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                ForallOptions {
                    command: "echo $REPO_PROJECT $REPO_PATH $REPO_LREV $REPO_REMOTE > out.txt".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let out = std::fs::read_to_string(tmp.path().join("foo/out.txt")).unwrap();
        assert_eq!(out.trim(), "foo foo main origin");
    }
}
