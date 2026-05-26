// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Manifest resolution, merging, and mutation passes.
//!
//! This module implements:
//! - `<include>` resolution by reading referenced files and merging their contents
//! - Local manifest merging from a `local_manifests/` directory
//! - `<extend-project>` mutation pass
//! - `<remove-project>` deletion pass

use crate::Error;
use crate::model::*;
use crate::parser::{parse_manifest, validate_manifest};
use crate::validate::is_valid_path;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

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
    let mut manifest = resolve_includes(path, &mut HashSet::new())?;

    let local_dir = path
        .parent()
        .unwrap_or(Path::new("."))
        .join("local_manifests");
    if local_dir.is_dir() {
        merge_local_manifests(&mut manifest, &local_dir)?;
    }

    apply_extend_projects(&mut manifest);
    apply_remove_projects(&mut manifest);

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
fn resolve_includes(path: &Path, visited: &mut HashSet<PathBuf>) -> Result<Manifest, Error> {
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
        let mut resolved = resolve_includes(&include_path, visited)?;

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
        let local = parse_manifest(&xml)?;
        merge_manifest(manifest, local)?;
    }

    Ok(())
}

/// Apply `<extend-project>` mutations to a manifest.
///
/// For each `<extend-project>` directive, finds all existing projects with a
/// matching `name` and applies the following mutations:
///
/// - **groups**: New groups are added to the existing set.
/// - **path / dest-path**: Overwrites the project's path.
/// - **revision**: Overwrites the project's revision.
/// - **remote**: Overwrites the project's remote.
/// - **dest-branch**: Overwrites the project's dest-branch.
/// - **upstream**: Overwrites the project's upstream.
/// - **copyfiles**: Appended to the project's copyfiles.
/// - **linkfiles**: Appended to the project's linkfiles.
/// - **annotations**: Appended to the project's annotations.
pub fn apply_extend_projects(manifest: &mut Manifest) {
    let extensions = manifest.extend_projects.clone();
    for ext in extensions {
        for project in &mut manifest.projects {
            if project.name != ext.name {
                continue;
            }

            for group in &ext.groups {
                project.groups.insert(group.clone());
            }

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
}

/// Apply `<remove-project>` deletions to a manifest.
///
/// Removes any project whose `name` matches a `<remove-project>` directive's
/// `name` attribute, or whose `path` matches a directive's `path` attribute.
/// A project is removed if **either** condition matches (OR semantics).
pub fn apply_remove_projects(manifest: &mut Manifest) {
    let removals = manifest.remove_projects.clone();
    manifest.projects.retain(|project| {
        for removal in &removals {
            let matches_name = removal.name.as_ref() == Some(&project.name);
            let matches_path = removal
                .path
                .as_ref()
                .is_some_and(|p| project.path.as_ref() == Some(p));

            if matches_name || matches_path {
                return false;
            }
        }
        true
    });
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

        let manifest = resolve_includes(&main_path, &mut HashSet::new()).unwrap();
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

        let manifest = resolve_includes(&main_path, &mut HashSet::new()).unwrap();
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

        let err = resolve_includes(&main_path, &mut HashSet::new()).unwrap_err();
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

        let err = resolve_includes(&main_path, &mut HashSet::new()).unwrap_err();
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

        let manifest = resolve_includes(&main_path, &mut HashSet::new()).unwrap();
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

        let err = resolve_includes(&main_path, &mut HashSet::new()).unwrap_err();
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

        apply_extend_projects(&mut manifest);

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

        apply_extend_projects(&mut manifest);

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

        apply_extend_projects(&mut manifest);

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

        apply_extend_projects(&mut manifest);

        assert!(!manifest.projects[0].groups.contains("extra"));
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

        apply_remove_projects(&mut manifest);

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

        apply_remove_projects(&mut manifest);

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

        apply_remove_projects(&mut manifest);

        assert_eq!(manifest.projects.len(), 1);
        assert_eq!(manifest.projects[0].name, "baz");
    }

    #[test]
    fn apply_remove_project_no_match() {
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

        apply_remove_projects(&mut manifest);

        assert_eq!(manifest.projects.len(), 1);
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

        let manifest = resolve_includes(&main_path, &mut HashSet::new()).unwrap();
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

        apply_extend_projects(&mut manifest);

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

        apply_remove_projects(&mut manifest);

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
}
