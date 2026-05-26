// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::{Utf8Path, Utf8PathBuf};
use std::fs;

/// Discover the `.repo/` directory by walking up from `start_dir`.
///
/// Returns `None` if no `.repo/` directory is found.
pub fn discover_repo_dir(start_dir: &Utf8Path) -> Option<Utf8PathBuf> {
    let mut current = Some(start_dir);
    while let Some(dir) = current {
        let repo_dir = dir.join(".repo");
        if repo_dir.is_dir() {
            return Some(repo_dir);
        }
        current = dir.parent();
    }
    None
}

/// Return the standard `.repo/` subdirectories and files.
pub struct RepoLayout {
    /// Repo dir.
    pub repo_dir: Utf8PathBuf,
    /// Manifest xml.
    pub manifest_xml: Utf8PathBuf,
    /// Local manifests.
    pub local_manifests: Utf8PathBuf,
    /// Manifest git.
    pub manifest_git: Utf8PathBuf,
    /// Project objects.
    pub project_objects: Utf8PathBuf,
    /// The list of projects in the manifest.
    pub projects: Utf8PathBuf,
    /// Repo git.
    pub repo_git: Utf8PathBuf,
}

impl RepoLayout {
    /// Create a new [`RepoLayout`] for the given `.repo/` directory.
    pub fn new(repo_dir: Utf8PathBuf) -> Self {
        Self {
            manifest_xml: repo_dir.join("manifest.xml"),
            local_manifests: repo_dir.join("local_manifests"),
            manifest_git: repo_dir.join("manifests.git"),
            project_objects: repo_dir.join("project-objects"),
            projects: repo_dir.join("projects"),
            repo_git: repo_dir.join("repo"),
            repo_dir,
        }
    }

    /// Returns the path to a project's bare git directory.
    #[must_use]
    pub fn project_gitdir(&self, project_name: &str) -> Utf8PathBuf {
        self.projects.join(format!("{project_name}.git"))
    }

    /// Returns the path to a project's shared object directory.
    #[must_use]
    pub fn project_objdir(&self, project_name: &str) -> Utf8PathBuf {
        self.project_objects.join(format!("{project_name}.git"))
    }
}

/// Check whether `path` is inside a repo client checkout.
pub fn is_inside_repo(path: &Utf8Path) -> bool {
    discover_repo_dir(path).is_some()
}

