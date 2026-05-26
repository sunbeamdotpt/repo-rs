// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Build [`RepoClient`] instances from parsed [`repo_rs_manifest::Manifest`] values.

use crate::{
    client::MetaProject,
    layout::RepoLayout,
    project::{CopyFile, LinkFile, RemoteSpec},
    Project, RepoClient,
};
use camino::Utf8PathBuf;
use indexmap::IndexMap;

/// Errors that can occur while building a [`RepoClient`].
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum BuildError {
    #[error("unknown remote reference: {0}")]
    /// A project references an undefined remote.
    UnknownRemote(String),
    #[error("project has no remote and no default remote is configured")]
    /// No remote is configured for the project.
    MissingRemote,
    #[error("invalid path: {0}")]
    /// The path is invalid.
    InvalidPath(String),
}

/// A builder that constructs a [`RepoClient`] from a parsed manifest.
#[derive(Debug, Clone)]
pub struct ManifestBuilder {
    repo_dir: Utf8PathBuf,
}

impl ManifestBuilder {
    /// Create a new builder for the given `.repo/` directory.
    pub fn new(repo_dir: Utf8PathBuf) -> Self {
        Self { repo_dir }
    }

    /// Build a [`RepoClient`] from a parsed manifest.
    ///
    /// # Errors
    ///
    /// Returns an error if a project references an unknown remote or if
    /// path resolution fails.
    pub fn build(
        &self,
        manifest: &repo_rs_manifest::model::Manifest,
    ) -> Result<RepoClient, BuildError> {
        let layout = RepoLayout::new(self.repo_dir.clone());
        let topdir = self
            .repo_dir
            .parent()
            .ok_or_else(|| BuildError::InvalidPath(self.repo_dir.to_string()))?
            .to_path_buf();
        let topdir = Utf8PathBuf::try_from(topdir).map_err(|e| {
            BuildError::InvalidPath(format!("non-utf8 topdir: {e:?}"))
        })?;

        let mut projects = IndexMap::new();
        for mp in &manifest.projects {
            let project = self.convert_project(&topdir, &layout, manifest, mp)?;
            projects.insert(project.relpath.clone(), project);
        }

        Ok(RepoClient {
            repo_dir: self.repo_dir.clone(),
            manifest_project: MetaProject {
                name: "manifests".to_string(),
                path: self.repo_dir.join("manifests"),
                gitdir: layout.manifest_git.clone(),
            },
            repo_project: MetaProject {
                name: "repo".to_string(),
                path: self.repo_dir.join("repo"),
                gitdir: layout.repo_git.clone(),
            },
            projects,
            submanifests: IndexMap::new(),
        })
    }

    /// Convert a `repo_rs_manifest::Remote` to a `repo_rs_model::RemoteSpec`.
    fn convert_remote(remote: &repo_rs_manifest::model::Remote) -> RemoteSpec {
        RemoteSpec {
            name: remote.name.clone(),
            fetch_url: remote.fetch.clone(),
            push_url: remote.pushurl.clone(),
            review_url: remote.review.clone(),
            alias: remote.alias.clone(),
        }
    }

    /// Resolve the remote for a manifest project.
    fn resolve_remote(
        &self,
        manifest: &repo_rs_manifest::model::Manifest,
        mp: &repo_rs_manifest::model::Project,
    ) -> Result<RemoteSpec, BuildError> {
        let remote_name = mp
            .remote
            .as_deref()
            .or_else(|| manifest.default.as_ref().and_then(|d| d.remote.as_deref()))
            .ok_or(BuildError::MissingRemote)?;

        manifest
            .remotes
            .get(remote_name)
            .map(|r| Self::convert_remote(r))
            .ok_or_else(|| BuildError::UnknownRemote(remote_name.to_string()))
    }

    /// Compute the revision expression for a project.
    fn resolve_revision(
        &self,
        manifest: &repo_rs_manifest::model::Manifest,
        mp: &repo_rs_manifest::model::Project,
    ) -> String {
        mp.revision
            .clone()
            .or_else(|| manifest.default.as_ref().and_then(|d| d.revision.clone()))
            .unwrap_or_else(|| "master".to_string())
    }

