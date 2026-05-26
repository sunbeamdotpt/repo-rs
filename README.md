# repo-rs

A Rust implementation of the [Android repo tool](https://gerrit.googlesource.com/git-repo/), providing a fast, statically-linked binary for managing very large multi-repository projects.

## Why?

Modern monorepo tooling is complex, nuanced, and requires alignment with how very large companies structure their repositories. `repo` is one of the most useful tools in modern software when it comes to treating all of your repositories as a single, cohesive unit. However, it is written in Python and focused almost exclusively on the Android platform. This project is a toolkit that let's builders, engineers, and other developers design and build modern monorepo tooling in almost any shape or form.

While the default implementation is for Gerrit, it is to showcase how the internals work and how to compose the library. This project is structured as a library so it can be easily integrated into other tools and workflows, allowing engineers to build tools that meet their exact needs.

Please let us know if you build something cool with it!

`repo-rs` mirrors the command-line interface of the Python-based `repo` tool while being
implemented entirely in Rust. It supports all 29 standard repo commands, from `init` and `sync`
to `upload`, `download`, and `forall`.

### Why Rust?

- **Single binary**: No Python runtime or virtualenv required
- **Performance**: Faster sync and grep operations through native compilation
- **Type safety**: Leverages Rust's type system to catch manifest and model errors at compile time
- **Async I/O**: Built on Tokio for efficient parallel repository operations

## Quick Start

```bash
# Build the repo binary
cargo build --release

# Initialize a repo checkout
./target/release/repo init --manifest-url https://example.com/manifests

# Sync all projects
./target/release/repo sync

# Start a topic branch
./target/release/repo start my-topic --all

# Check status
./target/release/repo status
```

## Architecture

The workspace is organized into six crates:

| Crate | Responsibility |
|-------|----------------|
| `repo-rs-manifest` | XML parsing, validation, and resolution of `manifest.xml` |
| `repo-rs-model` | Core data types: `Project`, `RepoClient`, `RemoteSpec`, etc. |
| `repo-rs-git` | Git backend abstraction over `git2` |
| `repo-rs-engine` | Command implementations (`sync`, `upload`, `start`, etc.) |
| `repo-rs-cmd` | CLI argument parsing with `clap` |
| `repo-rs-cli` | The `repo` binary entry point |

## Supported Commands

- `init` — Initialize a new repo checkout
- `sync` — Download projects and update working trees
- `start` — Create a new topic branch
- `branches` — List topic branches
- `abandon` — Remove a topic branch
- `status` — Show working tree status
- `diff` — Show project diffs
- `stage` — Stage changes
- `upload` — Upload changes for code review
- `download` — Download a change from code review
- `rebase` — Rebase topic branches
- `cherry-pick` — Cherry-pick across projects
- `checkout` — Checkout a branch
- `forall` — Run a shell command in each project
- `grep` — Search across projects
- `prune` — Prune merged topic branches
- `manifest` — Output the current manifest XML
- `info` — Display project information
- `list` — List all projects
- `diffmanifests` — Diff two manifest revisions
- `overview` — Overview of unmerged branches
- `gc` — Garbage collect project repositories
- `wipe` — Wipe and re-sync a project
- `smartsync` — Smart sync (update only changed projects)
- `selfupdate` — Update the repo tool itself
- `version` — Show version information
- `help` — Display help

## Development

### Prerequisites

- Rust 1.85+ (2024 edition)
- `git` CLI
- Docker & Docker Compose (for BATS/Gerrit integration tests)
- `bats-core` (for feature-parity tests)

### Building

```bash
cargo build --release
```

### Testing

```bash
# Run all unit and integration tests
cargo test --workspace

# Run integration tests against local bare repos
cargo test -p repo-rs-cli --test integration

# Run BATS feature-parity tests against Gerrit
bats tests/bats/
```

### Project Structure

```
repo-rs/
├── Cargo.toml                 # Workspace manifest
├── README.md                  # This file
├── crates/
│   ├── repo-rs-manifest/         # Manifest XML parsing
│   ├── repo-rs-model/            # Core data models
│   ├── repo-rs-git/              # Git backend (git2 wrapper)
│   ├── repo-rs-engine/           # Command implementations
│   ├── repo-rs-cmd/              # CLI parsing
│   └── repo-rs-cli/              # Binary entry point
│       └── tests/
│           ├── integration/   # Rust integration tests
│           └── bats/          # BATS parity tests
└── tests/
    └── bats/                  # BATS test suite & helpers
```

## Gerrit Integration Tests

The BATS test suite (`tests/bats/`) proves feature parity between the Rust and Python
`repo` tools against a shared [Gerrit](https://www.gerritcodereview.com/) instance.

See [`tests/bats/README.md`](tests/bats/README.md) for details on running and extending
the BATS tests.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
