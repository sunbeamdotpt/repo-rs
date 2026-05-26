# repo-rs BATS Feature-Parity Tests

This directory contains [BATS](https://github.com/bats-core/bats-core) integration tests that prove feature-by-feature parity between the Rust `repo` binary and the reference Python `repo` tool against a shared Gerrit instance.

## Prerequisites

- [bats-core](https://github.com/bats-core/bats-core) (`brew install bats-core`)
- Docker & Docker Compose (for Gerrit)
- The Rust `repo` binary built (`cargo build -p repo-rs-cli`)
- The Python `repo` tool installed (`~/.bin/repo`)

## Running the Tests

```bash
# Run all BATS tests
bats tests/bats/

# Run a specific test file
bats tests/bats/test_init_sync.bats

# Run with verbose output
bats --tap tests/bats/
```

## Test Coverage

| Test File | Commands Tested |
|-----------|-----------------|
| `test_init_sync.bats` | `init`, `sync` |
| `test_start_branch.bats` | `start`, `branches`, `abandon` |
| `test_status_diff.bats` | `status`, `diff`, `list`, `info`, `manifest` |
| `test_forall_grep.bats` | `forall`, `grep` |
| `test_version_help.bats` | `version`, `help` |

## Gerrit Setup

Tests automatically manage a Gerrit container via the Docker Compose file at `crates/repo-rs-cli/tests/docker-compose.gerrit.yml`. The container:

- Exposes HTTP on `localhost:8080`
- Exposes SSH on `localhost:29418`
- Uses `DEVELOPMENT_BECOME_ANY_ACCOUNT` auth type

If the container is already running, tests reuse it.

## Known Limitations

### `upload` (Rust)

The Rust `repo` tool uses **libgit2** for git operations. libgit2 does not support cookie-based HTTP authentication, which is the only auth method available when Gerrit is configured with `DEVELOPMENT_BECOME_ANY_ACCOUNT`. As a result:

- **Python `repo upload`** works against Gerrit HTTP remotes (uses system `git` CLI with `http.cookieFile`)
- **Rust `repo upload`** fails with "too many redirects or authentication replays" against Gerrit HTTP remotes

This limitation does not affect `file://` remotes (covered by the Rust integration tests) or SSH remotes.

### `download`

Both tools can successfully `download` changes from Gerrit when the change ref exists. Creating the change ref via `repo upload` is the typical workflow, but since Rust upload is limited, downloads are best tested by pre-seeding change refs through the Gerrit REST API or manual push.

## Helper Library

`test_helper.bash` provides reusable functions for:

- Starting/stopping the Gerrit container
- Authenticating with Gerrit via cookie-based auth
- Creating projects via REST API
- Pushing content via the Gerrit review workflow
- Running both repo implementations
