// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{Error, ProgressCallback};

/// Push refs to a remote.
///
/// # Errors
///
/// Returns an error if the push fails.
pub async fn push(
    repo: git2::Repository,
    remote: &str,
    refspecs: &[String],
    _progress: &dyn ProgressCallback,
) -> Result<(), Error> {
    let remote = remote.to_string();
    let refspecs: Vec<String> = refspecs.to_vec();
    tokio::task::spawn_blocking(move || -> Result<(), git2::Error> {
        let mut remote = repo.find_remote(&remote)?;
        let specs: Vec<&str> = refspecs.iter().map(|s| s.as_str()).collect();
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(|_url, username_from_url, allowed_types| {
            credentials_callback(_url, username_from_url, allowed_types)
        });
        let mut opts = git2::PushOptions::new();
        opts.remote_callbacks(callbacks);
        remote.push(&specs, Some(&mut opts))?;
        Ok(())
    })
    .await
    .map_err(|e| Error::Git2(e.to_string()))?
    .map_err(Error::from)
}

/// Credential callback used by push and fetch operations.
pub fn credentials_callback(
    _url: &str,
    username_from_url: Option<&str>,
    allowed_types: git2::CredentialType,
) -> Result<git2::Cred, git2::Error> {
    if allowed_types.contains(git2::CredentialType::SSH_KEY) {
        // Try SSH agent first
        if let Ok(cred) = git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git")) {
            return Ok(cred);
        }
        // Fall back to default key paths
        let username = username_from_url.unwrap_or("git");
        for key_path in ["~/.ssh/id_ed25519", "~/.ssh/id_rsa"] {
            let expanded = shellexpand::tilde(key_path);
            if std::path::Path::new(expanded.as_ref()).exists() {
                return git2::Cred::ssh_key(username, None, std::path::Path::new(expanded.as_ref()), None);
            }
        }
    }
    if allowed_types.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
        let username = username_from_url.unwrap_or("");
        // Check for credentials from environment variables
        if let (Ok(user), Ok(pass)) = (std::env::var("REPO_RS_TEST_USER"), std::env::var("REPO_RS_TEST_PASS")) {
            return git2::Cred::userpass_plaintext(&user, &pass);
        }
        // Check for credentials from git config
        if let (Some(user), Some(pass)) = (get_git_config("http.user"), get_git_config("http.password")) {
            return git2::Cred::userpass_plaintext(&user, &pass);
        }
        // Fallback: try anonymous
        return git2::Cred::userpass_plaintext(username, "");
    }
    Err(git2::Error::from_str("no suitable credentials found"))
}

fn get_git_config(key: &str) -> Option<String> {
    std::process::Command::new("git")
        .args(["config", "--global", key])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_push_returns_error_on_missing_remote() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let repo = git2::Repository::init(&path).unwrap();

        let err = push(repo, "origin", &[], &()).await.unwrap_err();
        assert!(matches!(err, Error::Git2(_)));
    }
}
