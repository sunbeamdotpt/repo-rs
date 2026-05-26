#!/usr/bin/env bats
#
# Feature-parity tests: status, diff, list, info, manifest
#

load test_helper

setup_file() {
    gerrit_start
    gerrit_ensure_auth

    FILE_SUFFIX="status${RANDOM}"
    export FILE_SUFFIX

    export MANIFEST_PROJECT="manifest-${FILE_SUFFIX}"
    export PROJECT_FOO="foo-${FILE_SUFFIX}"
    export PROJECT_BAR="bar-${FILE_SUFFIX}"

    gerrit_create_project "$MANIFEST_PROJECT"
    gerrit_create_project "$PROJECT_FOO"
    gerrit_create_project "$PROJECT_BAR"

    gerrit_push_and_submit "$PROJECT_FOO" "master" "Initial commit" \
        "README.md" "hello from foo" \
        "src/main.rs" "fn main() {}"

    gerrit_push_and_submit "$PROJECT_BAR" "master" "Initial commit" \
        "README.md" "hello from bar"

    setup_gerrit_manifest "$MANIFEST_PROJECT" "$GERRIT_URL" "$PROJECT_FOO" "$PROJECT_BAR"
}

setup() {
    export RUST_CHECKOUT="${BATS_TEST_TMPDIR}/rust-checkout"
    export PYTHON_CHECKOUT="${BATS_TEST_TMPDIR}/python-checkout"
    mkdir -p "$RUST_CHECKOUT" "$PYTHON_CHECKOUT"
}

@test "repo list shows projects" {
    cd "$RUST_CHECKOUT"
    run repo_rust init --manifest-url "$(gerrit_git_url "$MANIFEST_PROJECT")"
    [ "$status" -eq 0 ]
    run repo_rust sync
    [ "$status" -eq 0 ]

    run repo_rust list
    [ "$status" -eq 0 ]
    [[ "$output" == *"$PROJECT_FOO"* ]]
    [[ "$output" == *"$PROJECT_BAR"* ]]

    cd "$PYTHON_CHECKOUT"
    run repo_python init -u "$(gerrit_git_url "$MANIFEST_PROJECT")" --no-clone-bundle
    [ "$status" -eq 0 ]
    run repo_python sync
    [ "$status" -eq 0 ]

    run repo_python list
    [ "$status" -eq 0 ]
    [[ "$output" == *"$PROJECT_FOO"* ]]
    [[ "$output" == *"$PROJECT_BAR"* ]]
}

@test "repo info shows project info" {
    cd "$RUST_CHECKOUT"
    run repo_rust init --manifest-url "$(gerrit_git_url "$MANIFEST_PROJECT")"
    [ "$status" -eq 0 ]
    run repo_rust sync
    [ "$status" -eq 0 ]

    run repo_rust info
    [ "$status" -eq 0 ]
    [[ -n "$output" ]]

    cd "$PYTHON_CHECKOUT"
    run repo_python init -u "$(gerrit_git_url "$MANIFEST_PROJECT")" --no-clone-bundle
    [ "$status" -eq 0 ]
    run repo_python sync
    [ "$status" -eq 0 ]

    run repo_python info
    [ "$status" -eq 0 ]
    [[ -n "$output" ]]
}

@test "repo manifest outputs manifest xml" {
    cd "$RUST_CHECKOUT"
    run repo_rust init --manifest-url "$(gerrit_git_url "$MANIFEST_PROJECT")"
    [ "$status" -eq 0 ]
    run repo_rust sync
    [ "$status" -eq 0 ]

    run repo_rust manifest
    [ "$status" -eq 0 ]
    [[ "$output" == *"<?xml"* ]]

    cd "$PYTHON_CHECKOUT"
    run repo_python init -u "$(gerrit_git_url "$MANIFEST_PROJECT")" --no-clone-bundle
    [ "$status" -eq 0 ]
    run repo_python sync
    [ "$status" -eq 0 ]

    run repo_python manifest
    [ "$status" -eq 0 ]
    [[ "$output" == *"<?xml"* ]]
}

@test "repo status shows modified files" {
    cd "$RUST_CHECKOUT"
    run repo_rust init --manifest-url "$(gerrit_git_url "$MANIFEST_PROJECT")"
    [ "$status" -eq 0 ]
    run repo_rust sync
    [ "$status" -eq 0 ]

    echo "modified" > "$RUST_CHECKOUT/$PROJECT_FOO/README.md"

    run repo_rust status
    [ "$status" -eq 0 ]
    [[ "$output" == *"$PROJECT_FOO"* || "$output" == *"README.md"* ]]

    cd "$PYTHON_CHECKOUT"
    run repo_python init -u "$(gerrit_git_url "$MANIFEST_PROJECT")" --no-clone-bundle
    [ "$status" -eq 0 ]
    run repo_python sync
    [ "$status" -eq 0 ]

    echo "modified" > "$PYTHON_CHECKOUT/$PROJECT_FOO/README.md"

    run repo_python status
    [ "$status" -eq 0 ]
    [[ "$output" == *"$PROJECT_FOO"* || "$output" == *"README.md"* ]]
}

@test "repo diff shows dirty projects" {
    cd "$RUST_CHECKOUT"
    run repo_rust init --manifest-url "$(gerrit_git_url "$MANIFEST_PROJECT")"
    [ "$status" -eq 0 ]
    run repo_rust sync
    [ "$status" -eq 0 ]

    echo "modified" > "$RUST_CHECKOUT/$PROJECT_FOO/README.md"

    run repo_rust diff
    [ "$status" -eq 0 ]
    [[ "$output" == *"dirty"* || "$output" == *"$PROJECT_FOO"* ]]

    cd "$PYTHON_CHECKOUT"
    run repo_python init -u "$(gerrit_git_url "$MANIFEST_PROJECT")" --no-clone-bundle
    [ "$status" -eq 0 ]
    run repo_python sync
    [ "$status" -eq 0 ]

    echo "modified" > "$PYTHON_CHECKOUT/$PROJECT_FOO/README.md"

    run repo_python diff
    [ "$status" -eq 0 ]
    [[ "$output" == *"dirty"* || "$output" == *"$PROJECT_FOO"* ]]
}
