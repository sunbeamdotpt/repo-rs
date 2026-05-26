// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(missing_docs)]

use std::path::PathBuf;
use std::process::{Command, Output};

/// Path to the `repo` binary under test.
pub fn repo_binary() -> PathBuf {
    // Cargo sets this env var for integration tests
    std::env::var_os("CARGO_BIN_EXE_repo")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.push("..");
            path.push("..");
            path.push("target");
            path.push("debug");
            path.push("repo");
            path
        })
}

/// Run the repo binary with the given arguments in the given directory.
pub fn repo(cwd: &PathBuf, args: &[&str]) -> Output {
    Command::new(repo_binary())
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("failed to execute repo binary")
}

/// Run the repo binary and assert success.
pub fn repo_ok(cwd: &PathBuf, args: &[&str]) -> Output {
    let out = repo(cwd, args);
    if !out.status.success() {
        panic!(
            "repo {:?} failed in {}\nstdout: {}\nstderr: {}",
            args,
            cwd.display(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    out
}

/// Run the repo binary and assert failure.
pub fn repo_err(cwd: &PathBuf, args: &[&str]) -> Output {
    let out = repo(cwd, args);
    assert!(
        !out.status.success(),
        "repo {:?} expected to fail in {}\nstdout: {}\nstderr: {}",
        args,
        cwd.display(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}
