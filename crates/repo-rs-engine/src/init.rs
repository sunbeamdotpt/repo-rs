// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{Context, Error};
use repo_rs_model::{ManifestBuilder, RepoClient};

/// Options controlling init behavior.
#[derive(Debug, Clone, Default)]
pub struct InitOptions {
    /// Manifest url.
    pub manifest_url: String,
    /// Manifest branch.
    pub manifest_branch: Option<String>,
    /// The name of the manifest file.
    pub manifest_name: Option<String>,
    /// Mirror.
    pub mirror: bool,
    /// The filesystem path to the project's worktree.
    pub worktree: bool,
    /// Reference.
    pub reference: Option<String>,
    /// Depth.
    pub depth: Option<u32>,
    /// Partial clone.
    pub partial_clone: bool,
    /// Clone bundle.
    pub clone_bundle: bool,
    /// Submodules.
    pub submodules: bool,
    /// Repo url.
    pub repo_url: Option<String>,
    /// Repo rev.
    pub repo_rev: Option<String>,
    /// Force.
    pub force: bool,
}

/// Trait for init logic.
#[async_trait::async_trait]
pub trait Init {
    /// Initialize a new repo client checkout.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails.
    async fn init(&self, ctx: &Context, opts: InitOptions) -> Result<RepoClient, Error>;
}

/// Default init implementation.
pub struct DefaultInit;

#[async_trait::async_trait]
impl Init for DefaultInit {
    async fn init(&self, ctx: &Context, opts: InitOptions) -> Result<RepoClient, Error> {
        let repo_dir = ctx.repo_root.join(".repo");

        if repo_dir.exists() && !opts.force {
            return Err(Error::InvalidArguments(format!(
                "repo directory already exists: {}",
                repo_dir
            )));
        }

        if opts.force && repo_dir.exists() {
            tokio::fs::remove_dir_all(&repo_dir).await?;
        }

        tokio::fs::create_dir_all(&repo_dir).await?;
        tokio::fs::create_dir_all(repo_dir.join("projects")).await?;
        tokio::fs::create_dir_all(repo_dir.join("project-objects")).await?;
        tokio::fs::create_dir_all(repo_dir.join("local_manifests")).await?;

        let manifest_dst = repo_dir.join("manifests");
        let url = url::Url::parse(&opts.manifest_url).map_err(|e| {
            Error::InvalidArguments(format!("invalid manifest url: {e}"))
        })?;
        let _manifest_repo = repo_rs_git::GitBackend::clone(
            &*ctx.git,
            &url,
            &manifest_dst,
            opts.depth,
            opts.partial_clone.then_some("blob:none"),
            ctx.progress.as_ref(),
        )
        .await?;

        let manifest_name = opts.manifest_name.as_deref().unwrap_or("default.xml");
        let manifest_xml = repo_dir.join("manifest.xml");
        let wrapper = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <include name="{}" />
</manifest>
"#,
            manifest_name
        );
        tokio::fs::write(&manifest_xml, wrapper).await?;

        let manifest_path = manifest_dst.join(manifest_name);
        let manifest = repo_rs_manifest::load_manifest(manifest_path.as_std_path())?;

        let builder = ManifestBuilder::new(repo_dir);
        let client = builder.build(&manifest).map_err(|e| {
            Error::InvalidArguments(format!("manifest build error: {e}"))
        })?;

        Ok(client)
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
    use tempfile::tempdir;

