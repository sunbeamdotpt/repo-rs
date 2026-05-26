// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{Error, Project};
use camino::Utf8PathBuf;
use indexmap::IndexMap;
use std::collections::HashSet;

/// Resolve projects by name or path.
///
/// If `args` is empty, returns all projects.
pub fn resolve_projects<'a>(
    projects: &'a IndexMap<Utf8PathBuf, Project>,
    args: &[String],
) -> Result<Vec<&'a Project>, Error> {
    if args.is_empty() {
        return Ok(projects.values().collect());
    }

    let mut result = Vec::new();
    let mut seen = HashSet::new();

    for arg in args {
        // Try path lookup first
        if let Some(p) = projects.get(&Utf8PathBuf::from(arg)) {
            if seen.insert(&p.relpath) {
                result.push(p);
            }
            continue;
        }

        // Try name lookup
        let found: Vec<&'a Project> = projects
            .values()
            .filter(|p| p.name == *arg)
            .collect();

        if found.is_empty() {
            return Err(Error::ProjectNotFound(arg.clone()));
        }

        for p in found {
            if seen.insert(&p.relpath) {
                result.push(p);
            }
        }
    }

    Ok(result)
}

/// Filter projects by group membership.
///
/// If `groups` is empty, no filtering is applied.
pub fn filter_by_groups<'a>(
    projects: &'a [&'a Project],
    groups: &HashSet<String>,
) -> Vec<&'a Project> {
    if groups.is_empty() {
        return projects.to_vec();
    }
    projects
        .iter()
        .filter(|p| p.has_any_group(groups))
        .copied()
        .collect()
}

/// Build the auto-assigned groups for a project.
///
/// Every project automatically gets `"all"`, `"name:<name>"`, and
/// `"path:<relpath>"`.
pub fn auto_groups(name: &str, relpath: &Utf8PathBuf) -> HashSet<String> {
    let mut groups = HashSet::new();
    groups.insert("all".to_string());
    groups.insert(format!("name:{name}"));
    groups.insert(format!("path:{relpath}"));
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::RemoteSpec;

    fn make_project(name: &str, relpath: &str) -> Project {
        let mut groups = auto_groups(name, &Utf8PathBuf::from(relpath));
        groups.insert("test".to_string());
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
    fn test_resolve_empty() {
        let mut projects = IndexMap::new();
        projects.insert(Utf8PathBuf::from("foo"), make_project("foo", "foo"));
        let resolved = resolve_projects(&projects, &[]).unwrap();
        assert_eq!(resolved.len(), 1);
    }

    #[test]
    fn test_resolve_by_path() {
        let mut projects = IndexMap::new();
        projects.insert(Utf8PathBuf::from("foo"), make_project("foo", "foo"));
        let resolved = resolve_projects(&projects, &["foo".to_string()]).unwrap();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].name, "foo");
    }

    #[test]
    fn test_resolve_by_name() {
        let mut projects = IndexMap::new();
        projects.insert(Utf8PathBuf::from("a/foo"), make_project("foo", "a/foo"));
        let resolved = resolve_projects(&projects, &["foo".to_string()]).unwrap();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].relpath, "a/foo");
    }

    #[test]
    fn test_resolve_missing() {
        let projects = IndexMap::new();
        assert!(resolve_projects(&projects, &["missing".to_string()]).is_err());
    }

    #[test]
    fn test_filter_groups() {
        let p1 = make_project("foo", "foo");
        let p2 = make_project("bar", "bar");
        let all = vec![&p1, &p2];
        let filtered = filter_by_groups(&all, &HashSet::new());
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_groups_match() {
        let mut p1 = make_project("foo", "foo");
        p1.groups.insert("special".to_string());
        let p2 = make_project("bar", "bar");
        let all = vec![&p1, &p2];

        let mut groups = HashSet::new();
        groups.insert("special".to_string());
        let filtered = filter_by_groups(&all, &groups);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "foo");
    }

    #[test]
    fn test_filter_groups_no_match() {
        let p1 = make_project("foo", "foo");
        let p2 = make_project("bar", "bar");
        let all = vec![&p1, &p2];

        let mut groups = HashSet::new();
        groups.insert("missing".to_string());
        let filtered = filter_by_groups(&all, &groups);
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_resolve_duplicate_name() {
        let mut projects = IndexMap::new();
        projects.insert(
            Utf8PathBuf::from("a/foo"),
            make_project("foo", "a/foo"),
        );
        projects.insert(
            Utf8PathBuf::from("b/foo"),
            make_project("foo", "b/foo"),
        );
        let resolved = resolve_projects(&projects, &["foo".to_string()]).unwrap();
        assert_eq!(resolved.len(), 2);
        let relpaths: Vec<_> = resolved.iter().map(|p| p.relpath.as_str()).collect();
        assert!(relpaths.contains(&"a/foo"));
        assert!(relpaths.contains(&"b/foo"));
    }

    #[test]
    fn test_resolve_path_precedence_over_name() {
        let mut projects = IndexMap::new();
        // Project with name "foo" at path "bar"
        projects.insert(
            Utf8PathBuf::from("bar"),
            make_project("foo", "bar"),
        );
        // Also a project with name "bar" at path "foo"
        projects.insert(
            Utf8PathBuf::from("foo"),
            make_project("bar", "foo"),
        );
        // When we query "foo", path lookup should win
        let resolved = resolve_projects(&projects, &["foo".to_string()]).unwrap();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].name, "bar"); // path "foo" has name "bar"
    }

    #[test]
    fn test_resolve_deduplication() {
        let mut projects = IndexMap::new();
        projects.insert(
            Utf8PathBuf::from("foo"),
            make_project("foo", "foo"),
        );
        // Querying by both path and name for the same project
        let resolved = resolve_projects(
            &projects,
            &["foo".to_string(), "foo".to_string()],
        )
        .unwrap();
        assert_eq!(resolved.len(), 1);
    }

    #[test]
    fn test_resolve_mixed_args() {
        let mut projects = IndexMap::new();
        projects.insert(
            Utf8PathBuf::from("path1"),
            make_project("name1", "path1"),
        );
        projects.insert(
            Utf8PathBuf::from("path2"),
            make_project("name2", "path2"),
        );
        let resolved = resolve_projects(
            &projects,
            &["path1".to_string(), "name2".to_string()],
        )
        .unwrap();
        assert_eq!(resolved.len(), 2);
    }

    #[test]
    fn test_auto_groups() {
        let groups = auto_groups("my-project", &Utf8PathBuf::from("path/to/project"));
        assert!(groups.contains("all"));
        assert!(groups.contains("name:my-project"));
        assert!(groups.contains("path:path/to/project"));
        assert_eq!(groups.len(), 3);
    }

    #[test]
    fn test_auto_groups_simple() {
        let groups = auto_groups("foo", &Utf8PathBuf::from("foo"));
        assert!(groups.contains("all"));
        assert!(groups.contains("name:foo"));
        assert!(groups.contains("path:foo"));
    }
}
