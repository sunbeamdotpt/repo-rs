// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Callback trait for reporting progress of long-running git operations.
pub trait ProgressCallback: Send + Sync {
    /// Report progress.
    ///
    /// `current` and `total` are in arbitrary units defined by the
    /// operation. If `total` is `None`, the total is unknown.
    fn report(&self, operation: &str, current: u64, total: Option<u64>, message: &str);
}

impl ProgressCallback for () {
    fn report(&self, _operation: &str, _current: u64, _total: Option<u64>, _message: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unit_progress_callback() {
        // Should compile and run without panicking.
        ().report("test", 1, Some(10), "msg");
    }
}
