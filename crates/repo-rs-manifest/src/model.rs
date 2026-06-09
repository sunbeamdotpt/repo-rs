// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use indexmap::IndexMap;
use std::collections::HashSet;

/// A parsed manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    /// Notice text displayed after sync.
    pub notice: Option<String>,
    /// Remote definitions.
    pub remotes: IndexMap<String, Remote>,
    /// Default settings.
    pub default: Option<Default>,
    /// Manifest server URL.
    pub manifest_server: Option<String>,
    /// Submanifest definitions.
    pub submanifests: IndexMap<String, Submanifest>,
    /// Projects to remove.
    pub remove_projects: Vec<RemoveProject>,
    /// Project definitions.
    pub projects: Vec<Project>,
    /// Project extensions.
    pub extend_projects: Vec<ExtendProject>,
    /// Hooks project.
    pub repo_hooks: Option<RepoHooks>,
    /// Superproject definition.
    pub superproject: Option<Superproject>,
    /// Contact info.
    pub contactinfo: Option<ContactInfo>,
    /// Include directives.
    pub includes: Vec<Include>,
}

/// A remote definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote {
    /// The name of this item.
    pub name: String,
    /// An alternative name for this item.
    pub alias: Option<String>,
    /// The fetch URL template for the remote.
    pub fetch: String,
    /// The push URL for the remote.
    pub pushurl: Option<String>,
    /// The code review server URL.
    pub review: Option<String>,
    /// The git revision (branch, tag, or commit SHA).
    pub revision: Option<String>,
    /// Additional metadata annotations.
    pub annotations: Vec<Annotation>,
}

/// Default settings for projects.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Default {
    /// The remote repository configuration.
    pub remote: Option<String>,
    /// The git revision (branch, tag, or commit SHA).
    pub revision: Option<String>,
    /// The destination branch name.
    pub dest_branch: Option<String>,
    /// The upstream branch name.
    pub upstream: Option<String>,
    /// The number of parallel sync jobs.
    pub sync_j: Option<u32>,
    /// The maximum number of parallel sync jobs.
    pub sync_j_max: Option<u32>,
    /// Whether to sync only the current branch.
    pub sync_c: Option<bool>,
    /// Whether to sync submodules.
    pub sync_s: Option<bool>,
    /// Whether to sync git tags.
    pub sync_tags: Option<bool>,
}

/// A project definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    /// The name of this item.
    pub name: String,
    /// The filesystem path.
    pub path: Option<String>,
    /// The remote repository configuration.
    pub remote: Option<String>,
    /// The git revision (branch, tag, or commit SHA).
    pub revision: Option<String>,
    /// The destination branch name.
    pub dest_branch: Option<String>,
    /// The list of groups this item belongs to.
    pub groups: HashSet<String>,
    /// Whether to sync only the current branch.
    pub sync_c: Option<bool>,
    /// Whether to sync submodules.
    pub sync_s: Option<bool>,
    /// Whether to sync git tags.
    pub sync_tags: Option<bool>,
    /// The upstream branch name.
    pub upstream: Option<String>,
    /// The git clone depth (shallow clone limit).
    pub clone_depth: Option<u32>,
    /// Whether to force the specified path.
    pub force_path: Option<bool>,
    /// The synchronization strategy to use.
    pub sync_strategy: Option<String>,
    /// Additional metadata annotations.
    pub annotations: Vec<Annotation>,
    /// Files to copy after sync.
    pub copyfiles: Vec<CopyFile>,
    /// Files to symlink after sync.
    pub linkfiles: Vec<LinkFile>,
    /// Whether to rebase local changes on sync.
    pub rebase: Option<bool>,
    /// Child projects within this project.
    pub subprojects: Vec<Project>,
}

/// An annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    /// The name of this item.
    pub name: String,
    /// The annotation value.
    pub value: String,
    /// Whether to keep this annotation on rebase.
    pub keep: bool,
}

