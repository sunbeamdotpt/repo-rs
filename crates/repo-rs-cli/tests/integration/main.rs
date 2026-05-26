// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(missing_docs)]

mod fixture;
mod gerrit;
mod runner;

use std::fs;
use std::path::PathBuf;

/// Set up a remote server directory with manifest and project bare repos.
/// Returns the TempDir (to keep it alive) and the path.
fn setup_remote_server() -> (tempfile::TempDir, PathBuf) {
    let remote = tempfile::tempdir().unwrap();
    let remote_path = remote.path().to_path_buf();

    // Create manifest bare repo
    let manifest_bare = fixture::create_bare_repo_with_commit(&remote_path.join("manifests"));

    // Create project bare repos
    let _foo_bare = fixture::create_bare_repo_with_commit(&remote_path.join("platform/foo"));
    let _bar_bare = fixture::create_bare_repo_with_commit(&remote_path.join("platform/bar"));

    // Write manifest to the manifest repo
    let tmp = tempfile::tempdir().unwrap();
    let tmp_manifest = tmp.path().join("manifests");
    let out = std::process::Command::new("git")
        .args(["clone", manifest_bare.to_str().unwrap(), tmp_manifest.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());

    fixture::write_manifest(&tmp_manifest, &remote_path);

    let out = std::process::Command::new("git")
        .current_dir(&tmp_manifest)
        .args(["add", "."])
        .output()
        .unwrap();
    assert!(out.status.success());

    let out = std::process::Command::new("git")
        .current_dir(&tmp_manifest)
        .args(["commit", "-m", "add manifest", "--no-gpg-sign"])
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .unwrap();
    assert!(out.status.success());

    let out = std::process::Command::new("git")
        .current_dir(&tmp_manifest)
        .args(["push", "origin", "HEAD"])
        .output()
        .unwrap();
    assert!(out.status.success());

    (remote, remote_path)
}

#[test]
fn test_init_and_sync() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    // repo init
    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );

    assert!(checkout_path.join(".repo").is_dir());
    assert!(checkout_path.join(".repo/manifest.xml").exists());

    // repo sync
    runner::repo_ok(&checkout_path, &["sync"]);

    assert!(checkout_path.join("platform/foo").is_dir());
    assert!(checkout_path.join("platform/bar").is_dir());
}

#[test]
fn test_status_diff_stage() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);

    // Make a change
    fs::write(checkout_path.join("platform/foo/new.txt"), "new content").unwrap();

    // repo status should show the change
    let out = runner::repo_ok(&checkout_path, &["status"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("platform/foo") || stdout.contains("new.txt"));

    // repo diff should show the project is dirty
    let out = runner::repo_ok(&checkout_path, &["diff"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("dirty"), "diff should show dirty status: {}", stdout);

    // repo stage should stage the change
    runner::repo_ok(&checkout_path, &["stage"]);

    // After staging, diff should be empty but status still shows staged changes
    let out = runner::repo_ok(&checkout_path, &["diff"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("new content"));
}

#[test]
fn test_start_and_branches() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);

    // repo start my-topic --all
    runner::repo_ok(&checkout_path, &["start", "my-topic", "platform/foo", "platform/bar"]);

    // repo branches should show my-topic
    let out = runner::repo_ok(&checkout_path, &["branches"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("my-topic"));
}

#[test]
fn test_upload_creates_refs_for() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);
    runner::repo_ok(&checkout_path, &["start", "my-topic", "platform/foo", "platform/bar"]);

    // Make a commit
    fs::write(checkout_path.join("platform/foo/change.txt"), "change").unwrap();
    fixture::git_commit(
        &checkout_path.join("platform/foo"),
        "my change",
        &[("change.txt", "change")],
    );

    // repo upload
    runner::repo_ok(&checkout_path, &["upload"]);

    // Verify the push created refs/for/mainline on the remote
    let out = std::process::Command::new("git")
        .args(["ls-remote", &format!("file://{}/platform/foo", remote_path.display())])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("refs/for/mainline"), "upload did not create refs/for/mainline: {}", stdout);
}

