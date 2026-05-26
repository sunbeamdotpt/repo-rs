// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Validate that a path component is safe for use in a manifest.
///
/// Rejects `..`, `.git`, `.repo*`, absolute paths, Unicode directional
/// marks, and newlines.
///
/// Returns `false` if the path is invalid.
pub fn is_valid_path(path: &str) -> bool {
    // Reject absolute paths
    if path.starts_with('/') {
        return false;
    }

    // Reject empty paths
    if path.is_empty() {
        return false;
    }

    for component in path.split('/') {
        // Reject parent directory references
        if component == ".." {
            return false;
        }

        // Reject .git and .repo*
        if component == ".git" || component.starts_with(".repo") {
            return false;
        }

        // Reject Unicode directional marks
        if component.contains('\u{202A}')
            || component.contains('\u{202B}')
            || component.contains('\u{202C}')
            || component.contains('\u{202D}')
            || component.contains('\u{202E}')
            || component.contains('\u{2066}')
            || component.contains('\u{2067}')
            || component.contains('\u{2068}')
            || component.contains('\u{2069}')
        {
            return false;
        }

        // Reject newlines
        if component.contains('\n') || component.contains('\r') {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_paths() {
        assert!(is_valid_path("platform/build"));
        assert!(is_valid_path("foo/bar/baz"));
        assert!(is_valid_path(".gitignore")); // .gitignore is fine, .git is not
    }

    #[test]
    fn invalid_paths() {
        assert!(!is_valid_path("../foo"));
        assert!(!is_valid_path("foo/../bar"));
        assert!(!is_valid_path(".git"));
        assert!(!is_valid_path(".repo"));
        assert!(!is_valid_path(".repo_test"));
        assert!(!is_valid_path("/absolute"));
        assert!(!is_valid_path("foo\nbar"));
        assert!(!is_valid_path("foo\rbar"));
        assert!(!is_valid_path("foo\u{202A}bar")); // LRE
    }

    #[test]
    fn path_safety_edge_cases() {
        // Empty path
        assert!(!is_valid_path(""));

        // Single dot is fine
        assert!(is_valid_path("."));

        // Single dot in middle is fine
        assert!(is_valid_path("foo/./bar"));

        // Double dot at start
        assert!(!is_valid_path("../foo"));

        // Double dot at end
        assert!(!is_valid_path("foo/.."));

        // Double dot in middle
        assert!(!is_valid_path("foo/../bar"));

        // Triple dot is fine
        assert!(is_valid_path("foo/.../bar"));

        // Absolute path
        assert!(!is_valid_path("/foo/bar"));

        // .git anywhere
        assert!(!is_valid_path(".git"));
        assert!(!is_valid_path("foo/.git"));
        assert!(!is_valid_path("foo/.git/bar"));

        // .repo anywhere
        assert!(!is_valid_path(".repo"));
        assert!(!is_valid_path("foo/.repo"));
        assert!(!is_valid_path("foo/.repo_test"));
        assert!(!is_valid_path("foo/.repo/bar"));

        // .gitignore is fine, but .repo* is rejected
        assert!(is_valid_path(".gitignore"));

        // Newlines
        assert!(!is_valid_path("foo\nbar"));
        assert!(!is_valid_path("foo\rbar"));
        assert!(!is_valid_path("foo\r\nbar"));

        // Unicode directional marks (all categories)
        assert!(!is_valid_path("foo\u{202A}bar")); // LRE
        assert!(!is_valid_path("foo\u{202B}bar")); // RLE
        assert!(!is_valid_path("foo\u{202C}bar")); // PDF
        assert!(!is_valid_path("foo\u{202D}bar")); // LRO
        assert!(!is_valid_path("foo\u{202E}bar")); // RLO
        assert!(!is_valid_path("foo\u{2066}bar")); // LRI
        assert!(!is_valid_path("foo\u{2067}bar")); // RLI
        assert!(!is_valid_path("foo\u{2068}bar")); // FSI
        assert!(!is_valid_path("foo\u{2069}bar")); // PDI

        // Normal Unicode is fine
        assert!(is_valid_path("foo/日本語/bar"));
        assert!(is_valid_path("foo/émojis/bar"));

        // Trailing slash (empty last component) - this is allowed by current logic
        assert!(is_valid_path("foo/bar/"));

        // Leading dot is fine for non-special names
        assert!(is_valid_path(".hidden"));
        assert!(is_valid_path("foo/.hidden"));

        // Single component
        assert!(is_valid_path("foo"));

        // Deep nesting
        assert!(is_valid_path("a/b/c/d/e/f/g/h/i/j"));
    }
}
