//! Shared git subprocess helper for the git-family extensions.
//!
//! `git`, `remote`, `ahead`, `changes`, and `stash` all shell out to git against
//! a pane's working directory in exactly the same way; this is the one place that
//! invocation lives so they cannot drift apart.

use std::path::Path;
use std::process::Command;

/// One `git -C <path> <args…>` invocation; the trimmed stdout on success, else
/// `None` on any failure (not a repo, git missing, non-zero exit).
pub(crate) fn git_out(path: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The last path component of a repo root (`/home/u/proj` → `proj`), for the
/// compact REPO column shared by the git-family extensions.
pub(crate) fn repo_name(root: &str) -> String {
    Path::new(root)
        .file_name()
        .map_or_else(|| root.to_string(), |n| n.to_string_lossy().into_owned())
}
