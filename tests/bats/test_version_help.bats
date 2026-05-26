#!/usr/bin/env bats
#
# Feature-parity tests: version and help
#

load test_helper

@test "rust repo version prints version info" {
    run repo_rust version
    [ "$status" -eq 0 ]
    [[ "$output" == *"repo version"* ]]
}

@test "python repo version prints version info" {
    run repo_python version
    [ "$status" -eq 0 ]
    [[ "$output" == *"repo launcher version"* ]]
}

@test "rust repo help prints help text" {
    run repo_rust help
    [ "$status" -eq 0 ]
    [[ -n "$output" ]]
}

@test "python repo help prints help text" {
    run repo_python help
    [ "$status" -eq 0 ]
    [[ -n "$output" ]]
}
