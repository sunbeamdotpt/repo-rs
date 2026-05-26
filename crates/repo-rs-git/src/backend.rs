// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use async_trait::async_trait;
use camino::Utf8PathBuf;
use std::future::Future;
use std::pin::Pin;
use url::Url;

use crate::{Error, ProgressCallback};

/// Reopen a git2 repository by path so it can be moved into spawn_blocking.
fn reopen(repo: &git2::Repository) -> git2::Repository {
    let path = repo
        .workdir()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| repo.path().to_path_buf());
    git2::Repository::open(&path).expect("failed to reopen repository")
}

/// Async trait abstracting all git operations.
///
/// This trait allows swapping the real `git2` backend with a mock
/// for testing.
#[async_trait]
#[cfg_attr(any(test, feature = "test-util"), mockall::automock)]
#[allow(clippy::ref_option_ref)]
pub trait GitBackend: Send + Sync {
    /// Open an existing git repository.
    async fn open(&self, path: &Utf8PathBuf) -> Result<git2::Repository, Error>;

    /// Initialize a new non-bare git repository.
    async fn init(&self, path: &Utf8PathBuf) -> Result<git2::Repository, Error>;

    /// Initialize a new bare git repository.
    async fn init_bare(&self, path: &Utf8PathBuf) -> Result<git2::Repository, Error>;

    /// Clone a repository.
    async fn clone(
        &self,
        url: &Url,
        dst: &Utf8PathBuf,
        depth: Option<u32>,
        filter: Option<&str>,
        progress: &dyn ProgressCallback,
    ) -> Result<git2::Repository, Error>;

    /// Clone a repository as a bare mirror.
    async fn clone_bare(
        &self,
        url: &Url,
        dst: &Utf8PathBuf,
        progress: &dyn ProgressCallback,
    ) -> Result<git2::Repository, Error>;

    /// Fetch updates from a remote.
    async fn fetch(
        &self,
        repo: &git2::Repository,
        remote: &str,
        prune: bool,
        tags: bool,
        depth: Option<u32>,
        progress: &dyn ProgressCallback,
    ) -> Result<(), Error>;

    /// Fetch a specific refspec from a remote.
    async fn fetch_refspec(
        &self,
        repo: &git2::Repository,
        remote: &str,
        refspec: &str,
        progress: &dyn ProgressCallback,
    ) -> Result<(), Error>;

    /// Push refs to a remote.
    async fn push(
        &self,
        repo: &git2::Repository,
        remote: &str,
        refspecs: &[String],
        progress: &dyn ProgressCallback,
    ) -> Result<(), Error>;

    /// Read a config value as a string.
    async fn config_get_string(&self, repo: &git2::Repository, key: &str) -> Result<String, Error>;

    /// Read a config value as a boolean.
    async fn config_get_bool(&self, repo: &git2::Repository, key: &str) -> Result<Option<bool>, Error>;

    /// Set a git config value.
    async fn config_set(
        &self,
        repo: &mut git2::Repository,
        key: &str,
        value: &str,
    ) -> Result<(), Error>;

    /// Set a git config value in the local config file.
    async fn config_set_local(
        &self,
        repo: &mut git2::Repository,
        key: &str,
        value: &str,
    ) -> Result<(), Error>;

    /// Resolve a ref to its target SHA (full hex).
    async fn ref_resolve(&self, repo: &git2::Repository, name: &str) -> Result<String, Error>;

    /// List all refs matching an optional pattern.
    async fn ref_list(
        &self,
        repo: &git2::Repository,
        pattern: Option<&str>,
    ) -> Result<Vec<(String, String)>, Error>;

    /// Create or update a ref to point to a target object.
    async fn ref_update(
        &self,
        repo: &git2::Repository,
        name: &str,
        target: &str,
    ) -> Result<(), Error>;

    /// Checkout a target revision in the repository's worktree.
    async fn checkout(&self, repo: &git2::Repository, target: &str) -> Result<(), Error>;