/// Read the current manifest name from `.repo/manifest.xml`.
///
/// If the manifest is a generated wrapper, this parses the `<include>`
/// element to find the real manifest name.
pub fn read_manifest_name(repo_dir: &Utf8Path) -> Result<Option<String>, std::io::Error> {
    let manifest_path = repo_dir.join("manifest.xml");
    if !manifest_path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&manifest_path)?;
    // Simple heuristic: if the file contains `<include name="..." />`,
    // extract the name attribute.
    if let Some(start) = content.find("<include ") {
        if let Some(name_start) = content[start..].find("name=\"") {
            let name_start = start + name_start + 6;
            if let Some(name_end) = content[name_start..].find('"') {
                return Ok(Some(content[name_start..name_start + name_end].to_string()));
            }
        }
    }
    Ok(Some("default.xml".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_discover_repo_dir() {
        let tmp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(tmp.path().to_path_buf()).unwrap();
        let repo_dir = root.join(".repo");
        fs::create_dir(&repo_dir).unwrap();
        let subdir = root.join("a/b/c");
        fs::create_dir_all(&subdir).unwrap();

        assert_eq!(
            discover_repo_dir(&subdir),
            Some(repo_dir.clone())
        );
        assert_eq!(
            discover_repo_dir(&root),
            Some(repo_dir)
        );
    }

    #[test]
    fn test_discover_repo_dir_not_found() {
        let tmp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(tmp.path().to_path_buf()).unwrap();
        let subdir = root.join("a/b");
        fs::create_dir_all(&subdir).unwrap();

        assert_eq!(discover_repo_dir(&subdir), None);
        assert_eq!(discover_repo_dir(&root), None);
    }

    #[test]
    fn test_discover_repo_dir_file_named_repo() {
        let tmp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(tmp.path().to_path_buf()).unwrap();
        let file = root.join(".repo");
        fs::write(&file, "not a directory").unwrap();
        let subdir = root.join("a");
        fs::create_dir(&subdir).unwrap();

        // .repo must be a directory, not a file
        assert_eq!(discover_repo_dir(&subdir), None);
    }

    #[test]
    fn test_discover_repo_dir_nested() {
        let tmp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(tmp.path().to_path_buf()).unwrap();
        let inner_repo = root.join("inner/.repo");
        fs::create_dir_all(&inner_repo).unwrap();
        let deep = root.join("inner/a/b");
        fs::create_dir_all(&deep).unwrap();

        assert_eq!(discover_repo_dir(&deep), Some(inner_repo));
    }

    #[test]
    fn test_repo_layout() {
        let tmp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(tmp.path().to_path_buf()).unwrap();
        let layout = RepoLayout::new(root.join(".repo"));
        assert_eq!(layout.manifest_xml, root.join(".repo/manifest.xml"));
        assert_eq!(
            layout.project_gitdir("platform/build"),
            root.join(".repo/projects/platform/build.git")
        );
    }

    #[test]
    fn test_repo_layout_objdir() {
        let tmp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(tmp.path().to_path_buf()).unwrap();
        let layout = RepoLayout::new(root.join(".repo"));
        assert_eq!(
            layout.project_objdir("platform/build"),
            root.join(".repo/project-objects/platform/build.git")
        );
    }

    #[test]
    fn test_repo_layout_all_paths() {
        let tmp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(tmp.path().to_path_buf()).unwrap();
        let layout = RepoLayout::new(root.clone());
        assert_eq!(layout.repo_dir, root);
        assert_eq!(layout.manifest_xml, root.join("manifest.xml"));
        assert_eq!(layout.local_manifests, root.join("local_manifests"));
        assert_eq!(layout.manifest_git, root.join("manifests.git"));
        assert_eq!(layout.project_objects, root.join("project-objects"));
        assert_eq!(layout.projects, root.join("projects"));
        assert_eq!(layout.repo_git, root.join("repo"));
    }

    #[test]
    fn test_is_inside_repo_true() {
        let tmp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(tmp.path().to_path_buf()).unwrap();
        fs::create_dir(root.join(".repo")).unwrap();
        assert!(is_inside_repo(&root));
        assert!(is_inside_repo(&root.join("sub")));
    }

    #[test]
    fn test_is_inside_repo_false() {
        let tmp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(tmp.path().to_path_buf()).unwrap();
        assert!(!is_inside_repo(&root));
    }

    #[test]
    fn test_read_manifest_name_missing() {
        let tmp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(tmp.path().to_path_buf()).unwrap();
        let result = read_manifest_name(&root).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_manifest_name_default() {
        let tmp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(tmp.path().to_path_buf()).unwrap();
        fs::write(root.join("manifest.xml"), "<manifest></manifest>").unwrap();
        let result = read_manifest_name(&root).unwrap();
        assert_eq!(result, Some("default.xml".to_string()));
    }

    #[test]
    fn test_read_manifest_name_include() {
        let tmp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(tmp.path().to_path_buf()).unwrap();
        fs::write(
            root.join("manifest.xml"),
            r#"<manifest><include name="custom.xml" /></manifest>"#,
        )
        .unwrap();
        let result = read_manifest_name(&root).unwrap();
        assert_eq!(result, Some("custom.xml".to_string()));
    }

    #[test]
    fn test_read_manifest_name_include_with_other_attrs() {
        let tmp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(tmp.path().to_path_buf()).unwrap();
        fs::write(
            root.join("manifest.xml"),
            r#"<manifest><include revision="main" name="branch.xml" /></manifest>"#,
        )
        .unwrap();
        let result = read_manifest_name(&root).unwrap();
        assert_eq!(result, Some("branch.xml".to_string()));
    }

    #[test]
    fn test_read_manifest_name_multiple_includes_first_wins() {
        let tmp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(tmp.path().to_path_buf()).unwrap();
        fs::write(
            root.join("manifest.xml"),
            r#"<manifest><include name="first.xml" /><include name="second.xml" /></manifest>"#,
        )
        .unwrap();
        let result = read_manifest_name(&root).unwrap();
        assert_eq!(result, Some("first.xml".to_string()));
    }

    #[test]
    fn test_read_manifest_name_no_name_attr() {
        let tmp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(tmp.path().to_path_buf()).unwrap();
        fs::write(
            root.join("manifest.xml"),
            r#"<manifest><include revision="main" /></manifest>"#,
        )
        .unwrap();
        let result = read_manifest_name(&root).unwrap();
        assert_eq!(result, Some("default.xml".to_string()));
    }

    #[test]
    fn test_read_manifest_name_unclosed_quote() {
        let tmp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(tmp.path().to_path_buf()).unwrap();
        fs::write(
            root.join("manifest.xml"),
            r#"<manifest><include name="unclosed /></manifest>"#,
        )
        .unwrap();
        let result = read_manifest_name(&root).unwrap();
        assert_eq!(result, Some("default.xml".to_string()));
    }
}
