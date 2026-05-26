// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use crate::{Context, Error};

/// Options controlling upload behavior.
#[derive(Debug, Clone, Default)]
pub struct UploadOptions {
    /// The name of the currently checked-out branch.
    pub current_branch: bool,
    /// The topic branch name.
    pub topic: Option<String>,
    /// Hashtags.
    pub hashtags: Vec<String>,
    /// Labels.
    pub labels: Vec<String>,
    /// Reviewers.
    pub reviewers: Vec<String>,
    /// Cc.
    pub cc: Vec<String>,
    /// Private.
    pub private: bool,
    /// Wip.
    pub wip: bool,
    /// Ready.
    pub ready: bool,
    /// Dry run.
    pub dry_run: bool,
    /// Destination.
    pub destination: Option<String>,
    /// Push options.
    pub push_options: Vec<String>,
}

/// Result of an upload operation.
#[derive(Debug, Clone, Default)]
pub struct UploadReport {
    /// Pushed.
    pub pushed: Vec<String>,
    /// Errors.
    pub errors: Vec<String>,
}

/// Trait for upload logic.
#[async_trait::async_trait]
pub trait Upload {
    /// Upload changes to the code review server.
    async fn upload(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        opts: UploadOptions,
    ) -> Result<UploadReport, Error>;
}

/// Default upload implementation.
pub struct DefaultUpload;

