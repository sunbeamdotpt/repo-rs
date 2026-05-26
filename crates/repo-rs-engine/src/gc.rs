// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{Context, Error};

/// Options controlling gc behavior.
#[derive(Debug, Clone, Default)]
pub struct GcOptions {
    /// Aggressive.
    pub aggressive: bool,
    /// Dry run.
    pub dry_run: bool,
    /// Repack.
    pub repack: bool,
}

/// Trait for gc logic.
#[async_trait::async_trait]
pub trait Gc {
    /// Run garbage collection on project repositories.
    async fn gc(&self, ctx: &Context, opts: GcOptions) -> Result<Vec<String>, Error>;
}

/// Default gc implementation.
pub struct DefaultGc;

#[async_trait::async_trait]
impl Gc for DefaultGc {
    /// Run garbage collection on project repositories.
    async fn gc(&self, ctx: &Context, opts: GcOptions) -> Result<Vec<String>, Error> {
        let mut to_delete = Vec::new();

        // Build sets of paths that are still in use.
        let mut valid_gitdirs = std::collections::HashSet::new();
        let mut valid_objdirs = std::collections::HashSet::new();
        for project in ctx.client.projects.values() {
            valid_gitdirs.insert(project.gitdir.as_str().to_string());
            valid_objdirs.insert(project.objdir.as_str().to_string());
        }

        // Scan .repo/projects/ for orphaned .git directories.
        let projects_dir = ctx.client.repo_dir.join("projects");
        if projects_dir.exists() {
            Self::scan_orphans(&projects_dir, &valid_gitdirs, &mut to_delete)?;
        }

        // Scan .repo/project-objects/ for orphaned object stores.
        let objects_dir = ctx.client.repo_dir.join("project-objects");
        if objects_dir.exists() {
            Self::scan_orphans(&objects_dir, &valid_objdirs, &mut to_delete)?;
        }

        if opts.dry_run {
            return Ok(to_delete);
        }

        for path in &to_delete {
            tokio::fs::remove_dir_all(path)
                .await
                .map_err(Error::Io)?;
        }

        // Repack is a no-op for now; it would require iterating repos and
        // calling git2 packbuilder APIs.
        let _ = opts.repack;
        let _ = opts.aggressive;

        Ok(to_delete)
    }
}

impl DefaultGc {
    fn scan_orphans(
        root: &camino::Utf8Path,
        valid: &std::collections::HashSet<String>,
        orphans: &mut Vec<String>,
    ) -> Result<(), Error> {
        for entry in std::fs::read_dir(root).map_err(|e| Error::Io(e))? {
            let entry = entry.map_err(|e| Error::Io(e))?;
            let path = entry.path();
            let path_str = path.to_string_lossy().to_string();
            if !valid.contains(&path_str) {
                orphans.push(path_str);
            }
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

    fn make_project(name: &str, relpath: &str, repo_dir: &camino::Utf8Path) -> Project {
        Project {
            name: name.to_string(),
            relpath: Utf8PathBuf::from(relpath),
            worktree: repo_dir.join("..").join(relpath),
            gitdir: repo_dir.join(format!("projects/{relpath}.git")),
            objdir: repo_dir.join(format!("project-objects/{name}.git")),
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
        projects.insert(
            Utf8PathBuf::from("foo"),
            make_project("foo", "foo", &repo_dir),
        );
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
    async fn test_gc_dry_run_finds_orphans() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp);

        // Create an orphaned project gitdir.
        let orphan = tmp.path().join(".repo/projects/orphan.git");
        std::fs::create_dir_all(&orphan).unwrap();

        let engine = DefaultGc;
        let deleted = engine.gc(&ctx, GcOptions { dry_run: true, ..Default::default() }).await.unwrap();
        assert!(deleted.contains(&orphan.to_string_lossy().to_string()));
    }

    #[tokio::test]
    async fn test_gc_dry_run_keeps_valid() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp);

        // Create the valid project gitdir.
        let valid = tmp.path().join(".repo/projects/foo.git");
        std::fs::create_dir_all(&valid).unwrap();

        let engine = DefaultGc;
        let deleted = engine.gc(&ctx, GcOptions { dry_run: true, ..Default::default() }).await.unwrap();
        assert!(!deleted.contains(&valid.to_string_lossy().to_string()));
    }

    #[tokio::test]
    async fn test_gc_deletes_orphans() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp);

        let orphan = tmp.path().join(".repo/projects/orphan.git");
        std::fs::create_dir_all(&orphan).unwrap();

        let engine = DefaultGc;
        let deleted = engine.gc(&ctx, GcOptions { dry_run: false, ..Default::default() }).await.unwrap();
        assert!(deleted.contains(&orphan.to_string_lossy().to_string()));
        assert!(!orphan.exists());
    }

    #[tokio::test]
    async fn test_gc_no_orphans() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp);

        let engine = DefaultGc;
        let deleted = engine.gc(&ctx, GcOptions::default()).await.unwrap();
        assert!(deleted.is_empty());
    }

    #[tokio::test]
    async fn test_gc_scans_project_objects() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp);

        let orphan = tmp.path().join(".repo/project-objects/orphan.git");
        std::fs::create_dir_all(&orphan).unwrap();

        let engine = DefaultGc;
        let deleted = engine.gc(&ctx, GcOptions { dry_run: true, ..Default::default() }).await.unwrap();
        assert!(deleted.contains(&orphan.to_string_lossy().to_string()));
    }
}
