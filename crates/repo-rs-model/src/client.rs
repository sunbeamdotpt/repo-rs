// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::Project;
use camino::Utf8PathBuf;
use indexmap::IndexMap;

#[cfg(test)]
use std::collections::HashSet;

/// The in-memory representation of a repo client checkout.
#[derive(Debug, Clone)]
pub struct RepoClient {
    /// Absolute path to the `.repo/` directory.
    pub repo_dir: Utf8PathBuf,
    /// The manifest project (`.repo/manifests`).
    pub manifest_project: MetaProject,
    /// The repo tool project (`.repo/repo`).
    pub repo_project: MetaProject,
    /// All projects indexed by relative path.
    pub projects: IndexMap<Utf8PathBuf, Project>,
    /// Submanifest clients (for nested manifests).
    pub submanifests: IndexMap<String, RepoClient>,
}

/// A meta-project housed under `.repo/` itself.
#[derive(Debug, Clone)]
pub struct MetaProject {
    /// The name of this item.
    pub name: String,
    /// The filesystem path.
    pub path: Utf8PathBuf,
    /// The path to the project's git directory.
    pub gitdir: Utf8PathBuf,
}

impl RepoClient {
    /// Resolve a project by its relative path.
    #[must_use]
    pub fn project_by_path(&self, path: &Utf8PathBuf) -> Option<&Project> {
        self.projects.get(path)
    }

    /// Resolve a project by its server-side name.
    ///
    /// Note: multiple projects may share the same name but different
    /// paths. This returns the first match.
    #[must_use]
    pub fn project_by_name(&self, name: &str) -> Option<&Project> {
        self.projects.values().find(|p| p.name == name)
    }

    /// Find all projects matching a regex pattern on name or path.
    pub fn find_projects(&self, pattern: &str, inverse: bool) -> Result<Vec<&Project>, Error> {
        let regex = regex::Regex::new(pattern)?;
        let mut results: Vec<&Project> = self
            .projects
            .values()
            .filter(|p| {
                let matches = regex.is_match(&p.name) || regex.is_match(p.relpath.as_str());
                if inverse {
                    !matches
                } else {
                    matches
                }
            })
            .collect();
        results.sort_by_key(|p| &p.relpath);
        Ok(results)
    }
}

use crate::Error;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::RemoteSpec;

    fn make_project(name: &str, relpath: &str) -> Project {
        Project {
            name: name.to_string(),
            relpath: Utf8PathBuf::from(relpath),
            worktree: Utf8PathBuf::from(format!("/tmp/{relpath}")),
            gitdir: Utf8PathBuf::from(format!("/tmp/.repo/projects/{relpath}.git")),
            objdir: Utf8PathBuf::from(format!("/tmp/.repo/project-objects/{name}.git")),
            remote: RemoteSpec {
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

    fn make_client() -> RepoClient {
        let repo_dir = Utf8PathBuf::from("/tmp/.repo");
        RepoClient {
            repo_dir: repo_dir.clone(),
            manifest_project: MetaProject {
                name: "manifest".to_string(),
                path: repo_dir.join("manifests"),
                gitdir: repo_dir.join("manifests.git"),
            },
            repo_project: MetaProject {
                name: "repo".to_string(),
                path: repo_dir.join("repo"),
                gitdir: repo_dir.join("repo"),
            },
            projects: {
                let mut map = IndexMap::new();
                map.insert(Utf8PathBuf::from("foo"), make_project("foo", "foo"));
                map.insert(Utf8PathBuf::from("bar"), make_project("bar", "bar"));
                map.insert(
                    Utf8PathBuf::from("platform/build"),
                    make_project("platform/build", "platform/build"),
                );
                // Duplicate name, different path
                map.insert(
                    Utf8PathBuf::from("vendor/build"),
                    make_project("platform/build", "vendor/build"),
                );
                map
            },
            submanifests: IndexMap::new(),
        }
    }

    #[test]
    fn test_project_by_path_found() {
        let client = make_client();
        let p = client.project_by_path(&Utf8PathBuf::from("foo"));
        assert!(p.is_some());
        assert_eq!(p.unwrap().name, "foo");
    }

    #[test]
    fn test_project_by_path_not_found() {
        let client = make_client();
        let p = client.project_by_path(&Utf8PathBuf::from("missing"));
        assert!(p.is_none());
    }

    #[test]
    fn test_project_by_name_found() {
        let client = make_client();
        let p = client.project_by_name("foo");
        assert!(p.is_some());
        assert_eq!(p.unwrap().relpath, "foo");
    }

    #[test]
    fn test_project_by_name_not_found() {
        let client = make_client();
        let p = client.project_by_name("missing");
        assert!(p.is_none());
    }

    #[test]
    fn test_project_by_name_duplicate_returns_first() {
        let client = make_client();
        let p = client.project_by_name("platform/build");
        assert!(p.is_some());
        // First inserted match should be returned
        assert_eq!(p.unwrap().relpath, "platform/build");
    }

    #[test]
    fn test_find_projects_by_name_regex() {
        let client = make_client();
        let results = client.find_projects(r"^foo$", false).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "foo");
    }

    #[test]
    fn test_find_projects_by_path_regex() {
        let client = make_client();
        // Match by path only; "vendor/build" has name "platform/build" so
        // this regex would not match the name, only the path.
        let results = client.find_projects(r"^vendor/", false).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].relpath, "vendor/build");
    }

    #[test]
    fn test_find_projects_multiple_matches() {
        let client = make_client();
        let results = client.find_projects(r"build", false).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].relpath, "platform/build");
        assert_eq!(results[1].relpath, "vendor/build");
    }

    #[test]
    fn test_find_projects_inverse() {
        let client = make_client();
        let results = client.find_projects(r"^foo$", true).unwrap();
        assert_eq!(results.len(), 3);
        assert!(!results.iter().any(|p| p.name == "foo"));
    }

    #[test]
    fn test_find_projects_no_match() {
        let client = make_client();
        let results = client.find_projects(r"^missing$", false).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_find_projects_invalid_regex() {
        let client = make_client();
        let err = client.find_projects(r"[invalid", false);
        assert!(err.is_err());
    }

    #[test]
    fn test_find_projects_sorted_by_relpath() {
        let client = make_client();
        let results = client.find_projects(r".*", false).unwrap();
        let relpaths: Vec<_> = results.iter().map(|p| p.relpath.as_str()).collect();
        assert_eq!(relpaths, vec!["bar", "foo", "platform/build", "vendor/build"]);
    }
}
