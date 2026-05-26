#!/usr/bin/env bash
#
# BATS helper library for repo-rs feature-parity tests against Gerrit.
#

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

REPO_RUST_BINARY="${REPO_RUST_BINARY:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/target/debug/repo}"
REPO_PYTHON_BINARY="${REPO_PYTHON_BINARY:-$HOME/.bin/repo}"

GERRIT_URL="${GERRIT_URL:-http://localhost:8080}"
GERRIT_ADMIN_USER="${GERRIT_ADMIN_USER:-admin}"
GERRIT_ADMIN_PASS="${GERRIT_ADMIN_PASS:-secret}"
GERRIT_COMPOSE_FILE="${GERRIT_COMPOSE_FILE:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/crates/repo-cli/tests/docker-compose.gerrit.yml}"

GERRIT_COOKIE_JAR="${BATS_FILE_TMPDIR:-${BATS_TEST_TMPDIR:-/tmp}}/gerrit_cookies.txt"

# ---------------------------------------------------------------------------
# Gerrit container lifecycle
# ---------------------------------------------------------------------------

gerrit_is_running() {
    docker ps --format '{{.Names}}' | grep -qx 'repo-rs-gerrit-test'
}

gerrit_wait_for_ready() {
    local max_attempts=60
    local attempt=0
    while [[ $attempt -lt $max_attempts ]]; do
        if curl -sf "${GERRIT_URL}/config/server/version" >/dev/null 2>&1; then
            return 0
        fi
        sleep 1
        ((attempt++))
    done
    return 1
}

gerrit_start() {
    if gerrit_is_running; then
        return 0
    fi

    if [[ -f "$GERRIT_COMPOSE_FILE" ]]; then
        docker compose -f "$GERRIT_COMPOSE_FILE" up -d
    else
        docker run -d \
            --name repo-rs-gerrit-test \
            -p 8080:8080 \
            -p 29418:29418 \
            -e CANONICAL_WEB_URL="${GERRIT_URL}" \
            gerritcodereview/gerrit
    fi

    gerrit_wait_for_ready
}

gerrit_stop() {
    if [[ -f "$GERRIT_COMPOSE_FILE" ]]; then
        docker compose -f "$GERRIT_COMPOSE_FILE" down 2>/dev/null || true
    else
        docker stop repo-rs-gerrit-test 2>/dev/null || true
        docker rm repo-rs-gerrit-test 2>/dev/null || true
    fi
}

# ---------------------------------------------------------------------------
# Gerrit authentication
# ---------------------------------------------------------------------------

gerrit_ensure_auth() {
    mkdir -p "$(dirname "$GERRIT_COOKIE_JAR")"

    # Become admin account via DEVELOPMENT_BECOME_ANY_ACCOUNT
    # Follow redirects (-L) because the XSRF_TOKEN cookie is set on the redirect target page
    curl -s -L -c "$GERRIT_COOKIE_JAR" -b "$GERRIT_COOKIE_JAR" \
        "${GERRIT_URL}/login/%23/q/status:open?account_id=1000000" >/dev/null 2>&1 || true

    # Verify auth works
    if ! curl -sf -b "$GERRIT_COOKIE_JAR" "${GERRIT_URL}/config/server/version" >/dev/null 2>&1; then
        echo "ERROR: Failed to authenticate with Gerrit" >&2
        return 1
    fi
}

gerrit_xsrf_token() {
    if [[ ! -f "$GERRIT_COOKIE_JAR" ]]; then
        gerrit_ensure_auth
    fi
    grep 'XSRF_TOKEN' "$GERRIT_COOKIE_JAR" | awk '{print $7}' | tail -n1
}

# ---------------------------------------------------------------------------
# Gerrit REST API helpers
# ---------------------------------------------------------------------------