    /// Return the name of the current branch, or `None` if HEAD is detached.
    async fn head_name(&self, repo: &git2::Repository) -> Result<Option<String>, Error>;

    /// Return `true` if the working tree has uncommitted changes.
    async fn is_dirty(&self, repo: &git2::Repository) -> Result<bool, Error>;

    /// Return `(ahead, behind)` counts between HEAD and the given upstream ref.
    async fn ahead_behind(&self, repo: &git2::Repository, upstream: &str) -> Result<(usize, usize), Error>;

    /// Rebase the current branch onto the given upstream.
    async fn rebase(&self, repo: &git2::Repository, upstream: &str) -> Result<(), Error>;

    /// Stage all changes in the working tree.
    async fn stage_all(&self, repo: &git2::Repository) -> Result<(), Error>;

    /// Cherry-pick a commit onto the current branch.
    async fn cherry_pick(&self, repo: &git2::Repository, sha: &str) -> Result<(), Error>;

    /// List remote refs matching a pattern.
    async fn list_remote_refs(
        &self,
        repo: &git2::Repository,
        remote: &str,
        pattern: &str,
    ) -> Result<Vec<(String, String)>, Error>;
}

/// Default [`GitBackend`] implementation using `git2`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultBackend;

#[async_trait]
impl GitBackend for DefaultBackend {
    async fn open(&self, path: &Utf8PathBuf) -> Result<git2::Repository, Error> {
        crate::open::open(path).await
    }

    async fn init(&self, path: &Utf8PathBuf) -> Result<git2::Repository, Error> {
        crate::open::init(path).await
    }

    async fn init_bare(&self, path: &Utf8PathBuf) -> Result<git2::Repository, Error> {
        crate::open::init_bare(path).await
    }

    async fn clone(
        &self,
        url: &Url,
        dst: &Utf8PathBuf,
        depth: Option<u32>,
        filter: Option<&str>,
        progress: &dyn ProgressCallback,
    ) -> Result<git2::Repository, Error> {
        crate::clone::clone(url, dst, depth, filter, progress).await
    }

    async fn clone_bare(
        &self,
        url: &Url,
        dst: &Utf8PathBuf,
        progress: &dyn ProgressCallback,
    ) -> Result<git2::Repository, Error> {
        crate::clone::clone_bare(url, dst, progress).await
    }

