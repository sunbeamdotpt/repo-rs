// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::Error;

/// List remote refs matching a pattern.
///
/// Returns a vector of `(ref_name, target_oid)` pairs.
pub async fn list_remote_refs(
    repo: git2::Repository,
    remote: &str,
    pattern: &str,
) -> Result<Vec<(String, String)>, Error> {
    let remote = remote.to_string();
    let pattern = pattern.to_string();
    tokio::task::spawn_blocking(move || -> Result<Vec<(String, String)>, git2::Error> {
        let mut remote = repo.find_remote(&remote)?;
        remote.connect(git2::Direction::Fetch)?;
        let refs = remote.list()?;
        let mut results = Vec::new();
        for remote_ref in refs {
            let name = remote_ref.name();
            if name.contains(&pattern) {
                results.push((name.to_string(), remote_ref.oid().to_string()));
            }
        }
        Ok(results)
    })
    .await
    .map_err(|e| Error::Git2(e.to_string()))?
    .map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_list_remote_refs_on_empty_repo() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let repo = git2::Repository::init_bare(&path).unwrap();

        // A bare repo with no remotes should error.
        let err = list_remote_refs(repo, "origin", "refs/heads/")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("origin") || err.to_string().contains("remote"));
    }

    fn make_empty_commit(repo: &git2::Repository, ref_name: &str) -> git2::Oid {
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let mut index = repo.index().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some(ref_name), &sig, &sig, "test", &tree, &[])
            .unwrap()
    }

    #[tokio::test]
    async fn test_list_remote_refs_finds_matching_refs() {
        let remote_dir = tempdir().unwrap();
        let remote_path = remote_dir.path();
        let remote_repo = git2::Repository::init_bare(remote_path).unwrap();
        let commit_id = make_empty_commit(&remote_repo, "refs/heads/main");

        // Create a local repo that has the bare repo as remote.
        let local_dir = tempdir().unwrap();
        let local_path = local_dir.path();
        let local_repo = git2::Repository::init(local_path).unwrap();
        local_repo
            .remote("origin", remote_path.to_str().unwrap())
            .unwrap();

        let refs = list_remote_refs(local_repo, "origin", "refs/heads/")
            .await
            .unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].0, "refs/heads/main");
        assert_eq!(refs[0].1, commit_id.to_string());
    }

    #[tokio::test]
    async fn test_list_remote_refs_no_match() {
        let remote_dir = tempdir().unwrap();
        let remote_path = remote_dir.path();
        let remote_repo = git2::Repository::init_bare(remote_path).unwrap();
        make_empty_commit(&remote_repo, "refs/heads/main");

        let local_dir = tempdir().unwrap();
        let local_path = local_dir.path();
        let local_repo = git2::Repository::init(local_path).unwrap();
        local_repo
            .remote("origin", remote_path.to_str().unwrap())
            .unwrap();

        let refs = list_remote_refs(local_repo, "origin", "refs/tags/")
            .await
            .unwrap();
        assert!(refs.is_empty());
    }
}
