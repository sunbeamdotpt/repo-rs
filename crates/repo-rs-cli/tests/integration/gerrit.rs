// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(missing_docs)]

use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::Duration;

const GERRIT_HTTP: &str = "http://localhost:8080";
const COMPOSE_FILE: &str = "tests/docker-compose.gerrit.yml";

fn compose_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(COMPOSE_FILE)
}

/// Start the Gerrit container and wait for it to be healthy.
pub fn start() {
    stop(); // clean up any stale container

    let out = Command::new("docker")
        .args([
            "compose",
            "-f",
            compose_path().to_str().unwrap(),
            "up",
            "-d",
            "--wait",
        ])
        .output()
        .expect("docker compose up failed; is Docker running?");
    assert!(
        out.status.success(),
        "docker compose up failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Wait a bit more for Gerrit SSH to be ready
    std::thread::sleep(Duration::from_secs(5));
}

/// Stop and remove the Gerrit container.
pub fn stop() {
    let _ = Command::new("docker")
        .args([
            "compose",
            "-f",
            compose_path().to_str().unwrap(),
            "down",
            "-v",
        ])
        .output();
}

/// Query Gerrit REST API for open changes.
/// Returns `None` if Gerrit is not running.
pub fn query_changes(query: &str) -> Option<serde_json::Value> {
    let url = format!("{}/changes/?q={}", GERRIT_HTTP, query);
    let out = Command::new("curl")
        .args(["-s", "-f", &url])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    let text = text.strip_prefix(")]}'\n").unwrap_or(&text);
    serde_json::from_str(text).ok()
}