    fn fetch<'life0, 'life1, 'life2, 'life3, 'life4, 'async_trait>(
        &'life0 self,
        repo: &'life1 git2::Repository,
        remote: &'life2 str,
        _prune: bool,
        _tags: bool,
        _depth: Option<u32>,
        _progress: &'life3 dyn ProgressCallback,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        'life3: 'async_trait,
        Self: Sync,
    {
        let repo = reopen(repo);
        let remote = remote.to_string();
        Box::pin(async move { crate::fetch::fetch(repo, &remote, false, false, None, &()).await })
    }

    fn fetch_refspec<'life0, 'life1, 'life2, 'life3, 'life4, 'life5, 'async_trait>(
        &'life0 self,
        repo: &'life1 git2::Repository,
        remote: &'life2 str,
        refspec: &'life3 str,
        _progress: &'life4 dyn ProgressCallback,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        'life3: 'async_trait,
        'life4: 'async_trait,
        Self: Sync,
    {
        let repo = reopen(repo);
        let remote = remote.to_string();
        let refspec = refspec.to_string();
        Box::pin(async move { crate::fetch::fetch_refspec(repo, &remote, &refspec, &()).await })
    }

    fn push<'life0, 'life1, 'life2, 'life3, 'life4, 'life5, 'async_trait>(
        &'life0 self,
        repo: &'life1 git2::Repository,
        remote: &'life2 str,
        refspecs: &'life3 [String],
        _progress: &'life4 dyn ProgressCallback,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        'life3: 'async_trait,
        'life4: 'async_trait,
        Self: Sync,
    {
        let repo = reopen(repo);
        let remote = remote.to_string();
        let refspecs = refspecs.to_vec();
        Box::pin(async move { crate::push::push(repo, &remote, &refspecs, &()).await })
    }

    fn config_get_string<'life0, 'life1, 'life2, 'async_trait>(
        &'life0 self,
        repo: &'life1 git2::Repository,
        key: &'life2 str,
    ) -> Pin<Box<dyn Future<Output = Result<String, Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: Sync,
    {
        let repo = reopen(repo);
        let key = key.to_string();
        Box::pin(async move { crate::config::get_string(repo, &key).await })
    }

    fn config_get_bool<'life0, 'life1, 'life2, 'async_trait>(
        &'life0 self,
        repo: &'life1 git2::Repository,
        key: &'life2 str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<bool>, Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: Sync,
    {
        let repo = reopen(repo);
        let key = key.to_string();
        Box::pin(async move { crate::config::get_bool(repo, &key).await })
    }

    async fn config_set(
        &self,
        repo: &mut git2::Repository,
        key: &str,
        value: &str,
    ) -> Result<(), Error> {
        crate::config::set_string(repo, key, value).await
    }

    async fn config_set_local(
        &self,
        repo: &mut git2::Repository,
        key: &str,
        value: &str,
    ) -> Result<(), Error> {
        crate::config::set_string_local(repo, key, value).await
    }

    fn ref_resolve<'life0, 'life1, 'life2, 'async_trait>(
        &'life0 self,
        repo: &'life1 git2::Repository,
        name: &'life2 str,
    ) -> Pin<Box<dyn Future<Output = Result<String, Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: Sync,
    {
        let repo = reopen(repo);
        let name = name.to_string();
        Box::pin(async move { crate::refs::resolve(repo, &name).await })
    }

    fn ref_list<'life0, 'life1, 'life2, 'life3, 'async_trait>(
        &'life0 self,
        repo: &'life1 git2::Repository,
        pattern: Option<&'life2 str>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<(String, String)>, Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: Sync,
    {
        let repo = reopen(repo);
        let pattern = pattern.map(String::from);
        Box::pin(async move { crate::refs::list(repo, pattern.as_deref()).await })
    }

    fn ref_update<'life0, 'life1, 'life2, 'life3, 'async_trait>(
        &'life0 self,
        repo: &'life1 git2::Repository,
        name: &'life2 str,
        target: &'life3 str,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        'life3: 'async_trait,
        Self: Sync,
    {
        let repo = reopen(repo);
        let name = name.to_string();
        let target = target.to_string();
        Box::pin(async move { crate::refs::update(repo, &name, &target).await })
    }

    fn checkout<'life0, 'life1, 'life2, 'async_trait>(
        &'life0 self,
        repo: &'life1 git2::Repository,
        target: &'life2 str,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: Sync,
    {
        let repo = reopen(repo);
        let target = target.to_string();
        Box::pin(async move { crate::checkout::checkout(repo, &target).await })
    }

    fn head_name<'life0, 'life1, 'async_trait>(
        &'life0 self,
        repo: &'life1 git2::Repository,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>, Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: Sync,
    {
        let repo = reopen(repo);
        Box::pin(async move { crate::checkout::head_name(repo).await })
    }

    fn is_dirty<'life0, 'life1, 'async_trait>(
        &'life0 self,
        repo: &'life1 git2::Repository,
    ) -> Pin<Box<dyn Future<Output = Result<bool, Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: Sync,
    {
        let repo = reopen(repo);
        Box::pin(async move { crate::checkout::is_dirty(repo).await })
    }

    fn ahead_behind<'life0, 'life1, 'life2, 'async_trait>(
        &'life0 self,
        repo: &'life1 git2::Repository,
        upstream: &'life2 str,
    ) -> Pin<Box<dyn Future<Output = Result<(usize, usize), Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: Sync,
    {
        let repo = reopen(repo);
        let upstream = upstream.to_string();
        Box::pin(async move { crate::checkout::ahead_behind(repo, &upstream).await })
    }

    fn rebase<'life0, 'life1, 'life2, 'async_trait>(
        &'life0 self,
        repo: &'life1 git2::Repository,
        upstream: &'life2 str,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: Sync,
    {
        let repo = reopen(repo);
        let upstream = upstream.to_string();
        Box::pin(async move { crate::checkout::rebase(repo, &upstream).await })
    }

    fn stage_all<'life0, 'life1, 'async_trait>(
        &'life0 self,
        repo: &'life1 git2::Repository,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: Sync,
    {
        let repo = reopen(repo);
        Box::pin(async move { crate::stage::add_all(repo).await })
    }

    fn cherry_pick<'life0, 'life1, 'life2, 'async_trait>(
        &'life0 self,
        repo: &'life1 git2::Repository,
        sha: &'life2 str,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: Sync,
    {
        let repo = reopen(repo);
        let sha = sha.to_string();
        Box::pin(async move { crate::cherry_pick::cherry_pick(repo, &sha).await })
    }

    fn list_remote_refs<'life0, 'life1, 'life2, 'life3, 'async_trait>(
        &'life0 self,
        repo: &'life1 git2::Repository,
        remote: &'life2 str,
        pattern: &'life3 str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<(String, String)>, Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        'life3: 'async_trait,
        Self: Sync,
    {
        let repo = reopen(repo);
        let remote = remote.to_string();
        let pattern = pattern.to_string();
        Box::pin(async move {
            crate::download::list_remote_refs(repo, &remote, &pattern).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_backend_open_existing() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        git2::Repository::init(&path).unwrap();

        let backend = DefaultBackend;
        let repo = backend.open(&path).await.unwrap();
        assert!(!repo.is_bare());
    }

    #[tokio::test]
    async fn test_backend_open_nonexistent() {
        let path = Utf8PathBuf::from("/tmp/repo_git_nonexistent_test_54321");
        let backend = DefaultBackend;
        let result = backend.open(&path).await;
        assert!(result.is_err());
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected error"),
        };
        assert!(matches!(err, Error::Git2(_)));
    }

    #[tokio::test]
    async fn test_backend_init() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().join("repo")).unwrap();

        let backend = DefaultBackend;
        let repo = backend.init(&path).await.unwrap();
        assert!(!repo.is_bare());
    }

    #[tokio::test]
    async fn test_backend_init_bare() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().join("repo")).unwrap();

        let backend = DefaultBackend;
        let repo = backend.init_bare(&path).await.unwrap();
        assert!(repo.is_bare());
    }

    #[tokio::test]
    async fn test_backend_config_get_set() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let mut repo = git2::Repository::init(&path).unwrap();

        let backend = DefaultBackend;
        backend
            .config_set(&mut repo, "user.name", "Test")
            .await
            .unwrap();
        let val = backend.config_get_string(&repo, "user.name").await.unwrap();
        assert_eq!(val, "Test");
    }

