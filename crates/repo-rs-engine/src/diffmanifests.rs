// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{Context, Error};

/// Options controlling diffmanifests behavior.
#[derive(Debug, Clone, Default)]
pub struct DiffManifestsOptions {
    /// Raw.
    pub raw: bool,
    /// Pretty format.
    pub pretty_format: Option<String>,
}

/// Trait for diffmanifests logic.
#[async_trait::async_trait]
pub trait DiffManifests {
    /// Diff two manifest revisions.

    async fn diff_manifests(
        &self,
        ctx: &Context,
        m1: String,
        m2: Option<String>,
        opts: DiffManifestsOptions,
    ) -> Result<String, Error>;
}

/// Default diffmanifests implementation.
pub struct DefaultDiffManifests;

#[async_trait::async_trait]
impl DiffManifests for DefaultDiffManifests {
    async fn diff_manifests(
        &self,
        ctx: &Context,
        m1: String,
        m2: Option<String>,
        _opts: DiffManifestsOptions,
    ) -> Result<String, Error> {
        let manifest_dir = ctx.client.manifest_project.path.clone();
        let path1 = manifest_dir.join(&m1);
        let content1 = tokio::fs::read_to_string(&path1).await.map_err(|e| {
            Error::InvalidArguments(format!("cannot read manifest {m1}: {e}"))
        })?;

        let m2 = m2.unwrap_or_else(|| "manifest.xml".to_string());
        let path2 = manifest_dir.join(&m2);
        let content2 = tokio::fs::read_to_string(&path2).await.map_err(|e| {
            Error::InvalidArguments(format!("cannot read manifest {m2}: {e}"))
        })?;

        let mut lines = Vec::new();
        if content1 == content2 {
            lines.push("Manifests are identical".to_string());
        } else {
            lines.push(format!("--- {m1}"));
            lines.push(format!("+++ {m2}"));
            lines.push("Manifests differ".to_string());
        }
        Ok(lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use camino::Utf8PathBuf;
    use repo_rs_git::MockGitBackend;
    use repo_rs_model::{client::MetaProject, RepoClient};
    use std::sync::Arc;

    fn make_context(tmp: &tempfile::TempDir) -> Context {
        let repo_root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let repo_dir = repo_root.join(".repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
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
                projects: indexmap::IndexMap::new(),
                submanifests: indexmap::IndexMap::new(),
            },
            outer_client: None,
            progress: Box::new(()),
            color_choice: anstream::ColorChoice::Auto,
            git: Arc::new(MockGitBackend::new()),
        }
    }

    #[tokio::test]
    async fn test_diffmanifests_identical() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp);
        let manifests = ctx.repo_root.join(".repo/manifests");
        std::fs::create_dir_all(&manifests).unwrap();
        std::fs::write(manifests.join("a.xml"), "<manifest/>").unwrap();
        std::fs::write(manifests.join("b.xml"), "<manifest/>").unwrap();

        let engine = DefaultDiffManifests;
        let text = engine
            .diff_manifests(&ctx, "a.xml".to_string(), Some("b.xml".to_string()), DiffManifestsOptions::default())
            .await
            .unwrap();
        assert!(text.contains("identical"));
    }

    #[tokio::test]
    async fn test_diffmanifests_different() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp);
        let manifests = ctx.repo_root.join(".repo/manifests");
        std::fs::create_dir_all(&manifests).unwrap();
        std::fs::write(manifests.join("a.xml"), "<manifest/>").unwrap();
        std::fs::write(manifests.join("b.xml"), "<manifest><project/></manifest>").unwrap();

        let engine = DefaultDiffManifests;
        let text = engine
            .diff_manifests(&ctx, "a.xml".to_string(), Some("b.xml".to_string()), DiffManifestsOptions::default())
            .await
            .unwrap();
        assert!(text.contains("differ"));
    }

    #[tokio::test]
    async fn test_diffmanifests_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp);
        let engine = DefaultDiffManifests;
        let err = engine
            .diff_manifests(&ctx, "missing.xml".to_string(), None, DiffManifestsOptions::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("cannot read manifest"));
    }
}