#[async_trait::async_trait]
impl Upload for DefaultUpload {
    async fn upload(
        &self,
        ctx: &Context,
        projects: Vec<Utf8PathBuf>,
        opts: UploadOptions,
    ) -> Result<UploadReport, Error> {
        let mut report = UploadReport::default();

        for path in projects {
            let project = match ctx.client.project_by_path(&path) {
                Some(p) => p,
                None => {
                    report.errors.push(format!("project not found: {path}"));
                    continue;
                }
            };

            let repo = match ctx.git.open(&project.worktree).await {
                Ok(r) => r,
                Err(e) => {
                    report.errors.push(format!(
                        "failed to open {}: {e}",
                        project.name
                    ));
                    continue;
                }
            };

            let branches = if opts.current_branch {
                match ctx.git.head_name(&repo).await {
                    Ok(Some(name)) if name != "HEAD" => vec![(format!("refs/heads/{name}"), name)],
                    _ => {
                        report.errors.push(format!(
                            "{}: no current branch",
                            project.name
                        ));
                        continue;
                    }
                }
            } else {
                match ctx.git.ref_list(&repo, Some("refs/heads/")).await {
                    Ok(refs) => refs
                        .into_iter()
                        .map(|(name, _)| {
                            let short = name.trim_start_matches("refs/heads/").to_string();
                            (name, short)
                        })
                        .collect(),
                    Err(e) => {
                        report.errors.push(format!(
                            "failed to list branches in {}: {e}",
                            project.name
                        ));
                        continue;
                    }
                }
            };

            let dest = opts
                .destination
                .clone()
                .or_else(|| project.dest_branch.clone())
                .unwrap_or_else(|| project.revision_expr.clone());

            let mut refspecs = Vec::new();
            for (full_name, short_name) in branches {
                let upstream = format!("{dest}");
                let ahead = ctx
                    .git
                    .ahead_behind(&repo, &upstream)
                    .await
                    .map(|(a, _)| a)
                    .unwrap_or(1); // assume uploadable if we can't tell

                if ahead == 0 {
                    continue;
                }

                let mut refspec = format!("{full_name}:refs/for/{dest}");
                let mut push_opts = Vec::new();
                if let Some(topic) = &opts.topic {
                    push_opts.push(format!("topic={topic}"));
                }
                if opts.private {
                    push_opts.push("private".to_string());
                }
                if opts.wip {
                    push_opts.push("wip".to_string());
                }
                if opts.ready {
                    push_opts.push("ready".to_string());
                }
                for label in &opts.labels {
                    push_opts.push(format!("l={label}"));
                }
                for reviewer in &opts.reviewers {
                    push_opts.push(format!("r={reviewer}"));
                }
                for cc in &opts.cc {
                    push_opts.push(format!("cc={cc}"));
                }
                for hashtag in &opts.hashtags {
                    push_opts.push(format!("hashtag={hashtag}"));
                }
                if !push_opts.is_empty() {
                    refspec.push('%');
                    refspec.push_str(&push_opts.join(","));
                }
                refspecs.push(refspec);
                report.pushed.push(format!("{}: {short_name} -> {dest}", project.name));
            }

            if refspecs.is_empty() {
                continue;
            }

            if opts.dry_run {
                for r in &refspecs {
                    report.pushed.push(format!("{}: would push {r}", project.name));
                }
                continue;
            }

            if let Err(e) = ctx
                .git
                .push(&repo, &project.remote.name, &refspecs, ctx.progress.as_ref())
                .await
            {
                report.errors.push(format!(
                    "failed to push {}: {e}",
                    project.name
                ));
            }
        }

        Ok(report)
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
            dest_branch: Some("main".to_string()),
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
    async fn test_upload_empty() {
        let tmp = tempdir().unwrap();
        let ctx = make_context(&tmp, vec![]);
        let engine = DefaultUpload;
        let report = engine.upload(&ctx, vec![], UploadOptions::default()).await.unwrap();
        assert!(report.pushed.is_empty());
        assert!(report.errors.is_empty());
    }

    #[tokio::test]
    async fn test_upload_current_branch() {
        let tmp = tempdir().unwrap();
        let mut ctx = make_context(&tmp, vec![make_project("foo", "foo")]);
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|_| {
                Box::pin(async {
                    let dir = tempdir().unwrap();
                    let p = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
                    Ok(git2::Repository::init(&p).unwrap())
                })
            });
        mock.expect_head_name()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some("feature".to_string())) }));
        mock.expect_ahead_behind()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok((1, 0)) }));
        mock.expect_push()
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultUpload;
        let report = engine
            .upload(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                UploadOptions {
                    current_branch: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(report.pushed.len(), 1);
        assert!(report.pushed[0].contains("feature"));
        assert!(report.errors.is_empty());
    }

    #[tokio::test]
    async fn test_upload_all_branches() {
        let tmp = tempdir().unwrap();
        let mut ctx = make_context(&tmp, vec![make_project("foo", "foo")]);
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|_| {
                Box::pin(async {
                    let dir = tempdir().unwrap();
                    let p = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
                    Ok(git2::Repository::init(&p).unwrap())
                })
            });
        mock.expect_ref_list()
            .times(1)
            .returning(|_, _| {
                Box::pin(async {
                    Ok(vec![
                        ("refs/heads/feature".to_string(), "abc".to_string()),
                        ("refs/heads/fix".to_string(), "def".to_string()),
                    ])
                })
            });
        mock.expect_ahead_behind()
            .times(2)
            .returning(|_, _| Box::pin(async { Ok((1, 0)) }));
        mock.expect_push()
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultUpload;
        let report = engine
            .upload(&ctx, vec![Utf8PathBuf::from("foo")], UploadOptions::default())
            .await
            .unwrap();
        assert_eq!(report.pushed.len(), 2);
        assert!(report.errors.is_empty());
    }

    #[tokio::test]
    async fn test_upload_dry_run() {
        let tmp = tempdir().unwrap();
        let mut ctx = make_context(&tmp, vec![make_project("foo", "foo")]);
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|_| {
                Box::pin(async {
                    let dir = tempdir().unwrap();
                    let p = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
                    Ok(git2::Repository::init(&p).unwrap())
                })
            });
        mock.expect_head_name()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some("feature".to_string())) }));
        mock.expect_ahead_behind()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok((1, 0)) }));
        mock.expect_push().times(0);
        ctx.git = Arc::new(mock);

        let engine = DefaultUpload;
        let report = engine
            .upload(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                UploadOptions {
                    current_branch: true,
                    dry_run: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(!report.pushed.is_empty());
        assert!(report.pushed.iter().any(|s| s.contains("would push")));
        assert!(report.errors.is_empty());
    }

    #[tokio::test]
    async fn test_upload_missing_project() {
        let tmp = tempdir().unwrap();
        let ctx = make_context(&tmp, vec![]);
        let engine = DefaultUpload;
        let report = engine
            .upload(&ctx, vec![Utf8PathBuf::from("missing")], UploadOptions::default())
            .await
            .unwrap();
        assert!(report.pushed.is_empty());
        assert_eq!(report.errors, vec!["project not found: missing"]);
    }

    #[tokio::test]
    async fn test_upload_no_branches() {
        let tmp = tempdir().unwrap();
        let mut ctx = make_context(&tmp, vec![make_project("foo", "foo")]);
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|_| {
                Box::pin(async {
                    let dir = tempdir().unwrap();
                    let p = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
                    Ok(git2::Repository::init(&p).unwrap())
                })
            });
        mock.expect_head_name()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultUpload;
        let report = engine
            .upload(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                UploadOptions {
                    current_branch: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(report.pushed.is_empty());
        assert!(report.errors[0].contains("no current branch"));
    }

    #[tokio::test]
    async fn test_upload_push_error() {
        let tmp = tempdir().unwrap();
        let mut ctx = make_context(&tmp, vec![make_project("foo", "foo")]);
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|_| {
                Box::pin(async {
                    let dir = tempdir().unwrap();
                    let p = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
                    Ok(git2::Repository::init(&p).unwrap())
                })
            });
        mock.expect_head_name()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some("feature".to_string())) }));
        mock.expect_ahead_behind()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok((1, 0)) }));
        mock.expect_push()
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Err(repo_rs_git::Error::Git2("push failed".to_string())) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultUpload;
        let report = engine
            .upload(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                UploadOptions {
                    current_branch: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(!report.pushed.is_empty()); // still recorded as attempted
        assert!(report.errors[0].contains("push failed"));
    }

    #[tokio::test]
    async fn test_upload_with_options() {
        let tmp = tempdir().unwrap();
        let mut ctx = make_context(&tmp, vec![make_project("foo", "foo")]);
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|_| {
                Box::pin(async {
                    let dir = tempdir().unwrap();
                    let p = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
                    Ok(git2::Repository::init(&p).unwrap())
                })
            });
        mock.expect_head_name()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some("feature".to_string())) }));
        mock.expect_ahead_behind()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok((1, 0)) }));
        mock.expect_push()
            .times(1)
            .withf(|_, _, refspecs, _| {
                let r = &refspecs[0];
                r.contains("topic=my-topic") && r.contains("private") && r.contains("wip")
            })
            .returning(|_, _, _, _| Box::pin(async { Ok(()) }));
        ctx.git = Arc::new(mock);

        let engine = DefaultUpload;
        let report = engine
            .upload(
                &ctx,
                vec![Utf8PathBuf::from("foo")],
                UploadOptions {
                    current_branch: true,
                    topic: Some("my-topic".to_string()),
                    private: true,
                    wip: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(!report.pushed.is_empty());
        assert!(report.errors.is_empty());
    }
}
