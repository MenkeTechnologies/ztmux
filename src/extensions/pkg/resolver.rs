//! Resolve a user-supplied source spec into a staged plugin directory.
//! Ported from zshrs's `pkg/resolver.rs` — the source forms are the same ones
//! TPM users already type (`owner/repo`), plus the explicit forms.
//!
//! Source forms accepted by `znative add <SOURCE>`:
//! - `owner/repo` or `github:owner/repo` → `git clone https://github.com/owner/repo`
//! - `git+URL`, or any URL ending in `.git` → `git clone URL`
//! - `path:DIR`, an absolute path, or `./rel`, `../rel`, `~/dir` → a local
//!   directory, used in place (no network)
//!
//! `@REF` may be appended to a git/github source to pin a branch/tag/commit
//! (`owner/repo@v1.2.0`). The resolver clones into `$ZTMUX_HOME/pkg/git/` and
//! returns the working tree; the caller copies the loadable subset into the
//! content-addressed store.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::store::Store;
use super::{PkgError, PkgResult};

/// A staged source ready to install into the store.
pub(crate) struct Staged {
    /// Working directory containing the plugin tree.
    pub(crate) dir: PathBuf,
    /// Inferred plugin name (repo/dir basename).
    pub(crate) name: String,
    /// Provenance label recorded in the index: `github:owner/repo`,
    /// `git+URL`, or `path+file://DIR`.
    pub(crate) source: String,
}

/// Resolve `spec` into a [`Staged`] tree. Clones (git/github) land under
/// `store.git_dir()`; local paths are used in place.
pub(crate) fn resolve(spec: &str, store: &Store) -> PkgResult<Staged> {
    let (base, git_ref) = split_ref(spec);

    // Local path forms.
    if let Some(p) = local_path(base) {
        let dir = p
            .canonicalize()
            .map_err(|e| PkgError::Resolve(format!("path {}: {e}", p.display())))?;
        if !dir.is_dir() {
            return Err(PkgError::Resolve(format!(
                "path {} is not a directory",
                dir.display()
            )));
        }
        let name = basename(&dir);
        let source = format!("path+file://{}", dir.display());
        return Ok(Staged { dir, name, source });
    }

    // Git / GitHub forms.
    let (url, label, name) = git_url(base)?;
    store
        .ensure_layout()
        .map_err(|e| PkgError::Resolve(e.to_string()))?;
    let dir = store.git_dir().join(&name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .map_err(|e| PkgError::Io(format!("clear {}: {e}", dir.display())))?;
    }
    git_clone(&url, &dir, git_ref)?;
    Ok(Staged {
        dir,
        name,
        // Record the pinned ref in the source so `update` re-fetches the SAME
        // version and `load owner/repo@REF` matches only that pin.
        source: label_with_ref(label, git_ref),
    })
}

/// Append `@REF` to a provenance label when a version/ref was pinned, so the
/// recorded source round-trips back through `resolve`/`split_ref`.
fn label_with_ref(label: String, git_ref: Option<&str>) -> String {
    match git_ref {
        Some(r) => format!("{label}@{r}"),
        None => label,
    }
}

/// The provenance label a `spec` WOULD receive, computed WITHOUT cloning or
/// network access. Used by `znative load <spec>` to check whether a source is
/// already installed (the index keys on this label, since a repo's basename
/// often differs from its `ztnative.toml` plugin name — e.g. `tmux-resurrect`
/// → `resurrect`). Returns `None` for a bare plugin name (not a source form).
pub(crate) fn source_label(spec: &str) -> Option<String> {
    let (base, git_ref) = split_ref(spec);
    if let Some(p) = local_path(base) {
        // Match the `path+file://<canonical>` the installer records.
        let dir = p.canonicalize().ok()?;
        return Some(format!("path+file://{}", dir.display()));
    }
    git_url(base)
        .ok()
        .map(|(_url, label, _name)| label_with_ref(label, git_ref))
}

/// Split a trailing `@REF` (branch/tag/commit) off a spec. Only splits on an
/// `@` that comes after the last `/`, so `git@host:owner/repo.git` SSH URLs
/// keep their `@`.
fn split_ref(spec: &str) -> (&str, Option<&str>) {
    if let Some(at) = spec.rfind('@') {
        let after_slash = spec.rfind('/').is_none_or(|s| at > s);
        if after_slash && at + 1 < spec.len() {
            return (&spec[..at], Some(&spec[at + 1..]));
        }
    }
    (spec, None)
}

/// Recognize local-path forms; returns the path when `spec` is one.
fn local_path(spec: &str) -> Option<PathBuf> {
    if let Some(rest) = spec.strip_prefix("path:") {
        return Some(PathBuf::from(rest));
    }
    if spec.starts_with('/')
        || spec.starts_with("./")
        || spec.starts_with("../")
        || spec.starts_with('~')
    {
        let expanded = match (spec.strip_prefix("~/"), std::env::var_os("HOME")) {
            (Some(rest), Some(home)) if !home.is_empty() => PathBuf::from(home).join(rest),
            _ => PathBuf::from(spec),
        };
        return Some(expanded);
    }
    None
}

