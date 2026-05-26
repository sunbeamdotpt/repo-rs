// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(missing_docs)]

use camino::Utf8PathBuf;
use std::fs;
use std::path::PathBuf;

/// Create a minimal default.xml manifest for integration tests.
///
/// The manifest references the given remote path and defines two
/// test projects: `platform/foo` and `platform/bar`.
pub fn write_manifest(manifests_dir: &PathBuf, remote_path: &PathBuf) {
    let remote_url = format!("file://{}", remote_path.display());
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="{}" />
  <default remote="origin" revision="mainline" />
  <project name="platform/foo" path="platform/foo" />
  <project name="platform/bar" path="platform/bar" />
</manifest>
"#,
        remote_url
    );
    fs::write(manifests_dir.join("default.xml"), xml).unwrap();
}

/// Write a manifest without a `review` URL.
pub fn write_manifest_no_review(manifests_dir: &PathBuf, remote_path: &PathBuf) {
    let remote_url = format!("file://{}", remote_path.display());
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="{}" />
  <default remote="origin" revision="mainline" />
  <project name="platform/foo" path="platform/foo" />
</manifest>
"#,
        remote_url
    );
    fs::write(manifests_dir.join("default.xml"), xml).unwrap();
}

/// Create a `.repo/manifest.xml` wrapper pointing to `name`.
pub fn write_manifest_wrapper(repo_dir: &PathBuf, name: &str) {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <include name="{}" />
</manifest>
"#,
        name
    );
    fs::write(repo_dir.join("manifest.xml"), xml).unwrap();
}

/// Initialize a fresh repo checkout directory with a cloned manifest repo.
pub fn init_repo_checkout(
    root: &PathBuf,
    manifests_git: &PathBuf,
    remote_path: &PathBuf,
) -> PathBuf {
    let repo_dir = root.join(".repo");
    fs::create_dir_all(&repo_dir).unwrap();
    fs::create_dir_all(repo_dir.join("projects")).unwrap();
    fs::create_dir_all(repo_dir.join("project-objects")).unwrap();
    fs::create_dir_all(repo_dir.join("local_manifests")).unwrap();

    // Clone the manifest repo into .repo/manifests
    let manifest_dst = repo_dir.join("manifests");
    let out = std::process::Command::new("git")
        .args(["clone", manifests_git.to_str().unwrap(), manifest_dst.to_str().unwrap()])
        .output()
        .expect("git clone failed");
    assert!(out.status.success(), "git clone failed: {}", String::from_utf8_lossy(&out.stderr));

    write_manifest(&manifest_dst, remote_path);
    write_manifest_wrapper(&repo_dir, "default.xml");

    root.clone()
}

/// Create a commit in a git repo using the system git CLI.
pub fn git_commit(repo: &PathBuf, message: &str, files: &[(&str, &str)]) -> String {
    for (path, content) in files {
        let file_path = repo.join(path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&file_path, content).unwrap();
    }

    let out = std::process::Command::new("git")
        .current_dir(repo)
        .args(["add", "."])
        .output()
        .unwrap();
    assert!(out.status.success());

    let out = std::process::Command::new("git")
        .current_dir(repo)
        .args(["commit", "-m", message, "--no-gpg-sign"])
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .unwrap();
    assert!(out.status.success(), "git commit failed: {}", String::from_utf8_lossy(&out.stderr));

    let out = std::process::Command::new("git")
        .current_dir(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// Push a ref to a remote.
pub fn git_push(repo: &PathBuf, remote: &str, refspec: &str) {
    let out = std::process::Command::new("git")
        .current_dir(repo)
        .args(["push", remote, refspec])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git push failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Create a bare repo with an initial commit.
pub fn create_bare_repo_with_commit(path: &PathBuf) -> PathBuf {
    let bare_path = path.clone();
    fs::create_dir_all(&bare_path).unwrap();
    let out = std::process::Command::new("git")
        .args(["init", "--bare", bare_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());

    let tmp = tempfile::tempdir().unwrap();
    let tmp_path = tmp.path().join("work");
    let out = std::process::Command::new("git")
        .args(["clone", bare_path.to_str().unwrap(), tmp_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());

    fs::write(tmp_path.join("README.md"), "hello").unwrap();
    let out = std::process::Command::new("git")
        .current_dir(&tmp_path)
        .args(["add", "."])
        .output()
        .unwrap();
    assert!(out.status.success());

    let out = std::process::Command::new("git")
        .current_dir(&tmp_path)
        .args(["commit", "-m", "initial", "--no-gpg-sign"])
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .unwrap();
    assert!(out.status.success());

    let out = std::process::Command::new("git")
        .current_dir(&tmp_path)
        .args(["push", "origin", "HEAD"])
        .output()
        .unwrap();
    assert!(out.status.success());

    bare_path
}
