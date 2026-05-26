// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{Context, Error};

/// Version information.
#[derive(Debug, Clone, Default)]
pub struct VersionInfo {
    /// Repo version.
    pub repo_version: String,
    /// Git version.
    pub git_version: String,
    /// Rust version.
    pub rust_version: String,
    /// Os info.
    pub os_info: String,
}

/// Trait for version logic.
#[async_trait::async_trait]
pub trait Version {
    /// Return version information.
    async fn version(&self, ctx: &Context) -> Result<VersionInfo, Error>;
}

/// Default version implementation.
pub struct DefaultVersion;

#[async_trait::async_trait]
impl Version for DefaultVersion {
    async fn version(&self, _ctx: &Context) -> Result<VersionInfo, Error> {
        Ok(VersionInfo {
            repo_version: env!("CARGO_PKG_VERSION").to_string(),
            git_version: "git2 0.21".to_string(),
            rust_version: format!("{} on {}", std::env::consts::ARCH, std::env::consts::OS),
            os_info: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        })
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

    fn make_context() -> Context {
        Context {
            repo_root: Utf8PathBuf::from("/tmp"),
            client: RepoClient {
                repo_dir: Utf8PathBuf::from("/tmp/.repo"),
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
    async fn test_version_returns_info() {
        let ctx = make_context();
        let engine = DefaultVersion;
        let info = engine.version(&ctx).await.unwrap();
        assert_eq!(info.repo_version, env!("CARGO_PKG_VERSION"));
        assert!(info.git_version.contains("git2"));
        assert!(!info.rust_version.is_empty());
        assert!(!info.os_info.is_empty());
    }
}
