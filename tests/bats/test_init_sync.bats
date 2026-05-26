#!/usr/bin/env bats
#
# Feature-parity tests: repo init + repo sync
#

load test_helper

setup_file() {
    gerrit_start
    gerrit_ensure_auth

    # Unique suffix for this test file's projects
    FILE_SUFFIX="initsync${RANDOM}"
    export FILE_SUFFIX

    export MANIFEST_PROJECT="manifest-${FILE_SUFFIX}"
    export PROJECT_FOO="foo-${FILE_SUFFIX}"
    export PROJECT_BAR="bar-${FILE_SUFFIX}"

    # Create projects in Gerrit
    gerrit_create_project "$MANIFEST_PROJECT"
    gerrit_create_project "$PROJECT_FOO"
    gerrit_create_project "$PROJECT_BAR"

    # Seed project repos with initial content
    gerrit_push_and_submit "$PROJECT_FOO" "master" "Initial commit" \
        "README.md" "hello from foo"

    gerrit_push_and_submit "$PROJECT_BAR" "master" "Initial commit" \
        "README.md" "hello from bar"

    # Set up manifest repo
    setup_gerrit_manifest "$MANIFEST_PROJECT" "$GERRIT_URL" "$PROJECT_FOO" "$PROJECT_BAR"
}

setup() {
    export RUST_CHECKOUT="${BATS_TEST_TMPDIR}/rust-checkout"
    export PYTHON_CHECKOUT="${BATS_TEST_TMPDIR}/python-checkout"
    mkdir -p "$RUST_CHECKOUT" "$PYTHON_CHECKOUT"
}

@test "repo init creates .repo directory" {
    cd "$RUST_CHECKOUT"
    run repo_rust init --manifest-url "$(gerrit_git_url "$MANIFEST_PROJECT")"
    [ "$status" -eq 0 ]
    assert_dir_exists "$RUST_CHECKOUT/.repo"
    assert_file_exists "$RUST_CHECKOUT/.repo/manifest.xml"

    cd "$PYTHON_CHECKOUT"
    run repo_python init -u "$(gerrit_git_url "$MANIFEST_PROJECT")" --no-clone-bundle
    [ "$status" -eq 0 ]
    assert_dir_exists "$PYTHON_CHECKOUT/.repo"
    assert_file_exists "$PYTHON_CHECKOUT/.repo/manifest.xml"
}

@test "repo sync creates project worktrees" {
    # Rust
    cd "$RUST_CHECKOUT"
    run repo_rust init --manifest-url "$(gerrit_git_url "$MANIFEST_PROJECT")"
    [ "$status" -eq 0 ]

    run repo_rust sync
    [ "$status" -eq 0 ]
    assert_dir_exists "$RUST_CHECKOUT/$PROJECT_FOO"
    assert_dir_exists "$RUST_CHECKOUT/$PROJECT_BAR"
    assert_file_exists "$RUST_CHECKOUT/$PROJECT_FOO/README.md"
    assert_file_exists "$RUST_CHECKOUT/$PROJECT_BAR/README.md"

    # Python
    cd "$PYTHON_CHECKOUT"
    run repo_python init -u "$(gerrit_git_url "$MANIFEST_PROJECT")" --no-clone-bundle
    [ "$status" -eq 0 ]

    run repo_python sync
    [ "$status" -eq 0 ]
    assert_dir_exists "$PYTHON_CHECKOUT/$PROJECT_FOO"
    assert_dir_exists "$PYTHON_CHECKOUT/$PROJECT_BAR"
    assert_file_exists "$PYTHON_CHECKOUT/$PROJECT_FOO/README.md"
    assert_file_exists "$PYTHON_CHECKOUT/$PROJECT_BAR/README.md"
}

@test "synced worktrees have identical content" {
    # Rust checkout
    cd "$RUST_CHECKOUT"
    run repo_rust init --manifest-url "$(gerrit_git_url "$MANIFEST_PROJECT")"
    [ "$status" -eq 0 ]
    run repo_rust sync
    [ "$status" -eq 0 ]

    # Python checkout
    cd "$PYTHON_CHECKOUT"
    run repo_python init -u "$(gerrit_git_url "$MANIFEST_PROJECT")" --no-clone-bundle
    [ "$status" -eq 0 ]
    run repo_python sync
    [ "$status" -eq 0 ]

    # Compare project contents (ignoring .git and .repo)
    run diff -rq --exclude=.git --exclude=.repo "$RUST_CHECKOUT/$PROJECT_FOO" "$PYTHON_CHECKOUT/$PROJECT_FOO"
    [ "$status" -eq 0 ]

    run diff -rq --exclude=.git --exclude=.repo "$RUST_CHECKOUT/$PROJECT_BAR" "$PYTHON_CHECKOUT/$PROJECT_BAR"
    [ "$status" -eq 0 ]
}
