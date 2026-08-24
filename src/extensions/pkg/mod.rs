//! ztmux plugin package manager (`znative`) — a GLOBAL-only package manager
//! for tmux script plugins AND native (Rust cdylib) plugins.
//!
//! Ported from zshrs's `znative` (`zshrs/src/extensions/pkg/`), retargeted
//! from zsh plugins to tmux plugins: the store, the index, the source-spec
//! resolver, and the SHA-256 integrity pinning are the same design; what a
//! plugin *is* changed. A **script** plugin is a TPM plugin — a repo with a
//! `*.tmux` file that the server runs, which is every tmux plugin published
//! today. A **native** plugin is a `cdylib` built against the [`ztnative`]
//! ABI, which the server `dlopen`s to register real tmux commands, `#{…}`
//! formats, and hook subscriptions ([`super::plugin_host`]).
//!
//! It is **global only**: one content-addressed store under `$ZTMUX_HOME/pkg/`,
//! no per-project manifest or lockfile. The whole workflow is one line per
//! plugin in `.tmux.conf`:
//!
//! ```text
//! znative load owner/repo
//! ```
//!
//! On the first server start that installs the plugin and loads it; on every
//! start after, the same line loads it from the store with no network. There
//! is no separate install step. `znative` needs `git` on `PATH` for remote
//! sources, and `cargo` for native plugins that ship as source.
//!
//! Surface:
//! - [`manifest`] — a plugin's optional `ztnative.toml`
//!   (`[plugin]`/`[native]`/`[script]`); auto-detected when absent, so an
//!   unmodified TPM plugin installs with no metadata at all.
//! - [`store`]    — `$ZTMUX_HOME/pkg/{store,cache,git,bin}/` layout + the
//!   `installed.toml` global index.
//! - [`resolver`] — turn a source spec (`owner/repo`, `git+URL`, `path:DIR`)
//!   into a staged directory ready to install.
//! - [`commands`] — `znative {add,remove,list,info,load,update,gc,clean}`.
//! - [`cmd_znative`] — the `znative` tmux command itself.

pub(crate) mod cmd_znative;
pub(crate) mod commands;
pub(crate) mod manifest;
pub(crate) mod resolver;
pub(crate) mod store;

/// Result alias used throughout the package manager. Errors are
/// stringly-typed (one user-facing diagnostic per failure path), reported to
/// the client as `znative: <reason>` with the command failing — matching
/// tmux's terse error style.
pub(crate) type PkgResult<T> = Result<T, PkgError>;

/// Errors emitted by the package manager. `Display` produces the one-line
/// reason (no `znative:` prefix — the command adds it).
#[derive(Debug)]
pub(crate) enum PkgError {
    /// File I/O — read/write/create/copy.
    Io(String),
    /// Manifest parse error (bad TOML in a plugin's `ztnative.toml`).
    Manifest(String),
    /// Resolver error — unknown source form, clone/build failure.
    Resolve(String),
    /// The plugin kind could not be determined (no `ztnative.toml`, no
    /// `*.tmux`, no cdylib/`Cargo.toml`).
    Unknown(String),
    /// Generic runtime error.
    Other(String),
}

impl std::fmt::Display for PkgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PkgError::Io(s)
            | PkgError::Manifest(s)
            | PkgError::Resolve(s)
            | PkgError::Unknown(s)
            | PkgError::Other(s) => write!(f, "{s}"),
        }
    }
}

impl From<std::io::Error> for PkgError {
    fn from(e: std::io::Error) -> Self {
        PkgError::Io(e.to_string())
    }
}

/// What a subcommand produced: text for the client, and tmux commands to
/// queue. Keeping the queue out of the command implementations is what lets
/// them stay free of server internals — [`cmd_znative`] is the only place
/// that touches the command queue.
#[derive(Default)]
pub(crate) struct Outcome {
    /// Lines to print to the client that ran `znative`, in order.
    pub(crate) lines: Vec<String>,
    /// tmux command text to parse and queue, in order. Script (TPM) plugins
    /// load as `run-shell -b` of their `*.tmux` files.
    pub(crate) queue: Vec<String>,
}

impl Outcome {
    /// An outcome that only prints.
    pub(crate) fn say(line: impl Into<String>) -> Outcome {
        Outcome {
            lines: vec![line.into()],
            queue: Vec::new(),
        }
    }

    /// Fold `other` onto the end of this outcome — used where one subcommand
    /// runs another (`load` falling through to `add`, `update` reinstalling
    /// several plugins).
    pub(crate) fn absorb(&mut self, other: Outcome) {
        self.lines.extend(other.lines);
        self.queue.extend(other.queue);
    }
}

/// Deterministic SHA-256 of a directory tree, `sha256-<hex>`. Files are
/// walked in sorted order so the hash is stable regardless of filesystem
/// iteration; each file contributes `<relpath>\0F\0<len>\n<bytes>\n`,
/// symlinks their target. Recorded in the install index for change detection
/// and audit.
pub(crate) fn store_integrity(root: &std::path::Path) -> PkgResult<String> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    let mut entries: Vec<std::path::PathBuf> = Vec::new();
    fn walk(
        root: &std::path::Path,
        cur: &std::path::Path,
        out: &mut Vec<std::path::PathBuf>,
    ) -> PkgResult<()> {
        for entry in std::fs::read_dir(cur)? {
            let entry = entry?;
            let path = entry.path();
            let meta = entry.metadata()?;
            if meta.is_dir() && !meta.file_type().is_symlink() {
                walk(root, &path, out)?;
            } else {
                out.push(path.strip_prefix(root).unwrap_or(&path).to_path_buf());
            }
        }
        Ok(())
    }
    walk(root, root, &mut entries)?;
    entries.sort();
    for rel in &entries {
        let abs = root.join(rel);
        let meta = std::fs::symlink_metadata(&abs)?;
        let rel_s = rel.to_string_lossy();
        if meta.file_type().is_symlink() {
            let target = std::fs::read_link(&abs)?;
            hasher.update(rel_s.as_bytes());
            hasher.update(b"\0L\0");
            hasher.update(target.to_string_lossy().as_bytes());
            hasher.update(b"\n");
        } else if meta.is_file() {
            let bytes = std::fs::read(&abs)?;
            hasher.update(rel_s.as_bytes());
            hasher.update(b"\0F\0");
            hasher.update(bytes.len().to_string().as_bytes());
            hasher.update(b"\n");
            hasher.update(&bytes);
            hasher.update(b"\n");
        }
    }
    Ok(format!("sha256-{:x}", hasher.finalize()))
}
