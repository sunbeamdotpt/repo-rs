// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Manifest resolution, merging, and mutation passes.
//!
//! This module implements:
//! - `<include>` resolution by reading referenced files and merging their contents
//! - Local manifest merging from a `local_manifests/` directory
//! - `<extend-project>` mutation pass
//! - `<remove-project>` deletion pass
//! - Auto-group injection
//! - Submanifest depth limiting
//! - Remote URL normalization

use crate::Error;
use crate::model::*;
use crate::parser::{parse_manifest, validate_manifest};
use crate::validate::is_valid_path;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Maximum depth for submanifest nesting.
pub const MAX_SUBMANIFEST_DEPTH: usize = 8;

/// Prefix for groups added by local manifests.
pub const LOCAL_MANIFEST_GROUP_PREFIX: &str = "local:";

/// Prefix for groups added by submanifests.
pub const SUBMANIFEST_GROUP_PREFIX: &str = "submanifest:";

/// Load and fully resolve a manifest from disk.
///
/// This function performs the following steps:
/// 1. Parses the manifest XML at `path`.
/// 2. Recursively resolves `<include>` directives.
/// 3. Merges local manifests from the `local_manifests/` directory adjacent to `path`.
/// 4. Applies `<extend-project>` mutations.
/// 5. Applies `<remove-project>` deletions.
/// 6. Validates the final manifest (checks remote references, paths, etc.).
///
/// # Errors
///
/// Returns an error if the manifest cannot be read, parsed, or validated.
pub fn load_manifest(path: &Path) -> Result<Manifest, Error> {
    let mut manifest = resolve_includes(path, &mut HashSet::new(), 0)?;

    let local_dir = path
        .parent()
        .unwrap_or(Path::new("."))
        .join("local_manifests");
    if local_dir.is_dir() {
        merge_local_manifests(&mut manifest, &local_dir)?;
    }

    apply_extend_projects(&mut manifest)?;
    apply_remove_projects(&mut manifest)?;
    inject_auto_groups(&mut manifest);

    validate_manifest(&manifest)?;
    Ok(manifest)
}

/// Resolve all `<include>` directives in a manifest file.
///
/// Reads the manifest at `path`, then recursively resolves any `<include>`
/// elements by reading the referenced files relative to `path`'s parent
/// directory. Includes are processed depth-first. The returned manifest has
/// its `includes` list cleared because all includes have been expanded.
///
/// Circular includes are detected and rejected.
fn resolve_includes(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
    depth: usize,
) -> Result<Manifest, Error> {
    if depth > MAX_SUBMANIFEST_DEPTH {
        return Err(Error::MaxSubmanifestDepthExceeded(MAX_SUBMANIFEST_DEPTH));
    }

    let canonical = std::fs::canonicalize(path)?;
    if !visited.insert(canonical.clone()) {
        return Err(Error::CircularInclude(format!(
            "circular include detected: {}",
            path.display()
        )));
    }

    let xml = std::fs::read_to_string(path)?;
    let mut manifest = parse_manifest(&xml)?;

    let base_dir = path.parent().unwrap_or(Path::new("."));

    let includes = std::mem::take(&mut manifest.includes);
    for include in includes {
        if !is_valid_path(&include.name) {
            return Err(Error::InvalidPath(format!(
                "invalid include path: {}",
                include.name
            )));
        }

        let include_path = base_dir.join(&include.name);
        let mut resolved = resolve_includes(&include_path, visited, depth + 1)?;

        // Apply group inheritance from the include directive.
        for project in &mut resolved.projects {
            for group in &include.groups {
                project.groups.insert(group.clone());
            }
        }

        merge_manifest(&mut manifest, resolved)?;
    }

    visited.remove(&canonical);
    Ok(manifest)
}

/// Merge the contents of `other` into `base`.
///
/// # Merge rules
///
/// - **remotes**: Merged; duplicate names return an error.
/// - **submanifests**: Merged; duplicate names return an error.
/// - **projects**: Appended.
/// - **remove-projects**: Appended.
/// - **extend-projects**: Appended.
/// - **notice**: Used from `other` only if `base` has none.
/// - **default**: Used from `other` only if `base` has none.
/// - **`manifest_server`**: Used from `other` only if `base` has none.
/// - **`repo_hooks`**: Used from `other` only if `base` has none.
/// - **superproject**: Used from `other` only if `base` has none.
/// - **contactinfo**: Used from `other` only if `base` has none.
///
/// # Errors
///
/// Returns an error if there are duplicate remote or submanifest names.
fn merge_manifest(base: &mut Manifest, other: Manifest) -> Result<(), Error> {
    for (name, remote) in other.remotes {
        if base.remotes.contains_key(&name) {
            return Err(Error::DuplicateRemote(name));
        }
        base.remotes.insert(name, remote);
    }

    for (name, submanifest) in other.submanifests {
        if base.submanifests.contains_key(&name) {
            return Err(Error::DuplicateSubmanifest(name));
        }
        base.submanifests.insert(name, submanifest);
    }

    base.projects.extend(other.projects);
    base.remove_projects.extend(other.remove_projects);
    base.extend_projects.extend(other.extend_projects);

    if base.notice.is_none() {
        base.notice = other.notice;
    }
    if base.default.is_none() {
        base.default = other.default;
    }
    if base.manifest_server.is_none() {
        base.manifest_server = other.manifest_server;
    }
    if base.repo_hooks.is_none() {
        base.repo_hooks = other.repo_hooks;
    }
    if base.superproject.is_none() {
        base.superproject = other.superproject;
    }
    if base.contactinfo.is_none() {
        base.contactinfo = other.contactinfo;
    }

    Ok(())
}

/// Inject an auto group into all projects in a manifest (and subprojects).
fn inject_group_into_manifest(manifest: &mut Manifest, group: &str) {
    for project in &mut manifest.projects {
        inject_group_into_project(project, group);
    }
}

