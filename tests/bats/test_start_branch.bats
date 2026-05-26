#!/usr/bin/env bats
#
# Feature-parity tests: repo start, branches, abandon
#

load test_helper

setup_file() {
    gerrit_start
    gerrit_ensure_auth

    FILE_SUFFIX="branch${RANDOM}"
    export FILE_SUFFIX

    export MANIFEST_PROJECT="manifest-${FILE_SUFFIX}"
    export PROJECT_FOO="foo-${FILE_SUFFIX}"
    export PROJECT_BAR="bar-${FILE_SUFFIX}"

    gerrit_create_project "$MANIFEST_PROJECT"
    gerrit_create_project "$PROJECT_FOO"
    gerrit_create_project "$PROJECT_BAR"

    gerrit_push_and_submit "$PROJECT_FOO" "master" "Initial commit" \
        "README.md" "hello from foo"

    gerrit_push_and_submit "$PROJECT_BAR" "master" "Initial commit" \
        "README.md" "hello from bar"

    setup_gerrit_manifest "$MANIFEST_PROJECT" "$GERRIT_URL" "$PROJECT_FOO" "$PROJECT_BAR"
}

setup() {
    export RUST_CHECKOUT="${BATS_TEST_TMPDIR}/rust-checkout"
    export PYTHON_CHECKOUT="${BATS_TEST_TMPDIR}/python-checkout"
    mkdir -p "$RUST_CHECKOUT" "$PYTHON_CHECKOUT"
}

@test "repo start creates topic branches" {
    # Rust
    cd "$RUST_CHECKOUT"
    run repo_rust init --manifest-url "$(gerrit_git_url "$MANIFEST_PROJECT")"
    [ "$status" -eq 0 ]
    run repo_rust sync
    [ "$status" -eq 0 ]

    run repo_rust start "my-topic" "$PROJECT_FOO" "$PROJECT_BAR"
    [ "$status" -eq 0 ]

    # Verify branch exists in foo
    run bash -c "cd '$RUST_CHECKOUT/$PROJECT_FOO' && git branch"
    [ "$status" -eq 0 ]
    [[ "$output" == *"my-topic"* ]]

    # Verify branch exists in bar
    run bash -c "cd '$RUST_CHECKOUT/$PROJECT_BAR' && git branch"
    [ "$status" -eq 0 ]
    [[ "$output" == *"my-topic"* ]]

    # Python
    cd "$PYTHON_CHECKOUT"
    run repo_python init -u "$(gerrit_git_url "$MANIFEST_PROJECT")" --no-clone-bundle
    [ "$status" -eq 0 ]
    run repo_python sync
    [ "$status" -eq 0 ]

    run repo_python start "my-topic" "$PROJECT_FOO" "$PROJECT_BAR"
    [ "$status" -eq 0 ]

    run bash -c "cd '$PYTHON_CHECKOUT/$PROJECT_FOO' && git branch"
    [ "$status" -eq 0 ]
    [[ "$output" == *"my-topic"* ]]
}

@test "repo branches lists topic branches" {
    cd "$RUST_CHECKOUT"
    run repo_rust init --manifest-url "$(gerrit_git_url "$MANIFEST_PROJECT")"
    [ "$status" -eq 0 ]
    run repo_rust sync
    [ "$status" -eq 0 ]
    run repo_rust start "my-topic" "$PROJECT_FOO" "$PROJECT_BAR"
    [ "$status" -eq 0 ]

    run repo_rust branches
    [ "$status" -eq 0 ]
    [[ "$output" == *"my-topic"* ]]

    cd "$PYTHON_CHECKOUT"
    run repo_python init -u "$(gerrit_git_url "$MANIFEST_PROJECT")" --no-clone-bundle
    [ "$status" -eq 0 ]
    run repo_python sync
    [ "$status" -eq 0 ]
    run repo_python start "my-topic" "$PROJECT_FOO" "$PROJECT_BAR"
    [ "$status" -eq 0 ]

    run repo_python branches
    [ "$status" -eq 0 ]
    [[ "$output" == *"my-topic"* ]]
}

@test "repo abandon removes topic branches" {
    cd "$RUST_CHECKOUT"
    run repo_rust init --manifest-url "$(gerrit_git_url "$MANIFEST_PROJECT")"
    [ "$status" -eq 0 ]
    run repo_rust sync
    [ "$status" -eq 0 ]
    run repo_rust start "my-topic" "$PROJECT_FOO" "$PROJECT_BAR"
    [ "$status" -eq 0 ]

    run repo_rust abandon "my-topic"
    [ "$status" -eq 0 ]

    run bash -c "cd '$RUST_CHECKOUT/$PROJECT_FOO' && git branch"
    [ "$status" -eq 0 ]
    [[ "$output" != *"my-topic"* ]]

    cd "$PYTHON_CHECKOUT"
    run repo_python init -u "$(gerrit_git_url "$MANIFEST_PROJECT")" --no-clone-bundle
    [ "$status" -eq 0 ]
    run repo_python sync
    [ "$status" -eq 0 ]
    run repo_python start "my-topic" "$PROJECT_FOO" "$PROJECT_BAR"
    [ "$status" -eq 0 ]

    run repo_python abandon "my-topic"
    [ "$status" -eq 0 ]

    run bash -c "cd '$PYTHON_CHECKOUT/$PROJECT_FOO' && git branch"
    [ "$status" -eq 0 ]
    [[ "$output" != *"my-topic"* ]]
}