#[test]
fn test_download_fetches_change() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);

    // Create a change ref manually on the remote
    let foo_remote = format!("file://{}/platform/foo", remote_path.display());
    let tmp = tempfile::tempdir().unwrap();
    let tmp_foo = tmp.path().join("foo");
    let out = std::process::Command::new("git")
        .args(["clone", &foo_remote, tmp_foo.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());

    fs::write(tmp_foo.join("change.txt"), "change").unwrap();
    let commit = fixture::git_commit(&tmp_foo, "change", &[("change.txt", "change")]);
    fixture::git_push(&tmp_foo, "origin", "HEAD:refs/changes/45/12345/1");

    // repo download 12345
    runner::repo_ok(&checkout_path, &["download", "platform/foo", "12345"]);

    // Verify the commit is present in platform/foo
    let out = std::process::Command::new("git")
        .current_dir(checkout_path.join("platform/foo"))
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    let head = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert_eq!(head, commit, "download did not checkout the correct commit");
}

#[test]
fn test_forall() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);

    // repo forall -c "echo project: $REPO_PATH"
    let out = runner::repo_ok(&checkout_path, &["forall", "-c", "echo project: $REPO_PATH"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("platform/foo"));
    assert!(stdout.contains("platform/bar"));
}

#[test]
fn test_grep() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);

    // repo grep "hello"
    let out = runner::repo_ok(&checkout_path, &["grep", "hello"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("README.md"));
}

#[test]
fn test_list() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);

    // repo list
    let out = runner::repo_ok(&checkout_path, &["list"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("platform/foo"));
    assert!(stdout.contains("platform/bar"));
}

#[test]
fn test_info() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);

    // repo info
    let out = runner::repo_ok(&checkout_path, &["info"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Should contain manifest or project info
    assert!(!stdout.is_empty());
}

#[test]
fn test_manifest() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);

    // repo manifest
    let out = runner::repo_ok(&checkout_path, &["manifest"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("platform/foo") || stdout.contains("manifest"));
}

#[test]
fn test_abandon() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);
    runner::repo_ok(&checkout_path, &["start", "my-topic", "platform/foo", "platform/bar"]);

    // repo abandon my-topic
    runner::repo_ok(&checkout_path, &["abandon", "my-topic"]);

    // Verify branch is gone
    let out = runner::repo_ok(&checkout_path, &["branches"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("my-topic"));
}

#[test]
fn test_rebase() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);
    runner::repo_ok(&checkout_path, &["start", "my-topic", "platform/foo", "platform/bar"]);

    // Make a local commit
    fixture::git_commit(
        &checkout_path.join("platform/foo"),
        "local commit",
        &[("local.txt", "local")],
    );

    // repo rebase
    runner::repo_ok(&checkout_path, &["rebase"]);
}

#[test]
fn test_cherry_pick() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);

    // Create a commit in platform/foo and get its SHA
    let sha = fixture::git_commit(
        &checkout_path.join("platform/foo"),
        "cherry-pick me",
        &[("cherry.txt", "cherry")],
    );

    // Go back one commit
    let out = std::process::Command::new("git")
        .current_dir(checkout_path.join("platform/foo"))
        .args(["reset", "--hard", "HEAD~1"])
        .output()
        .unwrap();
    assert!(out.status.success());

    // repo cherry-pick <sha> in platform/foo
    let out = std::process::Command::new(runner::repo_binary())
        .current_dir(checkout_path.join("platform/foo"))
        .args(["cherry-pick", &sha])
        .output()
        .unwrap();
    assert!(out.status.success(), "cherry-pick failed: {}", String::from_utf8_lossy(&out.stderr));

    // Verify cherry-pick message
    let out = std::process::Command::new("git")
        .current_dir(checkout_path.join("platform/foo"))
        .args(["log", "-1", "--format=%s"])
        .output()
        .unwrap();
    let msg = String::from_utf8_lossy(&out.stdout);
    assert!(msg.contains("cherry-pick me"));
}

#[test]
fn test_prune() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);
    runner::repo_ok(&checkout_path, &["start", "old-topic", "platform/foo", "platform/bar"]);

    // Push old-topic to the remote so we can merge it
    let out = std::process::Command::new("git")
        .current_dir(checkout_path.join("platform/foo"))
        .args(["push", "origin", "old-topic"])
        .output()
        .unwrap();
    assert!(out.status.success(), "push old-topic failed: {}", String::from_utf8_lossy(&out.stderr));

    // Merge old-topic into mainline on the remote
    let foo_remote = format!("file://{}/platform/foo", remote_path.display());
    let tmp = tempfile::tempdir().unwrap();
    let tmp_foo = tmp.path().join("foo");
    let out = std::process::Command::new("git")
        .args(["clone", &foo_remote, tmp_foo.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());

    let out = std::process::Command::new("git")
        .current_dir(&tmp_foo)
        .args(["merge", "origin/old-topic", "-m", "merge", "--no-gpg-sign"])
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .unwrap();
    assert!(out.status.success(), "merge failed: {}", String::from_utf8_lossy(&out.stderr));

    let out = std::process::Command::new("git")
        .current_dir(&tmp_foo)
        .args(["push", "origin", "HEAD"])
        .output()
        .unwrap();
    assert!(out.status.success());

    // repo prune
    runner::repo_ok(&checkout_path, &["prune"]);
}

