// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{Context, Error};

/// Options controlling manifest export behavior.
#[derive(Debug, Clone, Default)]
pub struct ManifestOptions {
    /// Output.
    pub output: Option<String>,
    /// Format.
    pub format: ManifestFormat,
    /// Revision as head.
    pub revision_as_head: bool,
    /// Pretty.
    pub pretty: bool,
    /// No local manifests.
    pub no_local_manifests: bool,
}

/// Output format for manifest export.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ManifestFormat {
    #[default]
    /// XML output.
    Xml,
    /// Json variant.
    Json,
}

/// Trait for manifest logic.
#[async_trait::async_trait]
pub trait ManifestCmd {
    /// Output the current manifest XML.
    async fn manifest(&self, ctx: &Context, opts: ManifestOptions) -> Result<String, Error>;
}

/// Default manifest implementation.
pub struct DefaultManifestCmd;

#[async_trait::async_trait]
impl ManifestCmd for DefaultManifestCmd {
    async fn manifest(&self, ctx: &Context, _opts: ManifestOptions) -> Result<String, Error> {
        let manifest_xml = ctx.repo_root.join(".repo/manifest.xml");
        let content = tokio::fs::read_to_string(&manifest_xml).await.map_err(|e| {
            Error::InvalidArguments(format!("cannot read manifest.xml: {e}"))
        })?;
        Ok(content)
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
    async fn test_manifest_reads_file() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp);
        std::fs::write(ctx.repo_root.join(".repo/manifest.xml"), "<manifest/>").unwrap();

        let engine = DefaultManifestCmd;
        let text = engine.manifest(&ctx, ManifestOptions::default()).await.unwrap();
        assert_eq!(text, "<manifest/>");
    }

    #[tokio::test]
    async fn test_manifest_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp);
        let engine = DefaultManifestCmd;
        let err = engine.manifest(&ctx, ManifestOptions::default()).await.unwrap_err();
        assert!(err.to_string().contains("cannot read manifest.xml"));
    }
}
