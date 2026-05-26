#!/usr/bin/env bats
#
# Feature-parity tests: forall and grep
#

load test_helper

setup_file() {
    gerrit_start
    gerrit_ensure_auth

    FILE_SUFFIX="forall${RANDOM}"
    export FILE_SUFFIX

    export MANIFEST_PROJECT="manifest-${FILE_SUFFIX}"
    export PROJECT_FOO="foo-${FILE_SUFFIX}"
    export PROJECT_BAR="bar-${FILE_SUFFIX}"

    gerrit_create_project "$MANIFEST_PROJECT"
    gerrit_create_project "$PROJECT_FOO"
    gerrit_create_project "$PROJECT_BAR"

    gerrit_push_and_submit "$PROJECT_FOO" "master" "Initial commit" \
        "README.md" "hello from foo" \
        "src/lib.rs" "pub fn foo() {}"

    gerrit_push_and_submit "$PROJECT_BAR" "master" "Initial commit" \
        "README.md" "hello from bar" \
        "src/lib.rs" "pub fn bar() {}"

    setup_gerrit_manifest "$MANIFEST_PROJECT" "$GERRIT_URL" "$PROJECT_FOO" "$PROJECT_BAR"
}

setup() {
    export RUST_CHECKOUT="${BATS_TEST_TMPDIR}/rust-checkout"
    export PYTHON_CHECKOUT="${BATS_TEST_TMPDIR}/python-checkout"
    mkdir -p "$RUST_CHECKOUT" "$PYTHON_CHECKOUT"
}

@test "repo forall runs command in all projects" {
    cd "$RUST_CHECKOUT"
    run repo_rust init --manifest-url "$(gerrit_git_url "$MANIFEST_PROJECT")"
    [ "$status" -eq 0 ]
    run repo_rust sync
    [ "$status" -eq 0 ]

    run repo_rust forall -c "echo REPO_PATH=\$REPO_PATH"
    [ "$status" -eq 0 ]
    [[ "$output" == *"$PROJECT_FOO"* ]]
    [[ "$output" == *"$PROJECT_BAR"* ]]

    cd "$PYTHON_CHECKOUT"
    run repo_python init -u "$(gerrit_git_url "$MANIFEST_PROJECT")" --no-clone-bundle
    [ "$status" -eq 0 ]
    run repo_python sync
    [ "$status" -eq 0 ]

    run repo_python forall -c "echo REPO_PATH=\$REPO_PATH"
    [ "$status" -eq 0 ]
    [[ "$output" == *"$PROJECT_FOO"* ]]
    [[ "$output" == *"$PROJECT_BAR"* ]]
}

@test "repo grep finds matching content" {
    cd "$RUST_CHECKOUT"
    run repo_rust init --manifest-url "$(gerrit_git_url "$MANIFEST_PROJECT")"
    [ "$status" -eq 0 ]
    run repo_rust sync
    [ "$status" -eq 0 ]

    run repo_rust grep "pub fn"
    [ "$status" -eq 0 ]
    [[ "$output" == *"src/lib.rs"* ]]

    cd "$PYTHON_CHECKOUT"
    run repo_python init -u "$(gerrit_git_url "$MANIFEST_PROJECT")" --no-clone-bundle
    [ "$status" -eq 0 ]
    run repo_python sync
    [ "$status" -eq 0 ]

    run repo_python grep "pub fn"
    [ "$status" -eq 0 ]
    [[ "$output" == *"src/lib.rs"* ]]
}