/// Map a non-local spec to `(clone_url, provenance_label, name)`.
fn git_url(spec: &str) -> PkgResult<(String, String, String)> {
    if let Some(rest) = spec.strip_prefix("git+") {
        let name = repo_basename(rest);
        return Ok((rest.to_string(), format!("git+{rest}"), name));
    }
    if let Some(rest) = spec.strip_prefix("github:") {
        let owner_repo = rest.trim_end_matches(".git");
        let url = format!("https://github.com/{owner_repo}");
        let name = repo_basename(&url);
        return Ok((url, format!("github:{owner_repo}"), name));
    }
    if spec.ends_with(".git") || spec.contains("://") {
        let name = repo_basename(spec);
        return Ok((spec.to_string(), format!("git+{spec}"), name));
    }
    // `owner/repo` shorthand → GitHub. This is what every TPM user types.
    if spec.split('/').count() == 2 && !spec.contains(' ') {
        let owner_repo = spec.trim_end_matches(".git");
        let url = format!("https://github.com/{owner_repo}");
        let name = repo_basename(&url);
        return Ok((url, format!("github:{owner_repo}"), name));
    }
    Err(PkgError::Resolve(format!(
        "unrecognized source '{spec}': expected owner/repo, github:owner/repo, \
         git+URL, or a local path"
    )))
}

/// `git clone --depth 1 [--branch REF] URL DIR` — shallow for speed.
fn git_clone(url: &str, dir: &Path, git_ref: Option<&str>) -> PkgResult<()> {
    let mut cmd = Command::new("git");
    cmd.arg("clone").arg("--depth").arg("1");
    if let Some(r) = git_ref {
        cmd.arg("--branch").arg(r);
    }
    cmd.arg(url).arg(dir);
    let out = cmd
        .output()
        .map_err(|e| PkgError::Resolve(format!("git clone: {e} (is git installed?)")))?;
    if !out.status.success() {
        // Retry without --branch: a REF that is a commit sha cannot be used
        // with `--branch` on a shallow clone. Fall back to a full clone +
        // checkout.
        if let Some(r) = git_ref {
            return git_clone_checkout(url, dir, r);
        }
        return Err(PkgError::Resolve(format!(
            "git clone {url} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(())
}

/// Full clone + `git checkout REF` — the fallback when a shallow `--branch`
/// clone cannot reach an arbitrary commit.
fn git_clone_checkout(url: &str, dir: &Path, git_ref: &str) -> PkgResult<()> {
    if dir.exists() {
        let _ = std::fs::remove_dir_all(dir);
    }
    let out = Command::new("git")
        .arg("clone")
        .arg(url)
        .arg(dir)
        .output()
        .map_err(|e| PkgError::Resolve(format!("git clone: {e}")))?;
    if !out.status.success() {
        return Err(PkgError::Resolve(format!(
            "git clone {url} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    let out = Command::new("git")
        .current_dir(dir)
        .arg("checkout")
        .arg(git_ref)
        .output()
        .map_err(|e| PkgError::Resolve(format!("git checkout: {e}")))?;
    if !out.status.success() {
        return Err(PkgError::Resolve(format!(
            "git checkout {git_ref} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(())
}

/// Basename of a directory path, sans trailing separators.
fn basename(p: &Path) -> String {
    p.file_name()
        .map_or_else(|| "plugin".into(), |s| s.to_string_lossy().into_owned())
}

/// Repo name from a clone URL: strip `.git`, take the last path segment.
fn repo_basename(url: &str) -> String {
    url.trim_end_matches('/')
        .trim_end_matches(".git")
        .rsplit(['/', ':'])
        .next()
        .unwrap_or("plugin")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_ref_only_after_last_slash() {
        assert_eq!(split_ref("o/r@v1"), ("o/r", Some("v1")));
        assert_eq!(split_ref("o/r"), ("o/r", None));
        // SSH URL @ must not split.
        assert_eq!(
            split_ref("git@github.com:o/r.git"),
            ("git@github.com:o/r.git", None)
        );
        // A trailing bare `@` is not a ref.
        assert_eq!(split_ref("o/r@"), ("o/r@", None));
    }

    #[test]
    fn git_url_forms() {
        let (u, l, n) = git_url("tmux-plugins/tmux-resurrect").unwrap();
        assert_eq!(u, "https://github.com/tmux-plugins/tmux-resurrect");
        assert_eq!(l, "github:tmux-plugins/tmux-resurrect");
        assert_eq!(n, "tmux-resurrect");
        let (u, _, n) = git_url("github:a/b").unwrap();
        assert_eq!(u, "https://github.com/a/b");
        assert_eq!(n, "b");
        let (u, l, _) = git_url("git+https://x.com/y.git").unwrap();
        assert_eq!(u, "https://x.com/y.git");
        assert_eq!(l, "git+https://x.com/y.git");
        assert!(git_url("not a source").is_err());
    }

    #[test]
    fn local_path_forms() {
        assert!(local_path("path:/tmp/x").is_some());
        assert!(local_path("/abs").is_some());
        assert!(local_path("./rel").is_some());
        assert!(local_path("owner/repo").is_none());
    }

    /// The label `load` matches an installed plugin by must round-trip
    /// through the pin, or `znative load owner/repo@v1` would reinstall on
    /// every server start.
    #[test]
    fn source_label_round_trips_a_pin() {
        assert_eq!(
            source_label("tmux-plugins/tmux-sensible@v3.0.0").unwrap(),
            "github:tmux-plugins/tmux-sensible@v3.0.0"
        );
        assert_eq!(
            source_label("github:o/r").unwrap(),
            source_label("o/r").unwrap()
        );
        assert!(source_label("bare-name").is_none());
    }
}
