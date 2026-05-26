// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;
use url::Url;

use crate::{Error, ProgressCallback};

/// Clone a repository.
///
/// # Errors
///
/// Returns an error if the clone fails.
pub async fn clone(
    url: &Url,
    dst: &Utf8PathBuf,
    _depth: Option<u32>,
    _filter: Option<&str>,
    _progress: &dyn ProgressCallback,
) -> Result<git2::Repository, Error> {
    let url = url.to_string();
    let dst = dst.as_std_path().to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<git2::Repository, git2::Error> {
        let mut builder = git2::build::RepoBuilder::new();
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(|_url, username_from_url, allowed_types| {
            crate::push::credentials_callback(_url, username_from_url, allowed_types)
        });
        let mut fetch_opts = git2::FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);
        builder.fetch_options(fetch_opts);
        let repo = builder.clone(&url, &dst)?;
        Ok(repo)
    })
    .await
    .map_err(|e| Error::Git2(e.to_string()))?
    .map_err(Error::from)
}

/// Clone a repository as a bare mirror.
///
/// # Errors
///
/// Returns an error if the clone fails.
pub async fn clone_bare(
    url: &Url,
    dst: &Utf8PathBuf,
    _progress: &dyn ProgressCallback,
) -> Result<git2::Repository, Error> {
    let url = url.to_string();
    let dst = dst.as_std_path().to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<git2::Repository, git2::Error> {
        let mut builder = git2::build::RepoBuilder::new();
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(|_url, username_from_url, allowed_types| {
            crate::push::credentials_callback(_url, username_from_url, allowed_types)
        });
        let mut fetch_opts = git2::FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);
        builder.fetch_options(fetch_opts);
        let repo = builder.bare(true).clone(&url, &dst)?;
        Ok(repo)
    })
    .await
    .map_err(|e| Error::Git2(e.to_string()))?
    .map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use std::process::Command;
    use tempfile::tempdir;

    fn git_init_bare_with_commit(path: &std::path::Path) -> Utf8PathBuf {
        let path = Utf8PathBuf::from_path_buf(path.to_path_buf()).unwrap();

        Command::new("git")
            .args(["init", "--bare", path.as_str()])
            .output()
            .expect("git init --bare failed");

        let tmp = tempfile::tempdir().unwrap();
        let tmp_path = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();

        Command::new("git")
            .args(["clone", path.as_str(), tmp_path.as_str()])
            .output()
            .expect("git clone failed");

        std::fs::write(tmp_path.join("README.md"), "hello").unwrap();

        Command::new("git")
            .args(["-C", tmp_path.as_str(), "add", "."])
            .output()
            .expect("git add failed");

        Command::new("git")
            .args(["-C", tmp_path.as_str(), "commit", "-m", "initial", "--no-gpg-sign"])
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .output()
            .expect("git commit failed");

        Command::new("git")
            .args(["-C", tmp_path.as_str(), "push", "origin", "HEAD"])
            .output()
            .expect("git push failed");

        path
    }

    #[tokio::test]
    async fn test_clone_from_local_bare_repo() {
        let bare_dir = tempdir().unwrap();
        let bare_path = git_init_bare_with_commit(bare_dir.path());
        let dst_dir = tempdir().unwrap();
        let dst_path = Utf8PathBuf::from_path_buf(dst_dir.path().join("cloned")).unwrap();
        let url = Url::from_file_path(&bare_path).unwrap();

        let repo = clone(&url, &dst_path, None, None, &()).await.unwrap();
        assert!(!repo.is_bare());

        let head = crate::refs::resolve(repo, "HEAD").await.unwrap();
        assert_eq!(head.len(), 40);
    }

    #[tokio::test]
    async fn test_clone_bare_from_local_bare_repo() {
        let bare_dir = tempdir().unwrap();
        let bare_path = git_init_bare_with_commit(bare_dir.path());
        let dst_dir = tempdir().unwrap();
        let dst_path = Utf8PathBuf::from_path_buf(dst_dir.path().join("cloned")).unwrap();
        let url = Url::from_file_path(&bare_path).unwrap();

        let repo = clone_bare(&url, &dst_path, &()).await.unwrap();
        assert!(repo.is_bare());
    }

    #[tokio::test]
    async fn test_clone_invalid_url() {
        let dst_dir = tempdir().unwrap();
        let dst_path = Utf8PathBuf::from_path_buf(dst_dir.path().join("cloned")).unwrap();
        let url = Url::parse("file:///tmp/repo_git_nonexistent_clone_99999").unwrap();

        let result = clone(&url, &dst_path, None, None, &()).await;
        assert!(result.is_err());
        assert!(result.is_err());
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected error"),
        };
        assert!(matches!(err, Error::Git2(_)));
    }
}
