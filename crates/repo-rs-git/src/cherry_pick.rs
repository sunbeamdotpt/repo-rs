// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::Error;

/// Cherry-pick a commit onto the current branch.
///
/// The commit message is amended to remove any `Change-Id:` trailer and to
/// append a `(cherry picked from commit …)` reference.
pub async fn cherry_pick(repo: git2::Repository, sha: &str) -> Result<(), Error> {
    let sha = sha.to_string();
    tokio::task::spawn_blocking(move || -> Result<(), git2::Error> {
        let obj = repo.revparse_single(&sha)?;
        let commit = obj.peel_to_commit()?;
        let head = repo.head()?.peel_to_commit()?;

        let mut opts = git2::MergeOptions::new();
        let mut index = repo.cherrypick_commit(&commit, &head, 0, Some(&mut opts))?;

        if index.has_conflicts() {
            return Err(git2::Error::new(
                git2::ErrorCode::Conflict,
                git2::ErrorClass::Checkout,
                "cherry-pick resulted in conflicts",
            ));
        }

        let tree_id = index.write_tree_to(&repo)?;
        let tree = repo.find_tree(tree_id)?;
        let sig = repo.signature()?;
        let old_msg = commit.message().unwrap_or("");
        let full_sha = commit.id().to_string();
        let new_msg = format_cherry_pick_message(old_msg, &full_sha);

        let new_oid = repo.commit(None, &sig, &sig, &new_msg, &tree, &[&head])?;
        let mut head_ref = repo.head()?;
        head_ref.set_target(new_oid, "cherry-pick")?;
        Ok(())
    })
    .await
    .map_err(|e| Error::Git2(e.to_string()))?
    .map_err(Error::from)
}

/// Strip `Change-Id:` trailers and append the cherry-pick reference.
fn format_cherry_pick_message(old_msg: &str, sha: &str) -> String {
    let mut lines: Vec<&str> = old_msg
        .lines()
        .filter(|line| !line.trim().starts_with("Change-Id:"))
        .collect();

    // Trim trailing blank lines.
    while let Some(last) = lines.last() {
        if last.trim().is_empty() {
            lines.pop();
        } else {
            break;
        }
    }

    let mut msg = lines.join("\n");
    if !msg.is_empty() {
        msg.push('\n');
    }
    msg.push_str(&format!("(cherry picked from commit {sha})"));
    msg
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use tempfile::tempdir;

    fn make_repo_with_commit(dir: &std::path::Path, msg: &str) -> (git2::Repository, String) {
        let path = Utf8PathBuf::from_path_buf(dir.to_path_buf()).unwrap();
        let repo = git2::Repository::init(&path).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();

        // Create initial commit
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        let parent_oid = repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[]).unwrap();
        drop(tree);

        // Create file for second commit
        std::fs::write(dir.join("file.txt"), "content").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("file.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.find_commit(parent_oid).unwrap();
        let oid = repo
            .commit(Some("HEAD"), &sig, &sig, msg, &tree, &[&parent])
            .unwrap();
        drop(tree);
        drop(parent);

        (repo, oid.to_string())
    }

    #[tokio::test]
    async fn test_cherry_pick_basic() {
        let dir = tempdir().unwrap();
        let (repo, sha) = make_repo_with_commit(dir.path(), "add file\n\nChange-Id: I1234567890abcdef1234567890abcdef12345678");

        // Detach HEAD back to the initial commit so we can create a sibling branch
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        let initial = head.parent(0).unwrap();
        let mut head_ref = repo.head().unwrap();
        head_ref.set_target(initial.id(), "reset to initial").unwrap();
        drop(head);
        drop(head_ref);

        // Create a sibling commit with a different file so cherry-pick is meaningful
        std::fs::write(dir.path().join("other.txt"), "other").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("other.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let initial_commit = repo.find_commit(initial.id()).unwrap();
        let _new_head = repo.commit(Some("HEAD"), &sig, &sig, "sibling", &tree, &[&initial_commit]).unwrap();
        drop(tree);
        drop(initial_commit);
        drop(initial);

        cherry_pick(repo, &sha).await.unwrap();

        let repo = git2::Repository::open(dir.path()).unwrap();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        let msg = head.message().unwrap();
        assert!(msg.contains("add file"));
        assert!(!msg.contains("Change-Id:"));
        assert!(msg.contains("(cherry picked from commit"));
        assert!(msg.contains(&sha));
    }

    #[tokio::test]
    async fn test_cherry_pick_conflict() {
        let dir = tempdir().unwrap();
        let (repo, sha) = make_repo_with_commit(dir.path(), "add file");

        // Detach HEAD back to initial and create a sibling with conflicting content
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        let initial = head.parent(0).unwrap();
        let mut head_ref = repo.head().unwrap();
        head_ref.set_target(initial.id(), "reset to initial").unwrap();
        drop(head);
        drop(head_ref);

        std::fs::write(dir.path().join("file.txt"), "conflict").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("file.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let initial_commit = repo.find_commit(initial.id()).unwrap();
        let _new_head = repo.commit(Some("HEAD"), &sig, &sig, "conflicting", &tree, &[&initial_commit]).unwrap();
        drop(tree);
        drop(initial_commit);
        drop(initial);

        let err = cherry_pick(repo, &sha).await.unwrap_err();
        assert!(err.to_string().contains("conflict"));
    }

    #[test]
    fn test_format_cherry_pick_message() {
        let msg = "Hello world\n\nChange-Id: I123\n\nSigned-off-by: Foo\n";
        let result = format_cherry_pick_message(msg, "abc123");
        assert!(!result.contains("Change-Id:"));
        assert!(result.contains("Hello world"));
        assert!(result.contains("(cherry picked from commit abc123)"));
    }

    #[test]
    fn test_format_cherry_pick_message_no_change_id() {
        let msg = "Hello world\n\nSigned-off-by: Foo\n";
        let result = format_cherry_pick_message(msg, "abc123");
        assert!(result.contains("Hello world"));
        assert!(result.contains("(cherry picked from commit abc123)"));
    }
}
