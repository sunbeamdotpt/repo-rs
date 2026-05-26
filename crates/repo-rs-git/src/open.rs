// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8PathBuf;

use crate::Error;

/// Open an existing git repository at the given path.
///
/// # Errors
///
/// Returns an error if the path is not a valid git repository.
pub async fn open(path: &Utf8PathBuf) -> Result<git2::Repository, Error> {
    let path = path.as_std_path().to_path_buf();
    tokio::task::spawn_blocking(move || git2::Repository::open(path).map_err(Error::from)).await?
}

/// Initialize a new bare git repository at the given path.
///
/// # Errors
///
/// Returns an error if the repository cannot be created.
pub async fn init_bare(path: &Utf8PathBuf) -> Result<git2::Repository, Error> {
    let path = path.as_std_path().to_path_buf();
    tokio::task::spawn_blocking(move || git2::Repository::init_bare(path).map_err(Error::from)).await?
}

/// Initialize a new non-bare git repository at the given path.
///
/// # Errors
///
/// Returns an error if the repository cannot be created.
pub async fn init(path: &Utf8PathBuf) -> Result<git2::Repository, Error> {
    let path = path.as_std_path().to_path_buf();
    tokio::task::spawn_blocking(move || git2::Repository::init(path).map_err(Error::from)).await?
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_open_existing_repo() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        git2::Repository::init(&path).unwrap();

        let repo = open(&path).await.unwrap();
        assert!(!repo.is_bare());
    }

    #[tokio::test]
    async fn test_open_nonexistent_path() {
        let path = Utf8PathBuf::from("/tmp/repo_git_open_nonexistent_99999");
        let result = open(&path).await;
        assert!(result.is_err());
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected error"),
        };
        assert!(matches!(err, Error::Git2(_)));
    }

    #[tokio::test]
    async fn test_init_creates_valid_repo() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().join("repo")).unwrap();

        let repo = init(&path).await.unwrap();
        assert!(!repo.is_bare());
        assert!(path.join(".git").exists());
    }

    #[tokio::test]
    async fn test_init_bare_creates_valid_repo() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().join("repo")).unwrap();

        let repo = init_bare(&path).await.unwrap();
        assert!(repo.is_bare());
        assert!(path.join("HEAD").exists());
    }
}
