// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::Error;

/// Read a git config value as a string.
///
/// # Errors
///
/// Returns an error if the key is not found or the config is unreadable.
pub async fn get_string(repo: git2::Repository, key: &str) -> Result<String, Error> {
    let key = key.to_string();
    tokio::task::spawn_blocking(move || -> Result<String, git2::Error> {
        let config = repo.config()?;
        match config.get_string(&key) {
            Ok(v) => Ok(v),
            Err(e) => Err(git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Config,
                format!("key not found: {key}: {e}"),
            )),
        }
    })
    .await
    .map_err(|e| Error::Git2(e.to_string()))?
    .map_err(|e| Error::Config(e.message().to_string()))
}

/// Read a git config value as a boolean.
///
/// Returns `None` if the key is not found.
///
/// # Errors
///
/// Returns an error if the config is unreadable.
pub async fn get_bool(repo: git2::Repository, key: &str) -> Result<Option<bool>, Error> {
    let key = key.to_string();
    tokio::task::spawn_blocking(move || -> Result<Option<bool>, git2::Error> {
        let config = repo.config()?;
        match config.get_bool(&key) {
            Ok(v) => Ok(Some(v)),
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    })
    .await
    .map_err(|e| Error::Git2(e.to_string()))?
    .map_err(Error::from)
}

/// Set a git config value.
///
/// # Errors
///
/// Returns an error if the config cannot be written.
#[allow(clippy::missing_errors_doc)]
#[allow(clippy::unused_async)]
pub async fn set_string(repo: &mut git2::Repository, key: &str, value: &str) -> Result<(), Error> {
    let mut config = repo.config()?;
    config.set_str(key, value)?;
    Ok(())
}

/// Set a git config value in the local config file.
///
/// # Errors
///
/// Returns an error if the config cannot be written.
#[allow(clippy::missing_errors_doc)]
#[allow(clippy::unused_async)]
pub async fn set_string_local(
    repo: &mut git2::Repository,
    key: &str,
    value: &str,
) -> Result<(), Error> {
    let mut config = repo.config()?;
    config.set_str(key, value)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_get_set_string() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let mut repo = git2::Repository::init(&path).unwrap();

        set_string(&mut repo, "user.name", "Test User").await.unwrap();
        let val = get_string(repo, "user.name").await.unwrap();
        assert_eq!(val, "Test User");
    }

    #[tokio::test]
    async fn test_get_string_not_found() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let repo = git2::Repository::init(&path).unwrap();

        let err = get_string(repo, "nonexistent.key").await.unwrap_err();
        assert!(matches!(err, Error::Config(_)));
    }

    #[tokio::test]
    async fn test_get_set_bool() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let mut repo = git2::Repository::init(&path).unwrap();

        set_string(&mut repo, "core.bare", "true").await.unwrap();
        let val = get_bool(repo, "core.bare").await.unwrap();
        assert_eq!(val, Some(true));
    }

    #[tokio::test]
    async fn test_get_bool_not_found() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let repo = git2::Repository::init(&path).unwrap();

        let val = get_bool(repo, "nonexistent.key").await.unwrap();
        assert_eq!(val, None);
    }

    #[tokio::test]
    async fn test_set_string_local() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let mut repo = git2::Repository::init(&path).unwrap();

        set_string_local(&mut repo, "user.email", "test@example.com")
            .await
            .unwrap();
        let val = get_string(repo, "user.email").await.unwrap();
        assert_eq!(val, "test@example.com");
    }
}
