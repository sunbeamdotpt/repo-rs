// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::Error;

/// Checkout a target revision in the repository's worktree.
pub async fn checkout(repo: git2::Repository, target: &str) -> Result<(), Error> {
    let target = target.to_string();
    tokio::task::spawn_blocking(move || {
        let obj = repo.revparse_single(&target)?;
        repo.checkout_tree(&obj, None)?;

        // Update HEAD
        if repo.find_reference(&target).is_ok() {
            repo.set_head(&target)?;
        } else {
            let branch_ref = format!("refs/heads/{target}");
            if repo.find_reference(&branch_ref).is_ok() {
                repo.set_head(&branch_ref)?;
            } else {
                repo.set_head_detached(obj.id())?;
            }
        }

        Ok(())
    })
    .await?
}

/// Return the name of the current branch, or `None` if HEAD is detached.
pub async fn head_name(repo: git2::Repository) -> Result<Option<String>, Error> {
    tokio::task::spawn_blocking(move || {
        if repo.head_detached()? {
            return Ok(None);
        }
        let head = repo.head()?;
        Ok(head.shorthand().ok().map(|s| s.to_string()))
    })
    .await?
}

/// Return `true` if the working tree has uncommitted changes.
pub async fn is_dirty(repo: git2::Repository) -> Result<bool, Error> {
    tokio::task::spawn_blocking(move || {
        let statuses = repo.statuses(None)?;
        Ok(!statuses.is_empty())
    })
    .await?
}

/// Return `(ahead, behind)` counts between HEAD and the given upstream ref.
pub async fn ahead_behind(repo: git2::Repository, upstream: &str) -> Result<(usize, usize), Error> {
    let upstream = upstream.to_string();
    tokio::task::spawn_blocking(move || {
        let local_id = repo.head()?.peel_to_commit()?.id();
        let upstream_obj = repo.revparse_single(&upstream)?;
        let upstream_id = upstream_obj.id();
        let (ahead, behind) = repo.graph_ahead_behind(local_id, upstream_id)?;
        Ok((ahead, behind))
    })
    .await?
}

/// Rebase the current branch onto the given upstream.
pub async fn rebase(repo: git2::Repository, upstream: &str) -> Result<(), Error> {
    let upstream = upstream.to_string();
    tokio::task::spawn_blocking(move || {
        let upstream_obj = repo.revparse_single(&upstream)?;
        let upstream_commit = upstream_obj.peel_to_commit()?;
        let upstream_annotated = repo.find_annotated_commit(upstream_commit.id())?;
        let mut rebase = repo.rebase(None, Some(&upstream_annotated), None, None)?;
        let signature = repo.signature().unwrap_or_else(|_| {
            git2::Signature::now("repo-rs", "repo-rs@localhost").unwrap()
        });
        while let Some(op) = rebase.next() {
            let _op = op?;
            if repo.index()?.has_conflicts() {
                rebase.abort()?;
                return Err(Error::Git2("rebase conflicts detected".to_string()));
            }
            rebase.commit(None, &signature, None)?;
        }
        rebase.finish(Some(&signature))?;
        Ok(())
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use std::process::Command;
    use tempfile::tempdir;

    fn create_repo_with_branch(dir: &std::path::Path, branch: &str) -> git2::Repository {
        let path = Utf8PathBuf::from_path_buf(dir.to_path_buf()).unwrap();
        Command::new("git")
            .args(["init", "--initial-branch", branch, path.as_str()])
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
    async fn test_head_name_on_branch() {
        let dir = tempdir().unwrap();
        let _repo = create_repo_with_branch(dir.path(), "main");
        let repo = git2::Repository::open(dir.path()).unwrap();
        let name = head_name(repo).await.unwrap();
        assert_eq!(name, Some("main".to_string()));
    }

    #[tokio::test]
    async fn test_head_name_detached() {
        let dir = tempdir().unwrap();
        let _repo = create_repo_with_branch(dir.path(), "main");
        let sha = crate::refs::resolve(git2::Repository::open(dir.path()).unwrap(), "HEAD")
            .await
            .unwrap();
        Command::new("git")
            .args(["-C", dir.path().to_str().unwrap(), "checkout", &sha])
            .output()
            .expect("git checkout failed");
        let repo = git2::Repository::open(dir.path()).unwrap();
        let name = head_name(repo).await.unwrap();
        assert_eq!(name, None);
    }

    #[tokio::test]
    async fn test_is_dirty_clean() {
        let dir = tempdir().unwrap();
        let repo = create_repo_with_branch(dir.path(), "main");
        assert!(!is_dirty(repo).await.unwrap());
    }

    #[tokio::test]
    async fn test_is_dirty_with_changes() {
        let dir = tempdir().unwrap();
        let repo = create_repo_with_branch(dir.path(), "main");
        std::fs::write(dir.path().join("new.txt"), "hello").unwrap();
        assert!(is_dirty(repo).await.unwrap());
    }

    #[tokio::test]
    async fn test_checkout() {
        let dir = tempdir().unwrap();
        let repo = create_repo_with_branch(dir.path(), "main");
        // create a second branch
        Command::new("git")
            .args([
                "-C",
                dir.path().to_str().unwrap(),
                "checkout",
                "-b",
                "feature",
            ])
            .output()
            .expect("git checkout failed");
        Command::new("git")
            .args([
                "-C",
                dir.path().to_str().unwrap(),
                "commit",
                "--allow-empty",
                "-m",
                "second",
                "--no-gpg-sign",
            ])
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .output()
            .expect("git commit failed");

        // checkout back to main
        checkout(repo, "main").await.unwrap();
        let repo = git2::Repository::open(dir.path()).unwrap();
        let name = head_name(repo).await.unwrap();
        assert_eq!(name, Some("main".to_string()));
    }

    #[tokio::test]
    async fn test_ahead_behind() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        Command::new("git")
            .args(["init", "--initial-branch", "main", path.as_str()])
            .output()
            .expect("git init failed");
        Command::new("git")
            .args([
                "-C",
                path.as_str(),
                "commit",
                "--allow-empty",
                "-m",
                "first",
                "--no-gpg-sign",
            ])
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .output()
            .expect("git commit failed");

        Command::new("git")
            .args(["-C", path.as_str(), "checkout", "-b", "feature"])
            .output()
            .expect("git checkout failed");
        Command::new("git")
            .args([
                "-C",
                path.as_str(),
                "commit",
                "--allow-empty",
                "-m",
                "second",
                "--no-gpg-sign",
            ])
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .output()
            .expect("git commit failed");

        let repo = git2::Repository::open(dir.path()).unwrap();
        let (ahead, behind) = ahead_behind(repo, "main").await.unwrap();
        assert_eq!(ahead, 1);
        assert_eq!(behind, 0);
    }

    #[tokio::test]
    async fn test_rebase_returns_error() {
        let dir = tempdir().unwrap();
        let repo = create_repo_with_branch(dir.path(), "main");
        assert!(rebase(repo, "nonexistent").await.is_err());
    }
}