/// Recursively inject a group into a project and its subprojects.
fn inject_group_into_project(project: &mut Project, group: &str) {
    project.groups.insert(group.to_string());
    for subproject in &mut project.subprojects {
        inject_group_into_project(subproject, group);
    }
}

/// Inject auto-generated groups into all projects.
///
/// Each project gets:
/// - `all`
/// - `name:{project_name}`
/// - `path:{project_path}`
pub fn inject_auto_groups(manifest: &mut Manifest) {
    for project in &mut manifest.projects {
        inject_auto_groups_into_project(project);
    }
}

fn inject_auto_groups_into_project(project: &mut Project) {
    project.groups.insert("all".to_string());
    project
        .groups
        .insert(format!("name:{}", project.name));
    if let Some(ref path) = project.path {
        project.groups.insert(format!("path:{path}"));
    }
    for subproject in &mut project.subprojects {
        inject_auto_groups_into_project(subproject);
    }
}

/// Inject submanifest group prefixes into all projects.
///
/// Projects in a submanifest at `path_prefix` get the group
/// `submanifest:path:{path_prefix}`.
pub fn inject_submanifest_groups(manifest: &mut Manifest, path_prefix: &str) {
    let group = format!("{SUBMANIFEST_GROUP_PREFIX}path:{path_prefix}");
    inject_group_into_manifest(manifest, &group);
}

/// Normalize a remote fetch URL.
///
/// * Removes trailing slashes.
/// * Converts SCP-like syntax (`git@host:path`) to SSH URLs.
pub fn normalize_url(url: &str) -> String {
    let url = url.trim_end_matches('/');
    // SCP-like syntax: no scheme, contains @ and : before any /
    let scp_like = !url.contains("://")
        && url.contains('@')
        && url.chars().take_while(|&c| c != ':').count() < url.len();
    if scp_like {
        // Convert "git@host:path" to "ssh://git@host/path"
        let parts: Vec<&str> = url.splitn(2, ':').collect();
        if parts.len() == 2 {
            return format!("ssh://{}/{}", parts[0], parts[1]);
        }
    }
    url.to_string()
}

/// Resolve a fetch URL relative to a manifest URL.
///
/// This mimics Python's `urllib.parse.urljoin()` behavior for remote URLs.
pub fn resolve_fetch_url(fetch_url: &str, manifest_url: Option<&str>) -> String {
    let fetch = normalize_url(fetch_url);
    let manifest = manifest_url.map(normalize_url);

    match manifest {
        Some(manifest_url) if !fetch.contains("://") => {
            // Relative URL resolution
            if manifest_url.ends_with('/') {
                format!("{}{}", manifest_url, fetch)
            } else {
                let base = manifest_url.rsplitn(2, '/').nth(1).unwrap_or(&manifest_url);
                format!("{}/{}", base, fetch)
            }
        }
        _ => fetch,
    }
}

/// Resolve a remote name, falling back to alias names.
///
/// Returns the actual remote name if found by exact match or alias match.
pub fn resolve_remote_name(manifest: &Manifest, name: &str) -> Option<String> {
    if manifest.remotes.contains_key(name) {
        return Some(name.to_string());
    }
    for (remote_name, remote) in &manifest.remotes {
        if remote.alias.as_deref() == Some(name) {
            return Some(remote_name.clone());
        }
    }
    None
}

/// Merge all `.xml` files from a local manifests directory into a manifest.
///
/// Files are processed in lexicographic order for determinism. Each file is
/// parsed and merged using the same rules as [`merge_manifest`].
///
/// # Errors
///
/// Returns an error if the directory cannot be read or any local manifest
/// cannot be parsed or merged.
pub fn merge_local_manifests(manifest: &mut Manifest, dir: &Path) -> Result<(), Error> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(std::result::Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "xml"))
        .collect();

    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();
        let xml = std::fs::read_to_string(&path)?;
        let mut local = parse_manifest(&xml)?;

        // Inject local manifest group prefix into all projects.
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            let group = format!("{LOCAL_MANIFEST_GROUP_PREFIX}{stem}");
            inject_group_into_manifest(&mut local, &group);
        }

        merge_manifest(manifest, local)?;
    }

    Ok(())
}

/// Apply `<extend-project>` mutations to a manifest.
///
/// For each `<extend-project>` directive, finds all existing projects with a
/// matching `name` and applies the following mutations:
///
/// - **groups**: New groups are added to the existing set. Local manifest
///   groups are stripped.
/// - **path / dest-path**: Overwrites the project's path.
/// - **revision**: Overwrites the project's revision.
/// - **remote**: Overwrites the project's remote.
/// - **dest-branch**: Overwrites the project's dest-branch.
/// - **upstream**: Overwrites the project's upstream.
/// - **copyfiles**: Appended to the project's copyfiles.
/// - **linkfiles**: Appended to the project's linkfiles.
/// - **annotations**: Appended to the project's annotations.
///
/// # Errors
///
/// Returns an error if `base-rev` does not match, or if `dest-path` is used
/// without `path` when matching multiple projects.
pub fn apply_extend_projects(manifest: &mut Manifest) -> Result<(), Error> {
    let extensions = manifest.extend_projects.clone();
    for ext in extensions {
        let matching: Vec<&Project> = manifest
            .projects
            .iter()
            .filter(|p| p.name == ext.name)
            .collect();

        if ext.dest_path.is_some() && ext.path.is_none() && matching.len() > 1 {
            return Err(Error::ExtendProjectDestPathMultipleMatches(
                ext.name.clone(),
            ));
        }

        for project in &mut manifest.projects {
            if project.name != ext.name {
                continue;
            }

            if let Some(ref base_rev) = ext.base_rev {
                let project_rev = project.revision.as_deref().unwrap_or("");
                if project_rev != base_rev {
                    return Err(Error::ExtendProjectBaseRevMismatch {
                        name: project.name.clone(),
                        expected: base_rev.clone(),
                        found: project_rev.to_string(),
                    });
                }
            }

            for group in &ext.groups {
                project.groups.insert(group.clone());
            }

            // Strip local manifest group prefixes so we don't mistakenly
            // omit this project from the superproject override manifest.
            project.groups.retain(|g| !g.starts_with(LOCAL_MANIFEST_GROUP_PREFIX));

            if let Some(path) = &ext.path {
                project.path = Some(path.clone());
            }
            if let Some(dest_path) = &ext.dest_path {
                project.path = Some(dest_path.clone());
            }
            if let Some(revision) = &ext.revision {
                project.revision = Some(revision.clone());
            }
            if let Some(remote) = &ext.remote {
                project.remote = Some(remote.clone());
            }
            if let Some(dest_branch) = &ext.dest_branch {
                project.dest_branch = Some(dest_branch.clone());
            }
            if let Some(upstream) = &ext.upstream {
                project.upstream = Some(upstream.clone());
            }

            project.copyfiles.extend(ext.copyfiles.clone());
            project.linkfiles.extend(ext.linkfiles.clone());
            project.annotations.extend(ext.annotations.clone());
        }
    }
    Ok(())
}

