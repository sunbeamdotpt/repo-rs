// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use crate::Error;
use repo_rs_git::ProgressCallback;
use repo_rs_model::RepoClient;

/// Execution context passed to every engine operation.
pub struct Context {
    /// Absolute path to the repo checkout root.
    pub repo_root: Utf8PathBuf,
    /// The primary repo client.
    pub client: RepoClient,
    /// The outer repo client (for nested manifests).
    pub outer_client: Option<RepoClient>,
    /// Progress callback.
    pub progress: Box<dyn ProgressCallback>,
    /// Color choice override.
    pub color_choice: anstream::ColorChoice,
    /// Git backend for performing git operations.
    pub git: std::sync::Arc<dyn repo_rs_git::GitBackend>,
}

impl Context {
    /// Load a repo checkout from the current working directory.
    ///
    /// Walks up from `start_dir` looking for a `.repo/` directory, then
    /// loads the manifest and builds a [`RepoClient`].
    ///
    /// # Errors
    ///
    /// Returns an error if no `.repo/` directory is found, the manifest
    /// cannot be loaded, or project resolution fails.
    pub fn load(
        start_dir: &Utf8PathBuf,
        git: std::sync::Arc<dyn repo_rs_git::GitBackend>,
    ) -> Result<Self, Error> {
        let repo_dir = repo_rs_model::layout::discover_repo_dir(start_dir)
            .ok_or_else(|| Error::InvalidArguments(
                format!("no .repo directory found in or above {}", start_dir)
            ))?;
        let repo_root = repo_dir
            .parent()
            .ok_or_else(|| Error::InvalidArguments(
                format!("invalid repo dir: {}", repo_dir)
            ))?
            .to_path_buf();

        let manifest_name = repo_rs_model::layout::read_manifest_name(&repo_dir)
            .map_err(|e| Error::Io(e))?
            .unwrap_or_else(|| "default.xml".to_string());

        let manifest_path = repo_dir.join("manifests").join(&manifest_name);
        let manifest = repo_rs_manifest::load_manifest(manifest_path.as_std_path())
            .map_err(|e| Error::InvalidArguments(format!("manifest load error: {e}")))?;

        let builder = repo_rs_model::ManifestBuilder::new(repo_dir);
        let client = builder.build(&manifest)
            .map_err(|e| Error::InvalidArguments(format!("manifest build error: {e}")))?;

        Ok(Self {
            repo_root,
            client,
            outer_client: None,
            progress: Box::new(()),
            color_choice: anstream::ColorChoice::Auto,
            git,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn test_context_load() {
        let tmp = tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let repo_dir = root.join(".repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::fs::create_dir_all(repo_dir.join("manifests")).unwrap();
        std::fs::write(
            repo_dir.join("manifest.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <include name="default.xml" />
</manifest>
"#,
        ).unwrap();
        std::fs::write(
            repo_dir.join("manifests/default.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="foo" path="foo" />
</manifest>
"#,
        ).unwrap();

        let ctx = Context::load(&root, Arc::new(repo_rs_git::DefaultBackend)).unwrap();
        assert_eq!(ctx.client.projects.len(), 1);
        assert_eq!(ctx.client.projects.values().next().unwrap().name, "foo");
    }

    #[test]
    fn test_context_load_no_repo() {
        let tmp = tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        match Context::load(&root, Arc::new(repo_rs_git::DefaultBackend)) {
            Ok(_) => panic!("expected error"),
            Err(e) => assert!(e.to_string().contains("no .repo directory")),
        }
    }
}