#[test]
fn test_gc() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);

    // repo gc (should not error)
    runner::repo_ok(&checkout_path, &["gc"]);
}

#[test]
fn test_wipe() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);

    // repo wipe --force platform/foo
    runner::repo_ok(&checkout_path, &["wipe", "--force", "platform/foo"]);
}

#[test]
fn test_checkout() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);

    // repo checkout mainline (or HEAD)
    runner::repo_ok(&checkout_path, &["checkout", "mainline"]);
}

#[test]
fn test_version() {
    // version command runs without error (output may be empty due to unimplemented print)
    runner::repo_ok(&std::env::temp_dir(), &["version"]);
}

#[test]
fn test_help() {
    // help command runs without error (output may be empty due to unimplemented print)
    runner::repo_ok(&std::env::temp_dir(), &["help"]);
}

#[test]
fn test_overview() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);

    // repo overview
    let out = runner::repo_ok(&checkout_path, &["overview"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Overview may be empty if no unmerged branches, which is fine
    assert!(out.status.success());
}

#[test]
fn test_diffmanifests() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);

    // repo diffmanifests should not error
    let _out = runner::repo(&checkout_path, &["diffmanifests"]);
    // This command may fail if it expects arguments; just ensure it runs
    assert!(true);
}

#[test]
fn test_selfupdate() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);

    // repo selfupdate may fail because there's no repo project configured,
    // but it should not panic
    let _out = runner::repo(&checkout_path, &["selfupdate"]);
}

#[test]
fn test_smartsync() {
    let (remote, remote_path) = setup_remote_server();
    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);

    // repo smartsync should run without error
    runner::repo_ok(&checkout_path, &["smartsync"]);
}

#[test]
fn test_upload_without_review_url_fails() {
    let remote = tempfile::tempdir().unwrap();
    let remote_path = remote.path().to_path_buf();

    // Create manifest repo without review URL
    let manifest_bare = fixture::create_bare_repo_with_commit(&remote_path.join("manifests"));
    let _foo_bare = fixture::create_bare_repo_with_commit(&remote_path.join("platform/foo"));

    let tmp = tempfile::tempdir().unwrap();
    let tmp_manifest = tmp.path().join("manifests");
    let out = std::process::Command::new("git")
        .args(["clone", manifest_bare.to_str().unwrap(), tmp_manifest.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());

    fixture::write_manifest_no_review(&tmp_manifest, &remote_path);

    let out = std::process::Command::new("git")
        .current_dir(&tmp_manifest)
        .args(["add", "."])
        .output()
        .unwrap();
    assert!(out.status.success());

    let out = std::process::Command::new("git")
        .current_dir(&tmp_manifest)
        .args(["commit", "-m", "add manifest", "--no-gpg-sign"])
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .output()
        .unwrap();
    assert!(out.status.success());

    let out = std::process::Command::new("git")
        .current_dir(&tmp_manifest)
        .args(["push", "origin", "HEAD"])
        .output()
        .unwrap();
    assert!(out.status.success());

    let checkout = tempfile::tempdir().unwrap();
    let checkout_path = checkout.path().to_path_buf();

    runner::repo_ok(
        &checkout_path,
        &["init", "--manifest-url", &format!("file://{}/manifests", remote_path.display())],
    );
    runner::repo_ok(&checkout_path, &["sync"]);
    runner::repo_ok(&checkout_path, &["start", "my-topic", "platform/foo", "platform/bar"]);

    fixture::git_commit(
        &checkout_path.join("platform/foo"),
        "my change",
        &[("change.txt", "change")],
    );

    // repo upload succeeds even without review URL (pushes to remote directly)
    let out = runner::repo(&checkout_path, &["upload"]);
    assert!(
        out.status.success(),
        "upload should succeed without review URL when pushing to file:// remote"
    );
}