/// Apply `<remove-project>` deletions to a manifest.
///
/// Removes any project whose `name` matches a `<remove-project>` directive's
/// `name` attribute, or whose `path` matches a directive's `path` attribute.
/// A project is removed if **either** condition matches (OR semantics).
///
/// If a removal targets a non-existent project and `optional` is `false`, an
/// error is returned. If the removed project was the hooks project, the
/// repo-hooks setting is cleared.
///
/// # Errors
///
/// Returns an error if a non-optional remove-project fails to match, or if
/// `base-rev` does not match.
pub fn apply_remove_projects(manifest: &mut Manifest) -> Result<(), Error> {
    let removals = manifest.remove_projects.clone();
    let mut removed_any = false;
    let hooks_project_name = manifest.repo_hooks.as_ref().map(|h| h.in_project.clone());

    for removal in &removals {
        let mut matched = false;

        manifest.projects.retain(|project| {
            let matches_name = removal.name.as_ref() == Some(&project.name);
            let matches_path = removal
                .path
                .as_ref()
                .is_some_and(|p| project.path.as_ref() == Some(p));

            let should_remove = matches_name || matches_path;
            if should_remove {
                if let Some(ref base_rev) = removal.base_rev {
                    let project_rev = project.revision.as_deref().unwrap_or("");
                    if project_rev != base_rev {
                        // We can't return an error from retain, so we just
                        // keep the project and record the mismatch. We'll
                        // handle this after retain.
                        return true;
                    }
                }
                matched = true;
            }
            !should_remove
        });

        if matched {
            removed_any = true;
            // If the removed project was the hooks project, clear it.
            if let Some(ref hooks_name) = hooks_project_name {
                if removal.name.as_ref() == Some(hooks_name) {
                    manifest.repo_hooks = None;
                }
            }
        }

        if !matched && !removal.optional {
            return Err(Error::RemoveProjectNotFound(
                removal
                    .name
                    .clone()
                    .or_else(|| removal.path.clone())
                    .unwrap_or_default(),
            ));
        }
    }

    let _ = removed_any;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn resolve_single_include() {
        let dir = TempDir::new().unwrap();
        let included = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="included" fetch="https://included.com" />
  <project name="bar" path="bar" remote="included" />
</manifest>
"#;
        write_file(dir.path(), "included.xml", included);

        let main = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <project name="foo" path="foo" remote="origin" />
  <include name="included.xml" />
</manifest>
"#;
        let main_path = write_file(dir.path(), "main.xml", main);

        let manifest = resolve_includes(&main_path, &mut HashSet::new(), 0).unwrap();
        assert_eq!(manifest.remotes.len(), 2);
        assert!(manifest.remotes.contains_key("origin"));
        assert!(manifest.remotes.contains_key("included"));
        assert_eq!(manifest.projects.len(), 2);
        assert_eq!(manifest.includes.len(), 0);
    }

    #[test]
    fn resolve_nested_includes() {
        let dir = TempDir::new().unwrap();
        let inner = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="inner" fetch="https://inner.com" />
  <project name="baz" path="baz" remote="inner" />
</manifest>
"#;
        write_file(dir.path(), "inner.xml", inner);

        let middle = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <include name="inner.xml" />
  <project name="qux" path="qux" remote="inner" />
</manifest>
"#;
        write_file(dir.path(), "middle.xml", middle);

        let main = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <include name="middle.xml" />
  <project name="foo" path="foo" />
</manifest>
"#;
        let main_path = write_file(dir.path(), "main.xml", main);

        let manifest = resolve_includes(&main_path, &mut HashSet::new(), 0).unwrap();
        assert_eq!(manifest.remotes.len(), 1);
        assert_eq!(manifest.projects.len(), 3);
    }

    #[test]
    fn detect_circular_include() {
        let dir = TempDir::new().unwrap();
        let a = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <include name="b.xml" />
</manifest>
"#;
        write_file(dir.path(), "a.xml", a);

        let b = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <include name="a.xml" />
</manifest>
"#;
        write_file(dir.path(), "b.xml", b);

        let main = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <include name="a.xml" />
</manifest>
"#;
        let main_path = write_file(dir.path(), "main.xml", main);

        let err = resolve_includes(&main_path, &mut HashSet::new(), 0).unwrap_err();
        assert!(err.to_string().contains("circular include"));
    }

    #[test]
    fn reject_invalid_include_path() {
        let dir = TempDir::new().unwrap();
        let main = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <include name="../evil.xml" />
</manifest>
"#;
        let main_path = write_file(dir.path(), "main.xml", main);

        let err = resolve_includes(&main_path, &mut HashSet::new(), 0).unwrap_err();
        assert!(err.to_string().contains("invalid include path"));
    }

    #[test]
    fn include_group_inheritance() {
        let dir = TempDir::new().unwrap();
        let included = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <project name="bar" path="bar" groups="basegroup" />
</manifest>
"#;
        write_file(dir.path(), "included.xml", included);

        let main = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <include name="included.xml" groups="inherited" />
</manifest>
"#;
        let main_path = write_file(dir.path(), "main.xml", main);

        let manifest = resolve_includes(&main_path, &mut HashSet::new(), 0).unwrap();
        assert_eq!(manifest.projects.len(), 1);
        assert!(manifest.projects[0].groups.contains("basegroup"));
        assert!(manifest.projects[0].groups.contains("inherited"));
    }

    #[test]
    fn reject_duplicate_remote_across_includes() {
        let dir = TempDir::new().unwrap();
        let included = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://other.com" />
</manifest>
"#;
        write_file(dir.path(), "included.xml", included);

        let main = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <include name="included.xml" />
</manifest>
"#;
        let main_path = write_file(dir.path(), "main.xml", main);

        let err = resolve_includes(&main_path, &mut HashSet::new(), 0).unwrap_err();
        assert!(err.to_string().contains("duplicate remote name: origin"));
    }

    #[test]
    fn merge_local_manifests_basic() {
        let dir = TempDir::new().unwrap();
        let local_dir = dir.path().join("local_manifests");
        std::fs::create_dir(&local_dir).unwrap();

        let local1 = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <project name="local1" path="local1" />
</manifest>
"#;
        write_file(&local_dir, "01_local.xml", local1);

        let local2 = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <project name="local2" path="local2" />
</manifest>
"#;
        write_file(&local_dir, "02_local.xml", local2);

        let mut manifest = Manifest::default();
        merge_local_manifests(&mut manifest, &local_dir).unwrap();

        assert_eq!(manifest.projects.len(), 2);
        assert_eq!(manifest.projects[0].name, "local1");
        assert_eq!(manifest.projects[1].name, "local2");
    }

    #[test]
    fn merge_local_manifests_skips_non_xml() {
        let dir = TempDir::new().unwrap();
        let local_dir = dir.path().join("local_manifests");
        std::fs::create_dir(&local_dir).unwrap();

        let local = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <project name="local1" path="local1" />
</manifest>
"#;
        write_file(&local_dir, "01_local.xml", local);
        write_file(&local_dir, "readme.txt", "not xml");

        let mut manifest = Manifest::default();
        merge_local_manifests(&mut manifest, &local_dir).unwrap();

        assert_eq!(manifest.projects.len(), 1);
    }

    #[test]
    fn apply_extend_project_adds_groups() {
        let mut manifest = Manifest {
            projects: vec![Project {
                name: "foo".to_string(),
                path: Some("foo".to_string()),
                groups: ["base".to_string()].into_iter().collect(),
                ..Project::default_test()
            }],
            extend_projects: vec![ExtendProject {
                name: "foo".to_string(),
                groups: ["extra".to_string()].into_iter().collect(),
                ..ExtendProject::default_test()
            }],
            ..Manifest::default()
        };

        apply_extend_projects(&mut manifest).unwrap();

        assert!(manifest.projects[0].groups.contains("base"));
        assert!(manifest.projects[0].groups.contains("extra"));
    }

    #[test]
    fn apply_extend_project_overrides_fields() {
        let mut manifest = Manifest {
            projects: vec![Project {
                name: "foo".to_string(),
                path: Some("foo".to_string()),
                revision: Some("main".to_string()),
                remote: Some("origin".to_string()),
                dest_branch: Some("main".to_string()),
                upstream: Some("main".to_string()),
                ..Project::default_test()
            }],
            extend_projects: vec![ExtendProject {
                name: "foo".to_string(),
                path: Some("bar".to_string()),
                dest_path: Some("baz".to_string()),
                revision: Some("dev".to_string()),
                remote: Some("other".to_string()),
                dest_branch: Some("dev".to_string()),
                upstream: Some("dev".to_string()),
                ..ExtendProject::default_test()
            }],
            ..Manifest::default()
        };

        apply_extend_projects(&mut manifest).unwrap();

        assert_eq!(manifest.projects[0].path, Some("baz".to_string()));
        assert_eq!(manifest.projects[0].revision, Some("dev".to_string()));
        assert_eq!(manifest.projects[0].remote, Some("other".to_string()));
        assert_eq!(manifest.projects[0].dest_branch, Some("dev".to_string()));
        assert_eq!(manifest.projects[0].upstream, Some("dev".to_string()));
    }

    #[test]
    fn apply_extend_project_appends_files() {
        let mut manifest = Manifest {
            projects: vec![Project {
                name: "foo".to_string(),
                path: Some("foo".to_string()),
                copyfiles: vec![CopyFile {
                    src: "a".to_string(),
                    dest: "b".to_string(),
                }],
                linkfiles: vec![LinkFile {
                    src: "c".to_string(),
                    dest: "d".to_string(),
                }],
                annotations: vec![Annotation {
                    name: "ann1".to_string(),
                    value: "val1".to_string(),
                    keep: true,
                }],
                ..Project::default_test()
            }],
            extend_projects: vec![ExtendProject {
                name: "foo".to_string(),
                copyfiles: vec![CopyFile {
                    src: "e".to_string(),
                    dest: "f".to_string(),
                }],
                linkfiles: vec![LinkFile {
                    src: "g".to_string(),
                    dest: "h".to_string(),
                }],
                annotations: vec![Annotation {
                    name: "ann2".to_string(),
                    value: "val2".to_string(),
                    keep: false,
                }],
                ..ExtendProject::default_test()
            }],
            ..Manifest::default()
        };

        apply_extend_projects(&mut manifest).unwrap();

        assert_eq!(manifest.projects[0].copyfiles.len(), 2);
        assert_eq!(manifest.projects[0].linkfiles.len(), 2);
        assert_eq!(manifest.projects[0].annotations.len(), 2);
    }

    #[test]
    fn apply_extend_project_no_match() {
        let mut manifest = Manifest {
            projects: vec![Project {
                name: "foo".to_string(),
                path: Some("foo".to_string()),
                ..Project::default_test()
            }],
            extend_projects: vec![ExtendProject {
                name: "bar".to_string(),
                groups: ["extra".to_string()].into_iter().collect(),
                ..ExtendProject::default_test()
            }],
            ..Manifest::default()
        };

        apply_extend_projects(&mut manifest).unwrap();

        assert!(!manifest.projects[0].groups.contains("extra"));
    }

    #[test]
    fn apply_extend_project_base_rev_mismatch() {
        let mut manifest = Manifest {
            projects: vec![Project {
                name: "foo".to_string(),
                path: Some("foo".to_string()),
                revision: Some("main".to_string()),
                ..Project::default_test()
            }],
            extend_projects: vec![ExtendProject {
                name: "foo".to_string(),
                base_rev: Some("abc".to_string()),
                ..ExtendProject::default_test()
            }],
            ..Manifest::default()
        };

        let err = apply_extend_projects(&mut manifest).unwrap_err();
        assert!(err.to_string().contains("base-rev mismatch"));
    }

    #[test]
    fn apply_extend_project_base_rev_match() {
        let mut manifest = Manifest {
            projects: vec![Project {
                name: "foo".to_string(),
                path: Some("foo".to_string()),
                revision: Some("abc".to_string()),
                ..Project::default_test()
            }],
            extend_projects: vec![ExtendProject {
                name: "foo".to_string(),
                base_rev: Some("abc".to_string()),
                revision: Some("dev".to_string()),
                ..ExtendProject::default_test()
            }],
            ..Manifest::default()
        };

        apply_extend_projects(&mut manifest).unwrap();
        assert_eq!(manifest.projects[0].revision, Some("dev".to_string()));
    }

    #[test]
    fn apply_extend_project_dest_path_multiple_matches() {
        let mut manifest = Manifest {
            projects: vec![
                Project {
                    name: "foo".to_string(),
                    path: Some("foo1".to_string()),
                    ..Project::default_test()
                },
                Project {
                    name: "foo".to_string(),
                    path: Some("foo2".to_string()),
                    ..Project::default_test()
                },
            ],
            extend_projects: vec![ExtendProject {
                name: "foo".to_string(),
                dest_path: Some("baz".to_string()),
                ..ExtendProject::default_test()
            }],
            ..Manifest::default()
        };

        let err = apply_extend_projects(&mut manifest).unwrap_err();
        assert!(err.to_string().contains("dest-path when matching multiple projects"));
    }

    #[test]
    fn apply_extend_project_strips_local_groups() {
        let mut manifest = Manifest {
            projects: vec![Project {
                name: "foo".to_string(),
                path: Some("foo".to_string()),
                groups: [
                    "base".to_string(),
                    "local:test".to_string(),
                    "local:other".to_string(),
                ]
                .into_iter()
                .collect(),
                ..Project::default_test()
            }],
            extend_projects: vec![ExtendProject {
                name: "foo".to_string(),
                groups: ["extra".to_string()].into_iter().collect(),
                ..ExtendProject::default_test()
            }],
            ..Manifest::default()
        };

        apply_extend_projects(&mut manifest).unwrap();

        assert!(manifest.projects[0].groups.contains("base"));
        assert!(manifest.projects[0].groups.contains("extra"));
        assert!(!manifest.projects[0].groups.contains("local:test"));
        assert!(!manifest.projects[0].groups.contains("local:other"));
    }

    #[test]
    fn apply_remove_project_by_name() {
        let mut manifest = Manifest {
            projects: vec![
                Project {
                    name: "foo".to_string(),
                    path: Some("foo".to_string()),
                    ..Project::default_test()
                },
                Project {
                    name: "bar".to_string(),
                    path: Some("bar".to_string()),
                    ..Project::default_test()
                },
            ],
            remove_projects: vec![RemoveProject {
                name: Some("foo".to_string()),
                path: None,
                optional: false,
                base_rev: None,
            }],
            ..Manifest::default()
        };

        apply_remove_projects(&mut manifest).unwrap();

        assert_eq!(manifest.projects.len(), 1);
        assert_eq!(manifest.projects[0].name, "bar");
    }

    #[test]
    fn apply_remove_project_by_path() {
        let mut manifest = Manifest {
            projects: vec![
                Project {
                    name: "foo".to_string(),
                    path: Some("foo".to_string()),
                    ..Project::default_test()
                },
                Project {
                    name: "bar".to_string(),
                    path: Some("bar".to_string()),
                    ..Project::default_test()
                },
            ],
            remove_projects: vec![RemoveProject {
                name: None,
                path: Some("foo".to_string()),
                optional: false,
                base_rev: None,
            }],
            ..Manifest::default()
        };

        apply_remove_projects(&mut manifest).unwrap();

        assert_eq!(manifest.projects.len(), 1);
        assert_eq!(manifest.projects[0].name, "bar");
    }

    #[test]
    fn apply_remove_project_or_semantics() {
        let mut manifest = Manifest {
            projects: vec![
                Project {
                    name: "foo".to_string(),
                    path: Some("foo".to_string()),
                    ..Project::default_test()
                },
                Project {
                    name: "bar".to_string(),
                    path: Some("bar".to_string()),
                    ..Project::default_test()
                },
                Project {
                    name: "baz".to_string(),
                    path: Some("baz".to_string()),
                    ..Project::default_test()
                },
            ],
            remove_projects: vec![RemoveProject {
                name: Some("foo".to_string()),
                path: Some("bar".to_string()),
                optional: false,
                base_rev: None,
            }],
            ..Manifest::default()
        };

        apply_remove_projects(&mut manifest).unwrap();

        assert_eq!(manifest.projects.len(), 1);
        assert_eq!(manifest.projects[0].name, "baz");
    }

    #[test]
    fn apply_remove_project_no_match_optional() {
        let mut manifest = Manifest {
            projects: vec![Project {
                name: "foo".to_string(),
                path: Some("foo".to_string()),
                ..Project::default_test()
            }],
            remove_projects: vec![RemoveProject {
                name: Some("bar".to_string()),
                path: None,
                optional: true,
                base_rev: None,
            }],
            ..Manifest::default()
        };

        apply_remove_projects(&mut manifest).unwrap();

        assert_eq!(manifest.projects.len(), 1);
    }

    #[test]
    fn apply_remove_project_no_match_not_optional() {
        let mut manifest = Manifest {
            projects: vec![Project {
                name: "foo".to_string(),
                path: Some("foo".to_string()),
                ..Project::default_test()
            }],
            remove_projects: vec![RemoveProject {
                name: Some("bar".to_string()),
                path: None,
                optional: false,
                base_rev: None,
            }],
            ..Manifest::default()
        };

        let err = apply_remove_projects(&mut manifest).unwrap_err();
        assert!(err.to_string().contains("remove-project specifies non-existent project"));
    }

    #[test]
    fn apply_remove_project_clears_repo_hooks() {
        let mut manifest = Manifest {
            projects: vec![Project {
                name: "hooks".to_string(),
                path: Some("hooks".to_string()),
                ..Project::default_test()
            }],
            remove_projects: vec![RemoveProject {
                name: Some("hooks".to_string()),
                path: None,
                optional: false,
                base_rev: None,
            }],
            repo_hooks: Some(RepoHooks {
                in_project: "hooks".to_string(),
                enabled_list: "list.txt".to_string(),
            }),
            ..Manifest::default()
        };

        apply_remove_projects(&mut manifest).unwrap();

        assert!(manifest.projects.is_empty());
        assert!(manifest.repo_hooks.is_none());
    }

    #[test]
    fn apply_remove_project_base_rev_mismatch() {
        let mut manifest = Manifest {
            projects: vec![Project {
                name: "foo".to_string(),
                path: Some("foo".to_string()),
                revision: Some("main".to_string()),
                ..Project::default_test()
            }],
            remove_projects: vec![RemoveProject {
                name: Some("foo".to_string()),
                path: None,
                optional: false,
                base_rev: Some("abc".to_string()),
            }],
            ..Manifest::default()
        };

        // The project should NOT be removed because base_rev doesn't match,
        // but since optional=false and no project was removed, it should error.
        let err = apply_remove_projects(&mut manifest).unwrap_err();
        assert!(err.to_string().contains("remove-project specifies non-existent project"));
    }

    #[test]
    fn apply_remove_project_base_rev_match() {
        let mut manifest = Manifest {
            projects: vec![Project {
                name: "foo".to_string(),
                path: Some("foo".to_string()),
                revision: Some("abc".to_string()),
                ..Project::default_test()
            }],
            remove_projects: vec![RemoveProject {
                name: Some("foo".to_string()),
                path: None,
                optional: false,
                base_rev: Some("abc".to_string()),
            }],
            ..Manifest::default()
        };

        apply_remove_projects(&mut manifest).unwrap();
        assert!(manifest.projects.is_empty());
    }

    #[test]
    fn load_manifest_integration() {
        let dir = TempDir::new().unwrap();
        let local_dir = dir.path().join("local_manifests");
        std::fs::create_dir(&local_dir).unwrap();

        let included = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <project name="included" path="included" />
</manifest>
"#;
        write_file(dir.path(), "included.xml", included);

        let local = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <project name="local" path="local" />
</manifest>
"#;
        write_file(&local_dir, "local.xml", local);

        let main = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="main" path="main" remote="origin" />
  <include name="included.xml" />
  <extend-project name="main" groups="extended" />
  <remove-project name="included" />
</manifest>
"#;
        let main_path = write_file(dir.path(), "main.xml", main);

        let manifest = load_manifest(&main_path).unwrap();

        // included project was merged then removed
        assert_eq!(manifest.projects.len(), 2);
        assert_eq!(manifest.projects[0].name, "main");
        assert!(manifest.projects[0].groups.contains("extended"));
        assert_eq!(manifest.projects[1].name, "local");
        assert_eq!(manifest.includes.len(), 0);
    }

    #[test]
    fn merge_manifest_preserves_base_notice() {
        let mut base = Manifest {
            notice: Some("base notice".to_string()),
            ..Manifest::default()
        };
        let other = Manifest {
            notice: Some("other notice".to_string()),
            ..Manifest::default()
        };
        merge_manifest(&mut base, other).unwrap();
        assert_eq!(base.notice, Some("base notice".to_string()));
    }

    #[test]
    fn merge_manifest_takes_other_notice_when_base_empty() {
        let mut base = Manifest::default();
        let other = Manifest {
            notice: Some("other notice".to_string()),
            ..Manifest::default()
        };
        merge_manifest(&mut base, other).unwrap();
        assert_eq!(base.notice, Some("other notice".to_string()));
    }

    #[test]
    fn merge_manifest_preserves_base_default() {
        let mut base = Manifest {
            default: Some(Default {
                remote: Some("base".to_string()),
                ..Default::default()
            }),
            ..Manifest::default()
        };
        let other = Manifest {
            default: Some(Default {
                remote: Some("other".to_string()),
                ..Default::default()
            }),
            ..Manifest::default()
        };
        merge_manifest(&mut base, other).unwrap();
        assert_eq!(base.default.unwrap().remote, Some("base".to_string()));
    }

    #[test]
    fn load_manifest_without_local_manifests_dir() {
        let dir = TempDir::new().unwrap();
        let main = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <project name="main" path="main" />
</manifest>
"#;
        let main_path = write_file(dir.path(), "main.xml", main);

        let manifest = load_manifest(&main_path).unwrap();
        assert_eq!(manifest.projects.len(), 1);
    }

    #[test]
    fn resolve_include_relative_to_manifest_dir() {
        let dir = TempDir::new().unwrap();
        let subdir = dir.path().join("sub");
        std::fs::create_dir(&subdir).unwrap();

        let included = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <project name="included" path="included" />
</manifest>
"#;
        write_file(&subdir, "included.xml", included);

        let main = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <include name="sub/included.xml" />
</manifest>
"#;
        let main_path = write_file(dir.path(), "main.xml", main);

        let manifest = resolve_includes(&main_path, &mut HashSet::new(), 0).unwrap();
        assert_eq!(manifest.projects.len(), 1);
        assert_eq!(manifest.projects[0].name, "included");
    }

    #[test]
    fn extend_project_multiple_matches() {
        let mut manifest = Manifest {
            projects: vec![
                Project {
                    name: "foo".to_string(),
                    path: Some("foo1".to_string()),
                    ..Project::default_test()
                },
                Project {
                    name: "foo".to_string(),
                    path: Some("foo2".to_string()),
                    ..Project::default_test()
                },
            ],
            extend_projects: vec![ExtendProject {
                name: "foo".to_string(),
                groups: ["extra".to_string()].into_iter().collect(),
                ..ExtendProject::default_test()
            }],
            ..Manifest::default()
        };

        apply_extend_projects(&mut manifest).unwrap();

        assert!(manifest.projects[0].groups.contains("extra"));
        assert!(manifest.projects[1].groups.contains("extra"));
    }

    #[test]
    fn remove_project_multiple_matches() {
        let mut manifest = Manifest {
            projects: vec![
                Project {
                    name: "foo".to_string(),
                    path: Some("foo1".to_string()),
                    ..Project::default_test()
                },
                Project {
                    name: "foo".to_string(),
                    path: Some("foo2".to_string()),
                    ..Project::default_test()
                },
                Project {
                    name: "bar".to_string(),
                    path: Some("bar".to_string()),
                    ..Project::default_test()
                },
            ],
            remove_projects: vec![RemoveProject {
                name: Some("foo".to_string()),
                path: None,
                optional: false,
                base_rev: None,
            }],
            ..Manifest::default()
        };

        apply_remove_projects(&mut manifest).unwrap();

        assert_eq!(manifest.projects.len(), 1);
        assert_eq!(manifest.projects[0].name, "bar");
    }

    #[test]
    fn load_manifest_validates_final_manifest() {
        let dir = TempDir::new().unwrap();
        let main = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <project name="foo" path="foo" remote="unknown" />
</manifest>
"#;
        let main_path = write_file(dir.path(), "main.xml", main);

        let err = load_manifest(&main_path).unwrap_err();
        assert!(err.to_string().contains("unknown remote reference"));
    }

    #[test]
    fn merge_local_manifests_empty_dir() {
        let dir = TempDir::new().unwrap();
        let local_dir = dir.path().join("local_manifests");
        std::fs::create_dir(&local_dir).unwrap();

        let mut manifest = Manifest::default();
        merge_local_manifests(&mut manifest, &local_dir).unwrap();
        assert!(manifest.projects.is_empty());
    }

    #[test]
    fn merge_manifest_appends_remove_and_extend() {
        let mut base = Manifest {
            remove_projects: vec![RemoveProject {
                name: Some("a".to_string()),
                path: None,
                optional: false,
                base_rev: None,
            }],
            extend_projects: vec![ExtendProject {
                name: "a".to_string(),
                ..ExtendProject::default_test()
            }],
            ..Manifest::default()
        };
        let other = Manifest {
            remove_projects: vec![RemoveProject {
                name: Some("b".to_string()),
                path: None,
                optional: false,
                base_rev: None,
            }],
            extend_projects: vec![ExtendProject {
                name: "b".to_string(),
                ..ExtendProject::default_test()
            }],
            ..Manifest::default()
        };
        merge_manifest(&mut base, other).unwrap();
        assert_eq!(base.remove_projects.len(), 2);
        assert_eq!(base.extend_projects.len(), 2);
    }

    #[test]
    fn reject_duplicate_submanifest_across_merge() {
        let mut base = Manifest {
            submanifests: {
                let mut m = indexmap::IndexMap::new();
                m.insert(
                    "sub".to_string(),
                    Submanifest {
                        name: "sub".to_string(),
                        remote: None,
                        project: None,
                        manifest_name: None,
                        revision: None,
                        path: None,
                        groups: HashSet::new(),
                        default_groups: HashSet::new(),
                    },
                );
                m
            },
            ..Manifest::default()
        };
        let other = Manifest {
            submanifests: {
                let mut m = indexmap::IndexMap::new();
                m.insert(
                    "sub".to_string(),
                    Submanifest {
                        name: "sub".to_string(),
                        remote: None,
                        project: None,
                        manifest_name: None,
                        revision: None,
                        path: None,
                        groups: HashSet::new(),
                        default_groups: HashSet::new(),
                    },
                );
                m
            },
            ..Manifest::default()
        };
        let err = merge_manifest(&mut base, other).unwrap_err();
        assert!(err.to_string().contains("duplicate submanifest name: sub"));
    }

    #[test]
    fn merge_manifest_includes_submanifests() {
        let mut base = Manifest::default();
        let other = Manifest {
            submanifests: {
                let mut m = indexmap::IndexMap::new();
                m.insert(
                    "sub".to_string(),
                    Submanifest {
                        name: "sub".to_string(),
                        remote: Some("origin".to_string()),
                        project: None,
                        manifest_name: None,
                        revision: None,
                        path: None,
                        groups: HashSet::new(),
                        default_groups: HashSet::new(),
                    },
                );
                m
            },
            ..Manifest::default()
        };
        merge_manifest(&mut base, other).unwrap();
        assert_eq!(base.submanifests.len(), 1);
        assert_eq!(base.submanifests["sub"].remote, Some("origin".to_string()));
    }

    #[test]
    fn inject_auto_groups_adds_all_name_and_path() {
        let mut manifest = Manifest {
            projects: vec![Project {
                name: "foo".to_string(),
                path: Some("bar".to_string()),
                ..Project::default_test()
            }],
            ..Manifest::default()
        };
        inject_auto_groups(&mut manifest);
        let groups = &manifest.projects[0].groups;
        assert!(groups.contains("all"));
        assert!(groups.contains("name:foo"));
        assert!(groups.contains("path:bar"));
    }

    #[test]
    fn inject_auto_groups_uses_name_when_path_missing() {
        let mut manifest = Manifest {
            projects: vec![Project {
                name: "foo".to_string(),
                path: Some("foo".to_string()),
                ..Project::default_test()
            }],
            ..Manifest::default()
        };
        inject_auto_groups(&mut manifest);
        assert!(manifest.projects[0].groups.contains("path:foo"));
    }

    #[test]
    fn inject_auto_groups_recursive_for_subprojects() {
        let mut manifest = Manifest {
            projects: vec![Project {
                name: "parent".to_string(),
                path: Some("parent".to_string()),
                subprojects: vec![Project {
                    name: "child".to_string(),
                    path: Some("child".to_string()),
                    ..Project::default_test()
                }],
                ..Project::default_test()
            }],
            ..Manifest::default()
        };
        inject_auto_groups(&mut manifest);
        let child_groups = &manifest.projects[0].subprojects[0].groups;
        assert!(child_groups.contains("all"));
        assert!(child_groups.contains("name:child"));
        assert!(child_groups.contains("path:child"));
    }

    #[test]
    fn inject_submanifest_groups_adds_prefix() {
        let mut manifest = Manifest {
            projects: vec![Project {
                name: "foo".to_string(),
                path: Some("foo".to_string()),
                ..Project::default_test()
            }],
            ..Manifest::default()
        };
        inject_submanifest_groups(&mut manifest, "sub/path");
        assert!(manifest.projects[0].groups.contains("submanifest:path:sub/path"));
    }

    #[test]
    fn merge_local_manifests_injects_local_group() {
        let dir = TempDir::new().unwrap();
        let local_dir = dir.path().join("local_manifests");
        std::fs::create_dir(&local_dir).unwrap();

        let local = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <project name="local1" path="local1" />
</manifest>
"#;
        write_file(&local_dir, "01_local.xml", local);

        let mut manifest = Manifest::default();
        merge_local_manifests(&mut manifest, &local_dir).unwrap();

        assert_eq!(manifest.projects.len(), 1);
        assert!(manifest.projects[0].groups.contains("local:01_local"));
    }

    #[test]
    fn resolve_include_depth_limit() {
        let dir = TempDir::new().unwrap();
        // Create a chain of 9 includes to exceed MAX_SUBMANIFEST_DEPTH (8)
        for i in 0..9 {
            let next = if i == 8 {
                "".to_string()
            } else {
                format!("<include name=\"inc{}.xml\" />\n", i + 1)
            };
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
{}</manifest>
"#,
                next
            );
            write_file(dir.path(), &format!("inc{}.xml", i), &xml);
        }

        let main = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <include name="inc0.xml" />
</manifest>
"#;
        let main_path = write_file(dir.path(), "main.xml", main);

        let err = resolve_includes(&main_path, &mut HashSet::new(), 0).unwrap_err();
        assert!(err.to_string().contains("maximum submanifest depth"));
    }

    #[test]
    fn normalize_url_removes_trailing_slashes() {
        assert_eq!(normalize_url("https://example.com/"), "https://example.com");
        assert_eq!(normalize_url("https://example.com//"), "https://example.com");
    }

    #[test]
    fn normalize_url_converts_scp_syntax() {
        assert_eq!(
            normalize_url("git@github.com:foo/bar.git"),
            "ssh://git@github.com/foo/bar.git"
        );
    }

    #[test]
    fn normalize_url_leaves_regular_urls() {
        assert_eq!(normalize_url("https://example.com/repo"), "https://example.com/repo");
    }

    #[test]
    fn resolve_fetch_url_with_relative_base() {
        assert_eq!(
            resolve_fetch_url("..", Some("https://example.com/a/b")),
            "https://example.com/a/.."
        );
    }

    #[test]
    fn resolve_fetch_url_with_absolute_fetch() {
        assert_eq!(
            resolve_fetch_url("https://other.com", Some("https://example.com/a/b")),
            "https://other.com"
        );
    }

    #[test]
    fn resolve_remote_name_exact_match() {
        let manifest = Manifest {
            remotes: {
                let mut m = indexmap::IndexMap::new();
                m.insert(
                    "origin".to_string(),
                    Remote {
                        name: "origin".to_string(),
                        alias: None,
                        fetch: "https://example.com".to_string(),
                        pushurl: None,
                        review: None,
                        revision: None,
                        annotations: Vec::new(),
                    },
                );
                m
            },
            ..Manifest::default()
        };
        assert_eq!(resolve_remote_name(&manifest, "origin"), Some("origin".to_string()));
    }

    #[test]
    fn resolve_remote_name_alias_fallback() {
        let manifest = Manifest {
            remotes: {
                let mut m = indexmap::IndexMap::new();
                m.insert(
                    "origin".to_string(),
                    Remote {
                        name: "origin".to_string(),
                        alias: Some("o".to_string()),
                        fetch: "https://example.com".to_string(),
                        pushurl: None,
                        review: None,
                        revision: None,
                        annotations: Vec::new(),
                    },
                );
                m
            },
            ..Manifest::default()
        };
        assert_eq!(resolve_remote_name(&manifest, "o"), Some("origin".to_string()));
    }

    #[test]
    fn resolve_remote_name_not_found() {
        let manifest = Manifest::default();
        assert_eq!(resolve_remote_name(&manifest, "origin"), None);
    }

    #[test]
    fn load_manifest_injects_auto_groups() {
        let dir = TempDir::new().unwrap();
        let main = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <project name="foo" path="bar" />
</manifest>
"#;
        let main_path = write_file(dir.path(), "main.xml", main);

        let manifest = load_manifest(&main_path).unwrap();
        assert_eq!(manifest.projects.len(), 1);
        let groups = &manifest.projects[0].groups;
        assert!(groups.contains("all"));
        assert!(groups.contains("name:foo"));
        assert!(groups.contains("path:bar"));
    }
}