gerrit_rest() {
    local method="${1:-GET}"
    local endpoint="${2:-}"
    local data="${3:-}"
    local xsrf
    xsrf=$(gerrit_xsrf_token)

    local curl_args=(-s -b "$GERRIT_COOKIE_JAR" -H "X-Gerrit-Auth: $xsrf")

    if [[ "$method" != "GET" ]]; then
        curl_args+=(-X "$method" -H "Content-Type: application/json")
        if [[ -n "$data" ]]; then
            curl_args+=(-d "$data")
        fi
    fi

    # Strip Gerrit JSON anti-XSS prefix
    curl "${curl_args[@]}" "${GERRIT_URL}/a${endpoint}" | tail -c +5
}

gerrit_create_project() {
    local name="$1"
    gerrit_rest "PUT" "/projects/${name}" '{"create_empty_commit":true}' >/dev/null
}

gerrit_get_change() {
    local change_id="$1"
    gerrit_rest "GET" "/changes/?q=${change_id}"
}

gerrit_review_change() {
    local change_id="$1"
    local revision="${2:-1}"
    gerrit_rest "POST" "/changes/${change_id}/revisions/${revision}/review/" \
        '{"labels":{"Code-Review":2}}' >/dev/null
}

gerrit_submit_change() {
    local change_id="$1"
    gerrit_rest "POST" "/changes/${change_id}/submit" '{}' >/dev/null || true
}

gerrit_approve_and_submit() {
    local change_id="$1"
    gerrit_review_change "$change_id"
    gerrit_submit_change "$change_id" || true
}

# ---------------------------------------------------------------------------
# Git helpers for Gerrit repos
# ---------------------------------------------------------------------------

gerrit_git_url() {
    local project="$1"
    echo "http://${GERRIT_ADMIN_USER}:${GERRIT_ADMIN_PASS}@${GERRIT_URL#http://}/${project}"
}

gerrit_install_commit_msg_hook() {
    local repo_dir="$1"
    local hook_dir="${repo_dir}/.git/hooks"
    mkdir -p "$hook_dir"
    curl -sf -o "${hook_dir}/commit-msg" "${GERRIT_URL}/tools/hooks/commit-msg"
    chmod +x "${hook_dir}/commit-msg"
}

# Run git with the Gerrit cookie file configured for HTTP auth.
_gerrit_git() {
    git -c http.cookieFile="$GERRIT_COOKIE_JAR" "$@"
}

# Clone a Gerrit project, returning the clone directory.
gerrit_clone() {
    local project="$1"
    local dest="$2"
    _gerrit_git clone -q "http://${GERRIT_URL#http://}/${project}" "$dest"
}

