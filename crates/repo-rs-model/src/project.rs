// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use std::collections::HashSet;

/// A single Git repository managed by repo.
#[derive(Debug, Clone)]
pub struct Project {
    /// The server-side project name.
    pub name: String,
    /// Path relative to the repo topdir.
    pub relpath: Utf8PathBuf,
    /// Absolute path to the worktree.
    pub worktree: Utf8PathBuf,
    /// Absolute path to the Git directory (under `.repo/projects/`).
    pub gitdir: Utf8PathBuf,
    /// Absolute path to the shared object store.
    pub objdir: Utf8PathBuf,
    /// Resolved remote specification.
    pub remote: RemoteSpec,
    /// Manifest revision expression (branch, tag, or SHA).
    pub revision_expr: String,
    /// Resolved commit SHA.
    pub revision_id: Option<String>,
    /// Group membership.
    pub groups: HashSet<String>,
    /// Parent project (for submodules / nested manifests).
    pub parent: Option<Box<Project>>,
    /// Child projects.
    pub subprojects: Vec<Project>,
    /// Copyfile directives.
    pub copyfiles: Vec<CopyFile>,
    /// Linkfile directives.
    pub linkfiles: Vec<LinkFile>,
    /// Sync current branch only.
    pub sync_c: bool,
    /// Sync submodules.
    pub sync_s: bool,
    /// Sync tags.
    pub sync_tags: bool,
    /// Clone depth (shallow clone).
    pub clone_depth: Option<u32>,
    /// Upstream branch.
    pub upstream: Option<String>,
    /// Destination branch for uploads.
    pub dest_branch: Option<String>,
}

/// Remote specification for a project.
#[derive(Debug, Clone)]
pub struct RemoteSpec {
    /// The name of this item.
    pub name: String,
    /// The resolved fetch URL.
    pub fetch_url: String,
    /// Push url.
    pub push_url: Option<String>,
    /// Review url.
    pub review_url: Option<String>,
    /// An alternative name for this item.
    pub alias: Option<String>,
}

/// A copyfile directive.
#[derive(Debug, Clone)]
pub struct CopyFile {
    /// The source path.
    pub src: Utf8PathBuf,
    /// The destination path.
    pub dest: Utf8PathBuf,
}

/// A linkfile directive.
#[derive(Debug, Clone)]
pub struct LinkFile {
    /// The source path.
    pub src: Utf8PathBuf,
    /// The destination path.
    pub dest: Utf8PathBuf,
}

impl Project {
    /// Returns `true` if this project matches all of the given groups.
    pub fn matches_groups(&self, groups: &HashSet<String>) -> bool {
        groups.is_subset(&self.groups)
    }

    /// Returns `true` if this project has any of the given groups.
    pub fn has_any_group(&self, groups: &HashSet<String>) -> bool {
        self.groups.intersection(groups).next().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_project_with_groups(groups: HashSet<String>) -> Project {
        Project {
            name: "test".to_string(),
            relpath: Utf8PathBuf::from("test"),
            worktree: Utf8PathBuf::from("/tmp/test"),
            gitdir: Utf8PathBuf::from("/tmp/.repo/projects/test.git"),
            objdir: Utf8PathBuf::from("/tmp/.repo/project-objects/test.git"),
            remote: RemoteSpec {
                name: "origin".to_string(),
                fetch_url: "https://example.com".to_string(),
                push_url: None,
                review_url: None,
                alias: None,
            },
            revision_expr: "main".to_string(),
            revision_id: None,
            groups,
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

    #[test]
    fn test_matches_groups_exact() {
        let mut groups = HashSet::new();
        groups.insert("all".to_string());
        groups.insert("test".to_string());
        let project = make_project_with_groups(groups.clone());
        assert!(project.matches_groups(&groups));
    }

    #[test]
    fn test_matches_groups_subset() {
        let mut groups = HashSet::new();
        groups.insert("all".to_string());
        groups.insert("test".to_string());
        groups.insert("extra".to_string());
        let project = make_project_with_groups(groups);

        let mut subset = HashSet::new();
        subset.insert("all".to_string());
        assert!(project.matches_groups(&subset));

        let mut subset2 = HashSet::new();
        subset2.insert("test".to_string());
        subset2.insert("all".to_string());
        assert!(project.matches_groups(&subset2));
    }

    #[test]
    fn test_matches_groups_missing() {
        let mut groups = HashSet::new();
        groups.insert("all".to_string());
        let project = make_project_with_groups(groups);

        let mut query = HashSet::new();
        query.insert("missing".to_string());
        assert!(!project.matches_groups(&query));

        let mut query2 = HashSet::new();
        query2.insert("all".to_string());
        query2.insert("missing".to_string());
        assert!(!project.matches_groups(&query2));
    }

    #[test]
    fn test_matches_groups_empty() {
        let mut groups = HashSet::new();
        groups.insert("all".to_string());
        let project = make_project_with_groups(groups);

        let empty: HashSet<String> = HashSet::new();
        assert!(project.matches_groups(&empty));
    }

    #[test]
    fn test_has_any_group_match() {
        let mut groups = HashSet::new();
        groups.insert("all".to_string());
        groups.insert("test".to_string());
        let project = make_project_with_groups(groups);

        let mut query = HashSet::new();
        query.insert("test".to_string());
        assert!(project.has_any_group(&query));

        let mut query2 = HashSet::new();
        query2.insert("other".to_string());
        query2.insert("test".to_string());
        assert!(project.has_any_group(&query2));
    }

    #[test]
    fn test_has_any_group_no_match() {
        let mut groups = HashSet::new();
        groups.insert("all".to_string());
        let project = make_project_with_groups(groups);

        let mut query = HashSet::new();
        query.insert("missing".to_string());
        assert!(!project.has_any_group(&query));
    }

    #[test]
    fn test_has_any_group_empty() {
        let mut groups = HashSet::new();
        groups.insert("all".to_string());
        let project = make_project_with_groups(groups);

        let empty: HashSet<String> = HashSet::new();
        assert!(!project.has_any_group(&empty));
    }
}