    #[tokio::test]
    async fn test_backend_config_get_bool() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let mut repo = git2::Repository::init(&path).unwrap();

        let backend = DefaultBackend;
        backend
            .config_set(&mut repo, "core.bare", "true")
            .await
            .unwrap();
        let val = backend.config_get_bool(&repo, "core.bare").await.unwrap();
        assert_eq!(val, Some(true));
    }

    #[tokio::test]
    async fn test_backend_config_get_missing() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let repo = git2::Repository::init(&path).unwrap();

        let backend = DefaultBackend;
        let err = backend
            .config_get_string(&repo, "nonexistent.key")
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Config(_)));
    }

    #[tokio::test]
    async fn test_mock_backend_open() {
        let mut mock = MockGitBackend::new();
        mock.expect_open()
            .times(1)
            .returning(|_| Box::pin(async { Err(Error::Git2("mocked".to_string())) }));

        let path = Utf8PathBuf::from("/tmp/test");
        let result = mock.open(&path).await;
        assert!(result.is_err());
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected error"),
        };
        assert!(matches!(err, Error::Git2(msg) if msg == "mocked"));
    }

    #[tokio::test]
    async fn test_mock_backend_clone() {
        let mut mock = MockGitBackend::new();
        mock.expect_clone()
            .times(1)
            .returning(|_, _, _, _, _| Box::pin(async { Err(Error::Git2("clone mocked".to_string())) }));

        let url = Url::parse("file:///tmp/test").unwrap();
        let dst = Utf8PathBuf::from("/tmp/dst");
        let result = mock.clone(&url, &dst, None, None, &()).await;
        assert!(result.is_err());
        assert!(result.is_err());
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected error"),
        };
        assert!(matches!(err, Error::Git2(msg) if msg == "clone mocked"));
    }

    #[tokio::test]
    async fn test_backend_clone_and_fetch() {
        let bare_dir = tempdir().unwrap();
        let bare_path = crate::test_helpers::git_init_bare_with_commit(bare_dir.path());

        let dst_dir = tempdir().unwrap();
        let dst_path = Utf8PathBuf::from_path_buf(dst_dir.path().join("cloned")).unwrap();
        let url = Url::from_file_path(&bare_path).unwrap();

        let backend = DefaultBackend;
        let repo = GitBackend::clone(&backend, &url, &dst_path, None, None, &()).await.unwrap();
        assert!(!repo.is_bare());

        // Fetch should succeed (nothing new to fetch, but should not error).
        backend.fetch(&repo, "origin", false, false, None, &()).await.unwrap();
    }

    #[tokio::test]
    async fn test_backend_clone_bare() {
        let bare_dir = tempdir().unwrap();
        let bare_path = crate::test_helpers::git_init_bare_with_commit(bare_dir.path());

        let dst_dir = tempdir().unwrap();
        let dst_path = Utf8PathBuf::from_path_buf(dst_dir.path().join("cloned")).unwrap();
        let url = Url::from_file_path(&bare_path).unwrap();

        let backend = DefaultBackend;
        let repo = GitBackend::clone_bare(&backend, &url, &dst_path, &()).await.unwrap();
        assert!(repo.is_bare());
    }

    #[tokio::test]
    async fn test_backend_fetch_refspec() {
        let bare_dir = tempdir().unwrap();
        let bare_path = crate::test_helpers::git_init_bare_with_commit(bare_dir.path());

        let dst_dir = tempdir().unwrap();
        let dst_path = Utf8PathBuf::from_path_buf(dst_dir.path().join("cloned")).unwrap();
        let url = Url::from_file_path(&bare_path).unwrap();

        let backend = DefaultBackend;
        let repo = GitBackend::clone(&backend, &url, &dst_path, None, None, &()).await.unwrap();
        backend
            .fetch_refspec(&repo, "origin", "+refs/heads/*:refs/remotes/origin/*", &())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_backend_push() {
        let bare_dir = tempdir().unwrap();
        let bare_path = crate::test_helpers::git_init_bare_with_commit(bare_dir.path());

        let dst_dir = tempdir().unwrap();
        let dst_path = Utf8PathBuf::from_path_buf(dst_dir.path().join("cloned")).unwrap();
        let url = Url::from_file_path(&bare_path).unwrap();

        let backend = DefaultBackend;
        let repo = GitBackend::clone(&backend, &url, &dst_path, None, None, &()).await.unwrap();

        // Create a new branch and push it
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        let tree = parent.tree().unwrap();
        let oid = repo
            .commit(Some("refs/heads/feature"), &sig, &sig, "feature commit", &tree, &[&parent])
            .unwrap();

        backend.push(&repo, "origin", &["refs/heads/feature:refs/heads/feature".to_string()], &())
            .await
            .unwrap();

        // Verify the bare repo received the ref
        let bare_repo = git2::Repository::open(&bare_path).unwrap();
        let ref_ = bare_repo.find_reference("refs/heads/feature").unwrap();
        assert_eq!(ref_.target().unwrap().to_string(), oid.to_string());
    }

    #[tokio::test]
    async fn test_backend_ref_resolve() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let _repo = git2::Repository::init(&path).unwrap();
        crate::test_helpers::git_add_empty_commit(path.as_std_path(), "initial");
        let repo = git2::Repository::open(&path).unwrap();

        let backend = DefaultBackend;
        let sha = backend.ref_resolve(&repo, "HEAD").await.unwrap();
        assert_eq!(sha.len(), 40);
    }

    #[tokio::test]
    async fn test_backend_ref_list_and_update() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let _repo = git2::Repository::init(&path).unwrap();
        crate::test_helpers::git_add_empty_commit(path.as_std_path(), "initial");
        let repo = git2::Repository::open(&path).unwrap();

        let backend = DefaultBackend;
        let refs = backend.ref_list(&repo, None).await.unwrap();
        assert!(!refs.is_empty());

        let head = backend.ref_resolve(&repo, "HEAD").await.unwrap();
        backend.ref_update(&repo, "refs/tags/v1", &head).await.unwrap();

        let tag = backend.ref_resolve(&repo, "refs/tags/v1").await.unwrap();
        assert_eq!(tag, head);
    }

    #[tokio::test]
    async fn test_backend_checkout() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let _repo = git2::Repository::init(&path).unwrap();
        crate::test_helpers::git_add_empty_commit(path.as_std_path(), "initial");
        crate::test_helpers::git_add_empty_commit(path.as_std_path(), "second");
        let repo = git2::Repository::open(&path).unwrap();
        let first = repo.revparse_single("HEAD~1").unwrap().id().to_string();

        let backend = DefaultBackend;
        backend.checkout(&repo, &first).await.unwrap();
        let head = backend.ref_resolve(&repo, "HEAD").await.unwrap();
        assert_eq!(head, first);
    }

    #[tokio::test]
    async fn test_backend_head_name() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let _repo = git2::Repository::init(&path).unwrap();
        crate::test_helpers::git_add_empty_commit(path.as_std_path(), "initial");
        let repo = git2::Repository::open(&path).unwrap();

        let backend = DefaultBackend;
        let name = backend.head_name(&repo).await.unwrap();
        assert!(name.is_some());
    }

    #[tokio::test]
    async fn test_backend_is_dirty() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let repo = git2::Repository::init(&path).unwrap();
        std::fs::write(path.join("dirty.txt"), "hello").unwrap();

        let backend = DefaultBackend;
        let dirty = backend.is_dirty(&repo).await.unwrap();
        assert!(dirty);
    }

    #[tokio::test]
    async fn test_backend_is_not_dirty() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let _repo = git2::Repository::init(&path).unwrap();
        crate::test_helpers::git_add_empty_commit(path.as_std_path(), "initial");
        let repo = git2::Repository::open(&path).unwrap();

        let backend = DefaultBackend;
        let dirty = backend.is_dirty(&repo).await.unwrap();
        assert!(!dirty);
    }

    #[tokio::test]
    async fn test_backend_ahead_behind() {
        let bare_dir = tempdir().unwrap();
        let bare_path = crate::test_helpers::git_init_bare_with_commit(bare_dir.path());

        let dst_dir = tempdir().unwrap();
        let dst_path = Utf8PathBuf::from_path_buf(dst_dir.path().join("cloned")).unwrap();
        let url = Url::from_file_path(&bare_path).unwrap();

        let backend = DefaultBackend;
        let repo = GitBackend::clone(&backend, &url, &dst_path, None, None, &()).await.unwrap();
        crate::test_helpers::git_add_empty_commit(dst_path.as_std_path(), "local");
        let repo = git2::Repository::open(&dst_path).unwrap();

        let (ahead, behind) = backend.ahead_behind(&repo, "origin/HEAD").await.unwrap();
        assert_eq!(ahead, 1);
        assert_eq!(behind, 0);
    }

    #[tokio::test]
    async fn test_backend_rebase() {
        let bare_dir = tempdir().unwrap();
        let bare_path = crate::test_helpers::git_init_bare_with_commit(bare_dir.path());

        let dst_dir = tempdir().unwrap();
        let dst_path = Utf8PathBuf::from_path_buf(dst_dir.path().join("cloned")).unwrap();
        let url = Url::from_file_path(&bare_path).unwrap();

        let backend = DefaultBackend;
        let repo = GitBackend::clone(&backend, &url, &dst_path, None, None, &()).await.unwrap();
        // Create a commit with an actual file change so rebase has real work to do.
        std::fs::write(dst_path.join("change.txt"), "hello").unwrap();
        let repo = git2::Repository::open(&dst_path).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("change.txt")).unwrap();
        index.write().unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "local change", &tree, &[&parent]).unwrap();

        // Rebase current branch onto origin/HEAD.
        backend.rebase(&repo, "origin/HEAD").await.unwrap();
    }

    #[tokio::test]
    async fn test_backend_cherry_pick() {
        let bare_dir = tempdir().unwrap();
        let bare_path = crate::test_helpers::git_init_bare_with_commit(bare_dir.path());

        let dst_dir = tempdir().unwrap();
        let dst_path = Utf8PathBuf::from_path_buf(dst_dir.path().join("cloned")).unwrap();
        let url = Url::from_file_path(&bare_path).unwrap();

        let backend = DefaultBackend;
        let repo = GitBackend::clone(&backend, &url, &dst_path, None, None, &()).await.unwrap();
        crate::test_helpers::git_add_empty_commit(dst_path.as_std_path(), "local");
        let repo = git2::Repository::open(&dst_path).unwrap();
        let to_pick = repo.head().unwrap().target().unwrap().to_string();
        
        // Go back one commit and cherry-pick the latest
        let parent = repo.revparse_single("HEAD~1").unwrap().id().to_string();
        backend.checkout(&repo, &parent).await.unwrap();
        backend.cherry_pick(&repo, &to_pick).await.unwrap();
    }

    #[tokio::test]
    async fn test_backend_stage_all() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let repo = git2::Repository::init(&path).unwrap();
        std::fs::write(path.join("new.txt"), "hello").unwrap();

        let backend = DefaultBackend;
        backend.stage_all(&repo).await.unwrap();

        let index = repo.index().unwrap();
        assert_eq!(index.len(), 1);
    }

    #[tokio::test]
    async fn test_backend_list_remote_refs() {
        let bare_dir = tempdir().unwrap();
        let bare_path = crate::test_helpers::git_init_bare_with_commit(bare_dir.path());

        let dst_dir = tempdir().unwrap();
        let dst_path = Utf8PathBuf::from_path_buf(dst_dir.path().join("cloned")).unwrap();
        let url = Url::from_file_path(&bare_path).unwrap();

        let backend = DefaultBackend;
        let repo = GitBackend::clone(&backend, &url, &dst_path, None, None, &()).await.unwrap();

        let refs = backend.list_remote_refs(&repo, "origin", "refs/heads/").await.unwrap();
        assert!(!refs.is_empty());
    }

    #[tokio::test]
    async fn test_backend_config_set_local() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let mut repo = git2::Repository::init(&path).unwrap();

        let backend = DefaultBackend;
        backend.config_set_local(&mut repo, "local.key", "local-value").await.unwrap();
        let val = backend.config_get_string(&repo, "local.key").await.unwrap();
        assert_eq!(val, "local-value");
    }

    #[tokio::test]
    async fn test_backend_ref_list_with_pattern() {
        let dir = tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let _repo = git2::Repository::init(&path).unwrap();
        crate::test_helpers::git_add_empty_commit(path.as_std_path(), "initial");
        let repo = git2::Repository::open(&path).unwrap();

        let backend = DefaultBackend;
        let refs = backend.ref_list(&repo, Some("refs/heads/")).await.unwrap();
        assert!(!refs.is_empty());
    }

}