# Push content to a Gerrit project via review workflow and auto-submit.
# Arguments: project branch commit_message [file content]...
gerrit_push_and_submit() {
    local project="$1"
    local branch="$2"
    local message="$3"
    shift 3

    local tmpdir
    tmpdir=$(mktemp -d)

    gerrit_clone "$project" "$tmpdir/repo"
    gerrit_install_commit_msg_hook "$tmpdir/repo"

    (
        cd "$tmpdir/repo"

        while [[ $# -ge 2 ]]; do
            local f="$1"
            local content="$2"
            mkdir -p "$(dirname "$f")"
            printf '%s' "$content" > "$f"
            shift 2
        done

        _gerrit_git add -A
        _gerrit_git -c user.name="Test" -c user.email="test@example.com" \
            commit -m "$message" --no-gpg-sign >/dev/null

        local push_out
        push_out=$(_gerrit_git push origin "HEAD:refs/for/${branch}" 2>&1)

        local change_num
        change_num=$(echo "$push_out" | grep -oE '\+/[0-9]+' | head -n1 | cut -d/ -f2)

        if [[ -n "$change_num" ]]; then
            gerrit_approve_and_submit "${project}~${change_num}"
        fi
    )

    rm -rf "$tmpdir"
}

# Push a change to Gerrit and return the change number (does NOT submit).
# Arguments: project branch commit_message [file content]...
gerrit_push_change() {
    local project="$1"
    local branch="$2"
    local message="$3"
    shift 3

    local tmpdir
    tmpdir=$(mktemp -d)

    gerrit_clone "$project" "$tmpdir/repo"
    gerrit_install_commit_msg_hook "$tmpdir/repo"

    local change_num
    change_num=$(
        cd "$tmpdir/repo"

        while [[ $# -ge 2 ]]; do
            local f="$1"
            local content="$2"
            mkdir -p "$(dirname "$f")"
            printf '%s' "$content" > "$f"
            shift 2
        done

        _gerrit_git add -A
        _gerrit_git -c user.name="Test" -c user.email="test@example.com" \
            commit -m "$message" --no-gpg-sign >/dev/null

        local push_out
        push_out=$(_gerrit_git push origin "HEAD:refs/for/${branch}" 2>&1)

        echo "$push_out" >&2

        echo "$push_out" | grep -oE '\+/[0-9]+' | head -n1 | cut -d/ -f2
    )

    rm -rf "$tmpdir"
    echo "$change_num"
}

# ---------------------------------------------------------------------------
# Manifest setup helpers
# ---------------------------------------------------------------------------

# Write a default.xml manifest that references Gerrit projects.
# Usage: write_manifest <dest_dir> <gerrit_url> [project_names...]
write_manifest() {
    local dest_dir="$1"
    local gerrit_url="$2"
    shift 2

    cat > "${dest_dir}/default.xml" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="${gerrit_url}" review="${gerrit_url}" />
  <default remote="origin" revision="master" />
EOF

    for project_name in "$@"; do
        cat >> "${dest_dir}/default.xml" <<EOF
  <project name="${project_name}" path="${project_name}" />
EOF
    done

    cat >> "${dest_dir}/default.xml" <<EOF
</manifest>
EOF
}

# Set up a manifest repo in Gerrit with the given project names.
# Usage: setup_gerrit_manifest <manifest_project_name> <gerrit_url> <project_names...>
setup_gerrit_manifest() {
    local manifest_project="$1"
    local gerrit_url="$2"
    shift 2

    local tmpdir
    tmpdir=$(mktemp -d)

    gerrit_clone "$manifest_project" "$tmpdir/repo"
    gerrit_install_commit_msg_hook "$tmpdir/repo"

    write_manifest "$tmpdir/repo" "$gerrit_url" "$@"

    local change_num
    change_num=$(
        cd "$tmpdir/repo"
        _gerrit_git add -A
        _gerrit_git -c user.name="Test" -c user.email="test@example.com" \
            commit -m "Add manifest" --no-gpg-sign >/dev/null

        local push_out
        push_out=$(_gerrit_git push origin "HEAD:refs/for/master" 2>&1)

        echo "$push_out" | grep -oE '\+/[0-9]+' | head -n1 | cut -d/ -f2
    )

    if [[ -n "$change_num" ]]; then
        gerrit_approve_and_submit "${manifest_project}~${change_num}"
    fi

    rm -rf "$tmpdir"
}

# ---------------------------------------------------------------------------
# Repo binary runners (for use inside BATS tests after cd'ing)
# ---------------------------------------------------------------------------

repo_rust() {
    "$REPO_RUST_BINARY" "$@"
}

repo_python() {
    "$REPO_PYTHON_BINARY" "$@"
}

# ---------------------------------------------------------------------------
# Assertion helpers
# ---------------------------------------------------------------------------

assert_dir_exists() {
    [[ -d "$1" ]]
}

assert_file_exists() {
    [[ -f "$1" ]]
}

assert_output_contains() {
    local needle="$1"
    [[ "$output" == *"$needle"* ]]
}

# Compare two directories for structural equality (same files, same content).
# Ignores .git directories and .repo directories.
assert_dirs_equal() {
    local dir1="$1"
    local dir2="$2"

    local files1 files2
    files1=$(find "$dir1" -type f ! -path '*/.git/*' ! -path '*/.repo/*' | sort)
    files2=$(find "$dir2" -type f ! -path '*/.git/*' ! -path '*/.repo/*' | sort)

    [[ "$files1" == "$files2" ]]

    while IFS= read -r f; do
        local rel="${f#$dir1/}"
        diff -q "$f" "$dir2/$rel"
    done <<< "$files1"
}
