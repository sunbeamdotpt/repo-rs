// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Validate that a path component is safe for use in a manifest.
///
/// Rejects `..`, `.git`, `.repo*`, absolute paths, Unicode directional
/// marks, newlines, `~`, and `.` path components.
///
/// Returns `false` if the path is invalid.
pub fn is_valid_path(path: &str) -> bool {
    is_valid_path_inner(path, false, false)
}

/// Validate a path that may be a directory.
///
/// If `dir_ok` is true, trailing slashes are allowed.
pub fn is_valid_path_dir_ok(path: &str) -> bool {
    is_valid_path_inner(path, true, false)
}

/// Validate a path that may be the current directory (`.`).
///
/// If `cwd_dot_ok` is true, a single `.` is allowed.
pub fn is_valid_path_cwd_dot_ok(path: &str) -> bool {
    is_valid_path_inner(path, false, true)
}

/// Validate a path that may be a directory or the current directory.
pub fn is_valid_path_dir_cwd_dot_ok(path: &str) -> bool {
    is_valid_path_inner(path, true, true)
}

fn is_valid_path_inner(path: &str, dir_ok: bool, cwd_dot_ok: bool) -> bool {
    // Reject empty paths
    if path.is_empty() {
        return false;
    }

    // Reject ~ (due to 8.3 filenames on Windows filesystems)
    if path.contains('~') {
        return false;
    }

    // Reject bad Unicode codepoints that filesystems might elide when normalizing.
    // Cribbed from jgit's implementation.
    let bad_codepoints = [
        '\u{200c}', // ZERO WIDTH NON-JOINER
        '\u{200d}', // ZERO WIDTH JOINER
        '\u{200e}', // LEFT-TO-RIGHT MARK
        '\u{200f}', // RIGHT-TO-LEFT MARK
        '\u{202a}', // LEFT-TO-RIGHT EMBEDDING
        '\u{202b}', // RIGHT-TO-LEFT EMBEDDING
        '\u{202c}', // POP DIRECTIONAL FORMATTING
        '\u{202d}', // LEFT-TO-RIGHT OVERRIDE
        '\u{202e}', // RIGHT-TO-LEFT OVERRIDE
        '\u{2066}', // LEFT-TO-RIGHT ISOLATE
        '\u{2067}', // RIGHT-TO-LEFT ISOLATE
        '\u{2068}', // FIRST STRONG ISOLATE
        '\u{2069}', // POP DIRECTIONAL ISOLATE
        '\u{206a}', // INHIBIT SYMMETRIC SWAPPING
        '\u{206b}', // ACTIVATE SYMMETRIC SWAPPING
        '\u{206c}', // INHIBIT ARABIC FORM SHAPING
        '\u{206d}', // ACTIVATE ARABIC FORM SHAPING
        '\u{206e}', // NATIONAL DIGIT SHAPES
        '\u{206f}', // NOMINAL DIGIT SHAPES
        '\u{feff}', // ZERO WIDTH NO-BREAK SPACE
    ];
    for ch in bad_codepoints {
        if path.contains(ch) {
            return false;
        }
    }

    // Reject newlines
    if path.contains('\n') || path.contains('\r') {
        return false;
    }

    // Reject absolute paths (Unix and Windows)
    if path.starts_with('/') || path.starts_with('\\') || path.contains(":\\") {
        return false;
    }

    // Strip trailing slashes for component analysis
    let stripped = path.trim_end_matches('/').trim_end_matches('\\');
    if stripped.is_empty() {
        // Path was just slashes
        return dir_ok;
    }

    // Split by either / or \ (since Windows converts / to \)
    let parts: Vec<&str> = stripped.split(&['/', '\\'][..]).collect();

    // Check if single . is allowed
    if cwd_dot_ok && parts.len() == 1 && parts[0] == "." {
        return true;
    }

    for part in &parts {
        if *part == ".." {
            return false;
        }
        if *part == "." || *part == ".git" || part.starts_with(".repo") {
            return false;
        }
    }

    // Reject trailing slash unless dirs are allowed
    if !dir_ok && (path.ends_with('/') || path.ends_with('\\')) {
        return false;
    }

    // Check for path traversal via normalization
    let norm = stripped.replace('\\', "/");
    let norm_parts: Vec<&str> = norm.split('/').collect();
    let mut depth = 0i32;
    for part in &norm_parts {
        if *part == ".." {
            depth -= 1;
        } else if !part.is_empty() && *part != "." {
            depth += 1;
        }
        if depth < 0 {
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

        // Single dot is NOT fine by default
        assert!(!is_valid_path("."));

        // Single dot in middle is NOT fine by default
        assert!(!is_valid_path("foo/./bar"));

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

        // Trailing slash NOT allowed by default
        assert!(!is_valid_path("foo/bar/"));

        // Leading dot is fine for non-special names
        assert!(is_valid_path(".hidden"));
        assert!(is_valid_path("foo/.hidden"));

        // Single component
        assert!(is_valid_path("foo"));

        // Deep nesting
        assert!(is_valid_path("a/b/c/d/e/f/g/h/i/j"));

        // Tilde rejection
        assert!(!is_valid_path("foo~bar"));
        assert!(!is_valid_path("~foo"));

        // Windows absolute
        assert!(!is_valid_path("C:\\foo"));
        assert!(!is_valid_path("\\foo"));
    }

    #[test]
    fn dir_ok_paths() {
        assert!(is_valid_path_dir_ok("foo/bar/"));
        assert!(!is_valid_path_dir_ok("foo/bar/../"));
        assert!(!is_valid_path_dir_ok(""));
    }

    #[test]
    fn cwd_dot_ok_paths() {
        assert!(is_valid_path_cwd_dot_ok("."));
        assert!(!is_valid_path_cwd_dot_ok("./foo"));
        assert!(!is_valid_path_cwd_dot_ok("foo/.."));
    }

    #[test]
    fn dir_cwd_dot_ok_paths() {
        assert!(is_valid_path_dir_cwd_dot_ok("."));
        assert!(is_valid_path_dir_cwd_dot_ok("foo/"));
        assert!(!is_valid_path_dir_cwd_dot_ok("foo/../"));
    }

    #[test]
    fn more_bad_unicode() {
        assert!(!is_valid_path("foo\u{200c}bar")); // ZWNJ
        assert!(!is_valid_path("foo\u{200d}bar")); // ZWJ
        assert!(!is_valid_path("foo\u{200e}bar")); // LRM
        assert!(!is_valid_path("foo\u{200f}bar")); // RLM
        assert!(!is_valid_path("foo\u{206a}bar")); // ISS
        assert!(!is_valid_path("foo\u{206b}bar")); // ASS
        assert!(!is_valid_path("foo\u{206c}bar")); // IAFS
        assert!(!is_valid_path("foo\u{206d}bar")); // AAFS
        assert!(!is_valid_path("foo\u{206e}bar")); // NDS
        assert!(!is_valid_path("foo\u{206f}bar")); // NDS2
        assert!(!is_valid_path("foo\u{feff}bar")); // BOM
    }

    #[test]
    fn path_trailing_slash_dir_ok() {
        assert!(is_valid_path_dir_ok("foo/"));
        assert!(is_valid_path_dir_ok("foo/bar/"));
    }
}
