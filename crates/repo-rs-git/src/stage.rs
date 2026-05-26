// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::Error;

/// Stage all changes in the working tree (equivalent to `git add -A`).
pub async fn add_all(repo: git2::Repository) -> Result<(), Error> {
    tokio::task::spawn_blocking(move || -> Result<(), git2::Error> {
        let mut index = repo.index()?;
        index.add_all(&["."], git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;
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
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_add_all_stages_new_file() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let repo = git2::Repository::init(&path).unwrap();

        // Need an initial commit so HEAD exists
        {
            let sig = git2::Signature::now("Test", "test@example.com").unwrap();
            let tree_id = {
                let mut index = repo.index().unwrap();
                index.write_tree().unwrap()
            };
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
                .unwrap();
        }

        // Create a new file
        std::fs::write(dir.path().join("new.txt"), "hello").unwrap();

        // Stage it
        add_all(repo).await.unwrap();

        // Verify it's staged
        let repo = git2::Repository::open(dir.path()).unwrap();
        let index = repo.index().unwrap();
        let entry = index.get_path(std::path::Path::new("new.txt"), 0);
        assert!(entry.is_some());
    }

    #[tokio::test]
    async fn test_add_all_stages_modification() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let repo = git2::Repository::init(&path).unwrap();

        std::fs::write(dir.path().join("file.txt"), "v1").unwrap();
        {
            let sig = git2::Signature::now("Test", "test@example.com").unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(std::path::Path::new("file.txt")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
                .unwrap();
        }

        // Modify the file
        std::fs::write(dir.path().join("file.txt"), "v2").unwrap();

        add_all(repo).await.unwrap();

        let repo = git2::Repository::open(dir.path()).unwrap();
        let index = repo.index().unwrap();
        let entry = index.get_path(std::path::Path::new("file.txt"), 0).unwrap();
        let blob = repo.find_blob(entry.id).unwrap();
        let content = std::str::from_utf8(blob.content()).unwrap();
        assert_eq!(content, "v2");
    }
}
