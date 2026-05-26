// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::Error;

/// Resolve a ref to its target SHA (full hex).
///
/// # Errors
///
/// Returns an error if the ref does not exist.
pub async fn resolve(repo: git2::Repository, name: &str) -> Result<String, Error> {
    let name = name.to_string();
    tokio::task::spawn_blocking(move || -> Result<String, git2::Error> {
        let obj = repo.revparse_single(name.as_str())?;
        Ok(obj.id().to_string())
    })
    .await
    .map_err(|e| Error::Git2(e.to_string()))?
    .map_err(Error::from)
}

/// List all refs matching the given pattern.
///
/// Returns a vector of `(ref_name, target_sha)` pairs.
///
/// # Errors
///
/// Returns an error if the references cannot be read.
pub async fn list(
    repo: git2::Repository,
    pattern: Option<&str>,
) -> Result<Vec<(String, String)>, Error> {
    let pattern = pattern.map(String::from);
    tokio::task::spawn_blocking(move || -> Result<Vec<(String, String)>, git2::Error> {
        let mut results = Vec::new();
        let refs = repo.references()?;
        for reference in refs {
            let reference = reference?;
            let name = match reference.name() {
                Ok(n) => n.to_string(),
                Err(_) => continue,
            };
            if let Some(ref pat) = pattern {
                if !name.starts_with(pat) && !name.contains(pat) {
                    continue;
                }
            }
            if let Some(id) = reference.target() {
                results.push((name, id.to_string()));
            }
        }
        Ok(results)
    })
    .await
    .map_err(|e| Error::Git2(e.to_string()))?
    .map_err(Error::from)
}

/// Create or update a ref to point to a target object.
///
/// # Errors
///
/// Returns an error if the ref cannot be updated.
pub async fn update(repo: git2::Repository, name: &str, target: &str) -> Result<(), Error> {
    let name = name.to_string();
    let target = target.to_string();
    tokio::task::spawn_blocking(move || -> Result<(), git2::Error> {
        if target.is_empty() {
            let mut reference = repo.find_reference(&name)?;
            reference.delete()?;
        } else {
            let obj = repo.revparse_single(target.as_str())?;
            repo.reference(&name, obj.id(), true, "repo-rs: update ref")?;
        }
        Ok(())
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

    fn create_repo_with_commit(dir: &std::path::Path) -> git2::Repository {
        let path = Utf8PathBuf::from_path_buf(dir.to_path_buf()).unwrap();
        Command::new("git")
            .args(["init", path.as_str()])
            .output()
            .expect("git init failed");
        Command::new("git")
            .args([
                "-C",
                path.as_str(),
                "commit",
                "--allow-empty",
                "-m",
                "initial",
                "--no-gpg-sign",
            ])
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .output()
            .expect("git commit failed");
        git2::Repository::open(dir).unwrap()
    }

    #[tokio::test]
    async fn test_resolve_head() {
        let dir = tempdir().unwrap();
        let repo = create_repo_with_commit(dir.path());
        let sha = resolve(repo, "HEAD").await.unwrap();
        assert_eq!(sha.len(), 40);
    }

    #[tokio::test]
    async fn test_resolve_nonexistent_ref() {
        let dir = tempdir().unwrap();
        let repo = create_repo_with_commit(dir.path());
        let err = resolve(repo, "refs/heads/nonexistent")
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Git2(_)));
    }

    #[tokio::test]
    async fn test_list_refs() {
        let dir = tempdir().unwrap();
        let repo = create_repo_with_commit(dir.path());
        let refs = list(repo, None).await.unwrap();
        assert!(!refs.is_empty());
        assert!(refs.iter().any(|(name, _)| name.starts_with("refs/heads/")));
    }

    #[tokio::test]
    async fn test_list_refs_with_pattern() {
        let dir = tempdir().unwrap();
        let repo = create_repo_with_commit(dir.path());
        let refs = list(repo, Some("refs/heads/")).await.unwrap();
        assert!(!refs.is_empty());
        assert!(refs.iter().all(|(name, _)| name.starts_with("refs/heads/")));
    }

    #[tokio::test]
    async fn test_list_refs_no_match() {
        let dir = tempdir().unwrap();
        let repo = create_repo_with_commit(dir.path());
        let refs = list(repo, Some("zzzzzz")).await.unwrap();
        assert!(refs.is_empty());
    }

    #[tokio::test]
    async fn test_update_ref() {
        let dir = tempdir().unwrap();
        let repo = create_repo_with_commit(dir.path());
        let head_sha = {
            let repo2 = git2::Repository::open(dir.path()).unwrap();
            resolve(repo2, "HEAD").await.unwrap()
        };

        update(repo, "refs/tags/test-tag", &head_sha)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_update_ref_invalid_target() {
        let dir = tempdir().unwrap();
        let repo = create_repo_with_commit(dir.path());
        let err = update(repo, "refs/tags/bad", "deadbeef")
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Git2(_)));
    }
}