    /// Convert a `repo_rs_manifest::Project` to a `repo_rs_model::Project`.
    fn convert_project(
        &self,
        topdir: &Utf8PathBuf,
        layout: &RepoLayout,
        manifest: &repo_rs_manifest::model::Manifest,
        mp: &repo_rs_manifest::model::Project,
    ) -> Result<Project, BuildError> {
        let relpath = Utf8PathBuf::from(mp.path.as_deref().unwrap_or(&mp.name));
        let worktree = topdir.join(&relpath);
        let gitdir = layout.projects.join(format!("{relpath}.git"));
        let objdir = layout.project_objects.join(format!("{}.git", mp.name));
        let mut remote = self.resolve_remote(manifest, mp)?;
        // Append project name to fetch URL (standard repo behavior)
        if !remote.fetch_url.ends_with('/') {
            remote.fetch_url.push('/');
        }
        remote.fetch_url.push_str(&mp.name);
        let revision_expr = self.resolve_revision(manifest, mp);

        let mut groups = crate::resolve::auto_groups(&mp.name, &relpath);
        groups.extend(mp.groups.iter().cloned());

        let subprojects = mp
            .subprojects
            .iter()
            .map(|sp| self.convert_project(topdir, layout, manifest, sp))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Project {
            name: mp.name.clone(),
            relpath,
            worktree,
            gitdir,
            objdir,
            remote,
            revision_expr,
            revision_id: None,
            groups,
            parent: None,
            subprojects,
            copyfiles: mp
                .copyfiles
                .iter()
                .map(|cf| CopyFile {
                    src: Utf8PathBuf::from(&cf.src),
                    dest: Utf8PathBuf::from(&cf.dest),
                })
                .collect(),
            linkfiles: mp
                .linkfiles
                .iter()
                .map(|lf| LinkFile {
                    src: Utf8PathBuf::from(&lf.src),
                    dest: Utf8PathBuf::from(&lf.dest),
                })
                .collect(),
            sync_c: mp.sync_c.unwrap_or(
                manifest
                    .default
                    .as_ref()
                    .and_then(|d| d.sync_c)
                    .unwrap_or(false),
            ),
            sync_s: mp.sync_s.unwrap_or(
                manifest
                    .default
                    .as_ref()
                    .and_then(|d| d.sync_s)
                    .unwrap_or(false),
            ),
            sync_tags: mp.sync_tags.unwrap_or(
                manifest
                    .default
                    .as_ref()
                    .and_then(|d| d.sync_tags)
                    .unwrap_or(true),
            ),
            clone_depth: mp.clone_depth,
            upstream: mp
                .upstream
                .clone()
                .or_else(|| manifest.default.as_ref().and_then(|d| d.upstream.clone())),
            dest_branch: mp
                .dest_branch
                .clone()
                .or_else(|| manifest.default.as_ref().and_then(|d| d.dest_branch.clone())),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repo_rs_manifest::model as manifest;
    use std::collections::HashSet;
    use tempfile::TempDir;

    fn make_manifest() -> manifest::Manifest {
        let mut remotes = IndexMap::new();
        remotes.insert(
            "origin".to_string(),
            manifest::Remote {
                name: "origin".to_string(),
                alias: Some("o".to_string()),
                fetch: "https://example.com".to_string(),
                pushurl: Some("ssh://example.com".to_string()),
                review: Some("https://review.example.com".to_string()),
                revision: Some("main".to_string()),
                annotations: Vec::new(),
            },
        );
        remotes.insert(
            "mirror".to_string(),
            manifest::Remote {
                name: "mirror".to_string(),
                alias: None,
                fetch: "https://mirror.example.com".to_string(),
                pushurl: None,
                review: None,
                revision: None,
                annotations: Vec::new(),
            },
        );

        manifest::Manifest {
            notice: None,
            remotes,
            default: Some(manifest::Default {
                remote: Some("origin".to_string()),
                revision: Some("main".to_string()),
                dest_branch: Some("main".to_string()),
                upstream: Some("origin/main".to_string()),
                sync_j: None,
                sync_j_max: None,
                sync_c: Some(false),
                sync_s: Some(false),
                sync_tags: Some(true),
            }),
            manifest_server: None,
            submanifests: IndexMap::new(),
            remove_projects: Vec::new(),
            projects: Vec::new(),
            extend_projects: Vec::new(),
            repo_hooks: None,
            superproject: None,
            contactinfo: None,
            includes: Vec::new(),
        }
    }

    fn make_manifest_project(name: &str, path: Option<&str>) -> manifest::Project {
        manifest::Project {
            name: name.to_string(),
            path: path.map(String::from),
            remote: None,
            revision: None,
            dest_branch: None,
            groups: {
                let mut g = HashSet::new();
                g.insert("test".to_string());
                g
            },
            sync_c: None,
            sync_s: None,
            sync_tags: None,
            upstream: None,
            clone_depth: None,
            force_path: None,
            sync_strategy: None,
            annotations: Vec::new(),
            copyfiles: Vec::new(),
            linkfiles: Vec::new(),
            subprojects: Vec::new(),
        }
    }

    #[test]
    fn test_convert_remote() {
        let remote = manifest::Remote {
            name: "origin".to_string(),
            alias: Some("o".to_string()),
            fetch: "https://example.com".to_string(),
            pushurl: Some("ssh://example.com".to_string()),
            review: Some("https://review.example.com".to_string()),
            revision: Some("main".to_string()),
            annotations: Vec::new(),
        };
        let spec = ManifestBuilder::convert_remote(&remote);
        assert_eq!(spec.name, "origin");
        assert_eq!(spec.fetch_url, "https://example.com");
        assert_eq!(spec.push_url, Some("ssh://example.com".to_string()));
        assert_eq!(spec.review_url, Some("https://review.example.com".to_string()));
        assert_eq!(spec.alias, Some("o".to_string()));
    }

    #[test]
    fn test_build_empty() {
        let tmp = TempDir::new().unwrap();
        let repo_dir =
            Utf8PathBuf::try_from(tmp.path().join(".repo")).unwrap();
        std::fs::create_dir(&repo_dir).unwrap();

        let manifest = make_manifest();
        let builder = ManifestBuilder::new(repo_dir.clone());
        let client = builder.build(&manifest).unwrap();

        assert_eq!(client.repo_dir, repo_dir);
        assert_eq!(client.manifest_project.name, "manifests");
        assert_eq!(client.repo_project.name, "repo");
        assert!(client.projects.is_empty());
    }

    #[test]
    fn test_build_single_project() {
        let tmp = TempDir::new().unwrap();
        let repo_dir =
            Utf8PathBuf::try_from(tmp.path().join(".repo")).unwrap();
        std::fs::create_dir(&repo_dir).unwrap();

        let mut manifest = make_manifest();
        manifest.projects.push(make_manifest_project("foo", None));

        let builder = ManifestBuilder::new(repo_dir.clone());
        let client = builder.build(&manifest).unwrap();

        assert_eq!(client.projects.len(), 1);
        let project = client.projects.get(&Utf8PathBuf::from("foo")).unwrap();
        assert_eq!(project.name, "foo");
        assert_eq!(project.relpath, "foo");
        assert!(project
            .worktree
            .as_str()
            .ends_with("/foo"));
        assert!(project
            .gitdir
            .as_str()
            .ends_with(".repo/projects/foo.git"));
        assert!(project
            .objdir
            .as_str()
            .ends_with(".repo/project-objects/foo.git"));
        assert_eq!(project.remote.name, "origin");
        assert_eq!(project.revision_expr, "main");
        assert!(project.groups.contains("all"));
        assert!(project.groups.contains("name:foo"));
        assert!(project.groups.contains("path:foo"));
        assert!(project.groups.contains("test"));
    }

    #[test]
    fn test_build_project_with_path() {
        let tmp = TempDir::new().unwrap();
        let repo_dir =
            Utf8PathBuf::try_from(tmp.path().join(".repo")).unwrap();
        std::fs::create_dir(&repo_dir).unwrap();

        let mut manifest = make_manifest();
        manifest
            .projects
            .push(make_manifest_project("platform/build", Some("build")));

        let builder = ManifestBuilder::new(repo_dir.clone());
        let client = builder.build(&manifest).unwrap();

        let project = client.projects.get(&Utf8PathBuf::from("build")).unwrap();
        assert_eq!(project.name, "platform/build");
        assert_eq!(project.relpath, "build");
        assert!(project
            .objdir
            .as_str()
            .ends_with(".repo/project-objects/platform/build.git"));
        assert!(project.groups.contains("name:platform/build"));
        assert!(project.groups.contains("path:build"));
    }

    #[test]
    fn test_build_project_with_custom_remote() {
        let tmp = TempDir::new().unwrap();
        let repo_dir =
            Utf8PathBuf::try_from(tmp.path().join(".repo")).unwrap();
        std::fs::create_dir(&repo_dir).unwrap();

        let mut manifest = make_manifest();
        let mut project = make_manifest_project("foo", None);
        project.remote = Some("mirror".to_string());
        manifest.projects.push(project);

        let builder = ManifestBuilder::new(repo_dir.clone());
        let client = builder.build(&manifest).unwrap();

        let project = client.projects.get(&Utf8PathBuf::from("foo")).unwrap();
        assert_eq!(project.remote.name, "mirror");
        assert_eq!(project.remote.fetch_url, "https://mirror.example.com/foo");
        assert_eq!(project.remote.push_url, None);
    }

    #[test]
    fn test_build_project_unknown_remote() {
        let tmp = TempDir::new().unwrap();
        let repo_dir =
            Utf8PathBuf::try_from(tmp.path().join(".repo")).unwrap();
        std::fs::create_dir(&repo_dir).unwrap();

        let mut manifest = make_manifest();
        let mut project = make_manifest_project("foo", None);
        project.remote = Some("missing".to_string());
        manifest.projects.push(project);

        let builder = ManifestBuilder::new(repo_dir.clone());
        let result = builder.build(&manifest);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("missing"));
    }

    #[test]
    fn test_build_no_default_remote() {
        let tmp = TempDir::new().unwrap();
        let repo_dir =
            Utf8PathBuf::try_from(tmp.path().join(".repo")).unwrap();
        std::fs::create_dir(&repo_dir).unwrap();

        let mut manifest = make_manifest();
        manifest.default = None;
        manifest.projects.push(make_manifest_project("foo", None));

        let builder = ManifestBuilder::new(repo_dir.clone());
        let result = builder.build(&manifest);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no default remote"));
    }

    #[test]
    fn test_build_project_with_copyfiles_and_linkfiles() {
        let tmp = TempDir::new().unwrap();
        let repo_dir =
            Utf8PathBuf::try_from(tmp.path().join(".repo")).unwrap();
        std::fs::create_dir(&repo_dir).unwrap();

        let mut manifest = make_manifest();
        let mut project = make_manifest_project("foo", None);
        project.copyfiles.push(manifest::CopyFile {
            src: "src.txt".to_string(),
            dest: "dest.txt".to_string(),
        });
        project.linkfiles.push(manifest::LinkFile {
            src: "link_src".to_string(),
            dest: "link_dest".to_string(),
        });
        manifest.projects.push(project);

        let builder = ManifestBuilder::new(repo_dir.clone());
        let client = builder.build(&manifest).unwrap();

        let project = client.projects.get(&Utf8PathBuf::from("foo")).unwrap();
        assert_eq!(project.copyfiles.len(), 1);
        assert_eq!(project.copyfiles[0].src, "src.txt");
        assert_eq!(project.copyfiles[0].dest, "dest.txt");
        assert_eq!(project.linkfiles.len(), 1);
        assert_eq!(project.linkfiles[0].src, "link_src");
        assert_eq!(project.linkfiles[0].dest, "link_dest");
    }

    #[test]
    fn test_build_project_with_subprojects() {
        let tmp = TempDir::new().unwrap();
        let repo_dir =
            Utf8PathBuf::try_from(tmp.path().join(".repo")).unwrap();
        std::fs::create_dir(&repo_dir).unwrap();

        let mut manifest = make_manifest();
        let mut parent = make_manifest_project("parent", None);
        parent.subprojects.push(make_manifest_project("child", Some("parent/child")));
        manifest.projects.push(parent);

        let builder = ManifestBuilder::new(repo_dir.clone());
        let client = builder.build(&manifest).unwrap();

        let project = client
            .projects
            .get(&Utf8PathBuf::from("parent"))
            .unwrap();
        assert_eq!(project.subprojects.len(), 1);
        assert_eq!(project.subprojects[0].name, "child");
        assert_eq!(project.subprojects[0].relpath, "parent/child");
    }

    #[test]
    fn test_build_sync_flags_from_default() {
        let tmp = TempDir::new().unwrap();
        let repo_dir =
            Utf8PathBuf::try_from(tmp.path().join(".repo")).unwrap();
        std::fs::create_dir(&repo_dir).unwrap();

        let mut manifest = make_manifest();
        manifest.default = Some(manifest::Default {
            remote: Some("origin".to_string()),
            revision: Some("dev".to_string()),
            dest_branch: None,
            upstream: None,
            sync_j: None,
            sync_j_max: None,
            sync_c: Some(true),
            sync_s: Some(true),
            sync_tags: Some(false),
        });
        manifest.projects.push(make_manifest_project("foo", None));

        let builder = ManifestBuilder::new(repo_dir.clone());
        let client = builder.build(&manifest).unwrap();

        let project = client.projects.get(&Utf8PathBuf::from("foo")).unwrap();
        assert!(project.sync_c);
        assert!(project.sync_s);
        assert!(!project.sync_tags);
        assert_eq!(project.revision_expr, "dev");
    }

    #[test]
    fn test_build_sync_flags_override() {
        let tmp = TempDir::new().unwrap();
        let repo_dir =
            Utf8PathBuf::try_from(tmp.path().join(".repo")).unwrap();
        std::fs::create_dir(&repo_dir).unwrap();

        let mut manifest = make_manifest();
        let mut project = make_manifest_project("foo", None);
        project.sync_c = Some(true);
        project.sync_s = Some(false);
        project.sync_tags = Some(true);
        manifest.projects.push(project);

        let builder = ManifestBuilder::new(repo_dir.clone());
        let client = builder.build(&manifest).unwrap();

        let project = client.projects.get(&Utf8PathBuf::from("foo")).unwrap();
        assert!(project.sync_c);
        assert!(!project.sync_s);
        assert!(project.sync_tags);
    }

    #[test]
    fn test_build_upstream_and_dest_branch() {
        let tmp = TempDir::new().unwrap();
        let repo_dir =
            Utf8PathBuf::try_from(tmp.path().join(".repo")).unwrap();
        std::fs::create_dir(&repo_dir).unwrap();

        let mut manifest = make_manifest();
        let mut project = make_manifest_project("foo", None);
        project.upstream = Some("custom/upstream".to_string());
        project.dest_branch = Some("custom/dest".to_string());
        manifest.projects.push(project);

        let builder = ManifestBuilder::new(repo_dir.clone());
        let client = builder.build(&manifest).unwrap();

        let project = client.projects.get(&Utf8PathBuf::from("foo")).unwrap();
        assert_eq!(project.upstream, Some("custom/upstream".to_string()));
        assert_eq!(project.dest_branch, Some("custom/dest".to_string()));
    }

    #[test]
    fn test_build_clone_depth() {
        let tmp = TempDir::new().unwrap();
        let repo_dir =
            Utf8PathBuf::try_from(tmp.path().join(".repo")).unwrap();
        std::fs::create_dir(&repo_dir).unwrap();

        let mut manifest = make_manifest();
        let mut project = make_manifest_project("foo", None);
        project.clone_depth = Some(5);
        manifest.projects.push(project);

        let builder = ManifestBuilder::new(repo_dir.clone());
        let client = builder.build(&manifest).unwrap();

        let project = client.projects.get(&Utf8PathBuf::from("foo")).unwrap();
        assert_eq!(project.clone_depth, Some(5));
    }

    #[test]
    fn test_build_multiple_projects() {
        let tmp = TempDir::new().unwrap();
        let repo_dir =
            Utf8PathBuf::try_from(tmp.path().join(".repo")).unwrap();
        std::fs::create_dir(&repo_dir).unwrap();

        let mut manifest = make_manifest();
        manifest.projects.push(make_manifest_project("foo", None));
        manifest.projects.push(make_manifest_project("bar", Some("baz")));

        let builder = ManifestBuilder::new(repo_dir.clone());
        let client = builder.build(&manifest).unwrap();

        assert_eq!(client.projects.len(), 2);
        assert!(client.projects.contains_key(&Utf8PathBuf::from("foo")));
        assert!(client.projects.contains_key(&Utf8PathBuf::from("baz")));
    }

    #[test]
    fn test_build_invalid_repo_dir_no_parent() {
        let manifest = make_manifest();
        let builder = ManifestBuilder::new(Utf8PathBuf::from("/"));
        let result = builder.build(&manifest);
        assert!(result.is_err());
    }
}