    fn make_context(tmp: &tempfile::TempDir) -> Context {
        let repo_root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let repo_dir = repo_root.join(".repo");
        Context {
            repo_root,
            client: RepoClient {
                repo_dir,
                manifest_project: MetaProject {
                    name: "manifests".to_string(),
                    path: Utf8PathBuf::from("/tmp/.repo/manifests"),
                    gitdir: Utf8PathBuf::from("/tmp/.repo/manifests.git"),
                },
                repo_project: MetaProject {
                    name: "repo".to_string(),
                    path: Utf8PathBuf::from("/tmp/.repo/repo"),
                    gitdir: Utf8PathBuf::from("/tmp/.repo/repo"),
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
    async fn test_init_creates_repo_dir() {
        let tmp = tempdir().unwrap();
        let mut ctx = make_context(&tmp);
        let mut mock = MockGitBackend::new();
        mock.expect_clone()
            .times(1)
            .returning(|_, dst, _, _, _| {
                let dst = dst.clone();
                Box::pin(async move {
                    std::fs::create_dir_all(&dst).unwrap();
                    // write a minimal manifest so load_manifest succeeds
                    std::fs::write(dst.join("default.xml"), r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="foo" path="foo" />
</manifest>
"#).unwrap();
                    Ok(git2::Repository::init(&dst).unwrap())
                })
            });
        ctx.git = Arc::new(mock);

        let engine = DefaultInit;
        let client = engine
            .init(
                &ctx,
                InitOptions {
                    manifest_url: "https://example.com/manifest.git".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert!(ctx.repo_root.join(".repo").exists());
        assert!(ctx.repo_root.join(".repo/manifest.xml").exists());
        assert!(ctx.repo_root.join(".repo/manifests").exists());
        assert!(ctx.repo_root.join(".repo/projects").exists());
        assert!(ctx.repo_root.join(".repo/project-objects").exists());
        assert_eq!(client.projects.len(), 1);
    }

    #[tokio::test]
    async fn test_init_existing_repo_error() {
        let tmp = tempdir().unwrap();
        let ctx = make_context(&tmp);
        std::fs::create_dir_all(ctx.repo_root.join(".repo")).unwrap();

        let engine = DefaultInit;
        let err = engine
            .init(
                &ctx,
                InitOptions {
                    manifest_url: "https://example.com/manifest.git".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn test_init_force_overwrites() {
        let tmp = tempdir().unwrap();
        let mut ctx = make_context(&tmp);
        std::fs::create_dir_all(ctx.repo_root.join(".repo")).unwrap();
        std::fs::write(ctx.repo_root.join(".repo/old.txt"), "old").unwrap();

        let mut mock = MockGitBackend::new();
        mock.expect_clone()
            .times(1)
            .returning(|_, dst, _, _, _| {
                let dst = dst.clone();
                Box::pin(async move {
                    std::fs::create_dir_all(&dst).unwrap();
                    std::fs::write(dst.join("default.xml"), r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="bar" path="bar" />
</manifest>
"#).unwrap();
                    Ok(git2::Repository::init(&dst).unwrap())
                })
            });
        ctx.git = Arc::new(mock);

        let engine = DefaultInit;
        let client = engine
            .init(
                &ctx,
                InitOptions {
                    manifest_url: "https://example.com/manifest.git".to_string(),
                    force: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert!(!ctx.repo_root.join(".repo/old.txt").exists());
        assert_eq!(client.projects.len(), 1);
        assert!(client.projects.contains_key(&Utf8PathBuf::from("bar")));
    }

    #[tokio::test]
    async fn test_init_invalid_url() {
        let tmp = tempdir().unwrap();
        let ctx = make_context(&tmp);
        let engine = DefaultInit;
        let err = engine
            .init(
                &ctx,
                InitOptions {
                    manifest_url: "not a url".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("invalid manifest url"));
    }

    #[tokio::test]
    async fn test_init_clone_error() {
        let tmp = tempdir().unwrap();
        let mut ctx = make_context(&tmp);
        let mut mock = MockGitBackend::new();
        mock.expect_clone()
            .times(1)
            .returning(|_, _, _, _, _| {
                Box::pin(async { Err(repo_rs_git::Error::Git2("clone failed".to_string())) })
            });
        ctx.git = Arc::new(mock);

        let engine = DefaultInit;
        let err = engine
            .init(
                &ctx,
                InitOptions {
                    manifest_url: "https://example.com/manifest.git".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("clone failed"));
    }

    #[tokio::test]
    async fn test_init_custom_manifest_name() {
        let tmp = tempdir().unwrap();
        let mut ctx = make_context(&tmp);
        let mut mock = MockGitBackend::new();
        mock.expect_clone()
            .times(1)
            .returning(|_, dst, _, _, _| {
                let dst = dst.clone();
                Box::pin(async move {
                    std::fs::create_dir_all(&dst).unwrap();
                    std::fs::write(dst.join("custom.xml"), r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="baz" path="baz" />
</manifest>
"#).unwrap();
                    Ok(git2::Repository::init(&dst).unwrap())
                })
            });
        ctx.git = Arc::new(mock);

        let engine = DefaultInit;
        let client = engine
            .init(
                &ctx,
                InitOptions {
                    manifest_url: "https://example.com/manifest.git".to_string(),
                    manifest_name: Some("custom.xml".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let manifest_xml = std::fs::read_to_string(ctx.repo_root.join(".repo/manifest.xml")).unwrap();
        assert!(manifest_xml.contains("custom.xml"));
        assert_eq!(client.projects.len(), 1);
        assert!(client.projects.contains_key(&Utf8PathBuf::from("baz")));
    }
}