/// A copyfile directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyFile {
    /// The source path.
    pub src: String,
    /// The destination path.
    pub dest: String,
}

/// A linkfile directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkFile {
    /// The source path.
    pub src: String,
    /// The destination path.
    pub dest: String,
}

/// An extend-project directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtendProject {
    /// The name of this item.
    pub name: String,
    /// The filesystem path.
    pub path: Option<String>,
    /// The destination path for the project.
    pub dest_path: Option<String>,
    /// The list of groups this item belongs to.
    pub groups: HashSet<String>,
    /// The git revision (branch, tag, or commit SHA).
    pub revision: Option<String>,
    /// The remote repository configuration.
    pub remote: Option<String>,
    /// The destination branch name.
    pub dest_branch: Option<String>,
    /// The upstream branch name.
    pub upstream: Option<String>,
    /// The base revision for operations.
    pub base_rev: Option<String>,
    /// Additional metadata annotations.
    pub annotations: Vec<Annotation>,
    /// Files to copy after sync.
    pub copyfiles: Vec<CopyFile>,
    /// Files to symlink after sync.
    pub linkfiles: Vec<LinkFile>,
}

/// A remove-project directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveProject {
    /// The name of this item.
    pub name: Option<String>,
    /// The filesystem path.
    pub path: Option<String>,
    /// Whether this item is optional.
    pub optional: bool,
    /// The base revision for operations.
    pub base_rev: Option<String>,
}

/// A submanifest definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submanifest {
    /// The name of this item.
    pub name: String,
    /// The remote repository configuration.
    pub remote: Option<String>,
    /// Project.
    pub project: Option<String>,
    /// The name of the manifest file.
    pub manifest_name: Option<String>,
    /// The git revision (branch, tag, or commit SHA).
    pub revision: Option<String>,
    /// The filesystem path.
    pub path: Option<String>,
    /// The list of groups this item belongs to.
    pub groups: HashSet<String>,
    /// The default groups for new projects.
    pub default_groups: HashSet<String>,
}

/// Repo hooks definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoHooks {
    /// Whether this is configured in-project.
    pub in_project: String,
    /// The list of enabled hooks or features.
    pub enabled_list: String,
}

/// Superproject definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Superproject {
    /// The name of this item.
    pub name: String,
    /// The remote repository configuration.
    pub remote: Option<String>,
    /// The git revision (branch, tag, or commit SHA).
    pub revision: Option<String>,
}

/// Contact info.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactInfo {
    /// The URL for filing bugs.
    pub bugurl: String,
}

/// An include directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Include {
    /// The name of this item.
    pub name: String,
    /// The list of groups this item belongs to.
    pub groups: HashSet<String>,
    /// The git revision (branch, tag, or commit SHA).
    pub revision: Option<String>,
}

#[cfg(test)]
impl Project {
    /// Create a default test project.
    pub fn default_test() -> Self {
        Self {
            name: String::new(),
            path: Some(String::new()),
            remote: None,
            revision: None,
            dest_branch: None,
            groups: HashSet::new(),
            sync_c: None,
            sync_s: None,
            sync_tags: None,
            upstream: None,
            clone_depth: None,
            force_path: None,
            sync_strategy: None,
            annotations: Vec::new(),
            copyfiles: Vec::new(),
            linkfiles: Vec::new(),
            subprojects: Vec::new(),
            rebase: None,
        }
    }
}

#[cfg(test)]
impl ExtendProject {
    /// Create a default test extend-project.
    pub fn default_test() -> Self {
        Self {
            name: String::new(),
            path: None,
            dest_path: None,
            groups: HashSet::new(),
            revision: None,
            remote: None,
            dest_branch: None,
            upstream: None,
            base_rev: None,
            annotations: Vec::new(),
            copyfiles: Vec::new(),
            linkfiles: Vec::new(),
        }
    }
}
