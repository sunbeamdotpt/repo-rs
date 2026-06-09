# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.3] - 2026-06-09

### Fixed

- **command output to stdout**: `repo list`, `repo branches`, `repo manifest`, `repo info`, `repo status`, `repo diff`, `repo overview`, `repo help`, `repo diffmanifests`, `repo version`, and `repo upload` now print their output directly to stdout/stderr instead of via `tracing::info!`. This fixes the issue where these commands produced no visible output when `repo-rs` was used as a library inside another binary (e.g. `sunbeam`) with a tracing subscriber that filtered out `INFO` level events for `repo_rs_*` crates.

## [0.2.2] - 2026-06-09

### Fixed

- **sync defers copyfiles/linkfiles**: `copyfile` and `linkfile` application is now deferred until after all projects have been cloned, fetched, and checked out. This prevents linkfiles targeting paths inside later projects' worktrees from creating those directories early and blocking later `git clone` operations.
- **sync rebase applies copyfiles/linkfiles**: Projects successfully rebased during `repo sync` now also have their copyfiles and linkfiles applied (previously they were silently skipped).

### Added

- **regression tests**: Added tests verifying that linkfiles and copyfiles into not-yet-cloned project directories do not block clone operations, and that rebase correctly applies copyfiles and linkfiles.

## [0.2.1] - 2026-06-09

### Fixed

- **sync copyfiles/linkfiles**: `copyfile` and `linkfile` elements from the manifest XML are now correctly applied after clone and checkout during `repo sync`
- **linkfile symlink creation**: `linkfile` elements now create symbolic links at the destination path pointing to the source path within the project worktree

### Added

- **regression tests**: Added tests covering copyfile and linkfile application during both clone and checkout operations

## [0.2.0] - 2026-06-09

_This release brings the `repo-rs-manifest` crate to feature parity with the Python reference implementation's XML manifest handling._

### Added

- **manifest parsing**: Complete XML manifest parser with support for all core elements (`<project>`, `<remote>`, `<default>`, `<extend-project>`, `<remove-project>`, `<submanifest>`, `<include>`, `<repo-hooks>`, `<superproject>`, `<contactinfo>`, `<manifest-server>`, `<notice>`, `<copyfile>`, `<linkfile>`, `<annotation>`)
- **manifest validation**: Path safety validation, boolean/integer attribute validation, duplicate element detection, circular include detection, remote reference validation with alias fallback
- **manifest resolution**: `<include>` resolution with depth-first expansion, local manifest merging from `local_manifests/` directory, `<extend-project>` mutation pass, `<remove-project>` deletion pass
- **auto-group injection**: Projects automatically receive `all`, `name:{name}`, and `path:{path}` groups; local manifests inject `local:{filename}` groups; submanifests support `submanifest:path:{prefix}` groups
- **submanifest depth limiting**: Enforces `MAX_SUBMANIFEST_DEPTH = 8` during include resolution
- **remote URL normalization**: SCP-like syntax (`git@host:path`) converted to SSH URLs; trailing slash stripping; relative URL resolution against manifest base URL
- **remote alias resolution**: Project remote lookups fall back to alias names when no exact match exists
- **XML serialization**: Full `manifest_to_xml()` implementation with round-trip support for all manifest elements
- **test coverage**: 163 tests with 91.96% line coverage on the manifest crate

### Changed

- Relaxed dependency version constraints in workspace `Cargo.toml` to use minor-only pinning

### Fixed

- Added missing package metadata (`description`, `repository`, `keywords`, `categories`, `readme`) to all crate manifests

## [0.1.0] - 2026-06-08

### Added

- Initial workspace structure with crates: `repo-rs-cli`, `repo-rs-cmd`, `repo-rs-engine`, `repo-rs-git`, `repo-rs-manifest`, `repo-rs-model`
