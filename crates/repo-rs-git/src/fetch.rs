// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{Error, ProgressCallback};

/// Fetch updates for a repository from its default remote.
///
/// # Errors
///
/// Returns an error if the fetch fails.
pub async fn fetch(
    repo: git2::Repository,
    remote: &str,
    _prune: bool,
    _tags: bool,
    _depth: Option<u32>,
    _progress: &dyn ProgressCallback,
) -> Result<(), Error> {
    let remote = remote.to_string();
    tokio::task::spawn_blocking(move || -> Result<(), git2::Error> {
        let mut remote = repo.find_remote(&remote)?;
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(|_url, username_from_url, allowed_types| {
            super::push::credentials_callback(_url, username_from_url, allowed_types)
        });
        let mut opts = git2::FetchOptions::new();
        opts.remote_callbacks(callbacks);
        remote.fetch(&[] as &[&str], Some(&mut opts), None)?;
        Ok(())
    })
    .await
    .map_err(|e| Error::Git2(e.to_string()))?
    .map_err(Error::from)
}

/// Fetch a specific ref from a remote.
///
/// # Errors
///
/// Returns an error if the fetch fails.
pub async fn fetch_refspec(
    repo: git2::Repository,
    remote: &str,
    refspec: &str,
    _progress: &dyn ProgressCallback,
) -> Result<(), Error> {
    let remote = remote.to_string();
    let refspec = refspec.to_string();
    tokio::task::spawn_blocking(move || -> Result<(), git2::Error> {
        let mut remote = repo.find_remote(&remote)?;
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(|_url, username_from_url, allowed_types| {
            super::push::credentials_callback(_url, username_from_url, allowed_types)
        });
        let mut opts = git2::FetchOptions::new();
        opts.remote_callbacks(callbacks);
        remote.fetch(&[&refspec], Some(&mut opts), None)?;
        Ok(())
    })
    .await
    .map_err(|e| Error::Git2(e.to_string()))?
    .map_err(Error::from)
}
