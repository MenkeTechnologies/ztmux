//! `znative` subcommand implementations. Ported from zshrs's
//! `pkg/commands.rs`, retargeted to tmux plugins: a **native** plugin is
//! `dlopen`ed through [`crate::extensions::plugin_host`], a **script** (TPM)
//! plugin is loaded by running its `*.tmux` files the way TPM does.
//!
//! Nothing here touches the command queue or any server internal — a
//! subcommand returns an [`Outcome`]: lines to print, and shell command lines
//! for [`super::cmd_znative`] to queue as `run-shell -b`. That split is what
//! keeps this module testable and free of `unsafe`.

use std::path::Path;

use super::manifest::{PluginKind, PluginManifest};
use super::store::{InstalledIndex, InstalledPlugin, Store};
use super::{Outcome, PkgError, PkgResult, resolver};

/// `znative add <SOURCE>` — resolve, install into the store, record, and load.
pub(crate) fn add(spec: &str) -> PkgResult<Outcome> {
    let store = Store::user_default()?;
    store.ensure_layout()?;

    let staged = resolver::resolve(spec, &store)?;
    let manifest = PluginManifest::load(&staged.dir)?;
    let meta = manifest.as_ref().map(|m| &m.plugin);
    let declared_name = meta.map(|m| m.name.clone()).filter(|n| !n.is_empty());
    let declared_version = meta.map(|m| m.version.clone()).filter(|v| !v.is_empty());
    let description = meta.map(|m| m.description.clone()).unwrap_or_default();
    let kind = PluginKind::detect(&staged.dir, manifest.as_ref())?;

    // Native plugins may need a build step before the cdylib exists at the
    // tree root (where the store copy will find it), and once it does exist
    // it is the authoritative source of the plugin's identity — a repository
    // called `tmux-hello` whose plugin declares itself `hello` installs as
    // `hello`, so `znative info hello` and the name in `znative loaded`
    // agree. An explicit `ztnative.toml` still wins over both.
    let mut probed: Option<(String, String)> = None;
    if let PluginKind::Native(spec) = &kind {
        let staging_name = declared_name.clone().unwrap_or_else(|| staged.name.clone());
        prepare_native(&staged.dir, spec, &staging_name)?;
        if let Some(libfile) = find_cdylib(&staged.dir) {
            let lib = staged.dir.join(libfile);
            probed = crate::extensions::plugin_host::probe(&lib.to_string_lossy()).ok();
        }
    }

    let name = declared_name
        .or_else(|| probed.as_ref().map(|(n, _)| n.clone()))
        .unwrap_or_else(|| staged.name.clone());
    let version = declared_version
        .or_else(|| probed.as_ref().map(|(_, v)| v.clone()))
        .unwrap_or_else(|| "0.0.0".into());

    // Copy the loadable subset into the content-addressed store.
    let store_path = store.install_dir(&name, &version, &staged.dir)?;
    let integrity = super::store_integrity(&store_path)?;

    // Build the index record from the store-relative load info.
    let mut entry = InstalledPlugin {
        name: name.clone(),
        version: version.clone(),
        source: staged.source.clone(),
        description,
        integrity,
        ..Default::default()
    };
    match &kind {
        PluginKind::Native(_) => {
            entry.kind = "native".into();
            entry.lib = find_cdylib(&store_path)
                .ok_or_else(|| PkgError::Resolve(format!("{name}: no cdylib after build")))?;
        }
        PluginKind::Script(s) => {
            entry.kind = "script".into();
            entry.run.clone_from(&s.run);
        }
    }

    let mut index = InstalledIndex::load_from(&store)?;
    index.upsert(entry.clone());
    index.save_to(&store)?;

    // Clean the git clone scratch — the store copy is authoritative.
    if staged.source.starts_with("github:") || staged.source.starts_with("git+") {
        let _ = std::fs::remove_dir_all(&staged.dir);
    }

    let mut outcome = load_entry(&store, &entry)?;
    let desc = if entry.description.is_empty() {
        String::new()
    } else {
        format!(" — {}", entry.description)
    };
    outcome.lines.insert(
        0,
        format!("znative: added {name}@{version} ({}){desc}", entry.kind),
    );
    Ok(outcome)
}

/// `znative remove <NAME>` — unload (native), drop the store copy + index row.
pub(crate) fn remove(name: &str) -> PkgResult<Outcome> {
    let store = Store::user_default()?;
    let mut index = InstalledIndex::load_from(&store)?;
    let Some(entry) = index.remove(name) else {
        return Err(PkgError::Other(format!("{name} is not installed")));
    };
    if entry.kind == "native" {
        let _ = crate::extensions::plugin_host::unload(name);
    }
    let dir = store.package_dir(&entry.name, &entry.version);
    if dir.is_dir() {
        std::fs::remove_dir_all(&dir)
            .map_err(|e| PkgError::Io(format!("remove {}: {e}", dir.display())))?;
    }
    index.save_to(&store)?;
    // A script plugin's key bindings and options outlive its files: the
    // server has no record of which binding came from which plugin, exactly
    // as with TPM. Say so rather than implying a clean uninstall.
    let mut outcome = Outcome::say(format!("znative: removed {name}"));
    if entry.kind == "script" {
        outcome.lines.push(format!(
            "znative: {name} was a script plugin — its key bindings and options \
             stay until the server restarts"
        ));
    }
    Ok(outcome)
}

/// `znative list` — one line per installed plugin.
pub(crate) fn list() -> PkgResult<Outcome> {
    let store = Store::user_default()?;
    let index = InstalledIndex::load_from(&store)?;
    if index.packages.is_empty() {
        return Ok(Outcome::say("znative: no plugins installed"));
    }
    let mut lines = Vec::with_capacity(index.packages.len());
    for p in &index.packages {
        // For a native plugin the loaded version is the ground truth about
        // what is running right now; a stale store copy is worth seeing.
        let loaded = match crate::extensions::plugin_host::loaded_version(&p.name) {
            Some(v) if v == p.version => "  [loaded]".to_string(),
            Some(v) => format!("  [loaded {v}]"),
            None => String::new(),
        };
        lines.push(format!(
            "{:<24} {:<10} {:<7} {}{loaded}",
            p.name, p.version, p.kind, p.source
        ));
    }
    Ok(Outcome {
        lines,
        queue: Vec::new(),
    })
}

/// `znative loaded` — the native plugins live in THIS server right now, and
/// the file each is running from. `list` reports what is installed; this
/// reports what is mapped, which is the question when a plugin was updated
/// under a running server.
pub(crate) fn loaded() -> PkgResult<Outcome> {
    let live = crate::extensions::plugin_host::list();
    if live.is_empty() {
        return Ok(Outcome::say("znative: no native plugins loaded"));
    }
    Ok(Outcome {
        lines: live
            .into_iter()
            .map(|(name, version, path)| format!("{name:<24} {version:<10} {path}"))
            .collect(),
        queue: Vec::new(),
    })
}

/// `znative info <NAME>` — full record for one plugin.
pub(crate) fn info(name: &str) -> PkgResult<Outcome> {
    let store = Store::user_default()?;
    let index = InstalledIndex::load_from(&store)?;
    let Some(p) = index.find(name) else {
        return Err(PkgError::Other(format!("{name} is not installed")));
    };
    let mut lines = vec![
        format!("name       {}", p.name),
        format!("version    {}", p.version),
        format!("kind       {}", p.kind),
        format!("source     {}", p.source),
        format!(
            "store      {}",
            store.package_dir(&p.name, &p.version).display()
        ),
    ];
    if !p.description.is_empty() {
        lines.push(format!("about      {}", p.description));
    }
    if !p.integrity.is_empty() {
        lines.push(format!("integrity  {}", p.integrity));
    }
    if !p.lib.is_empty() {
        lines.push(format!("lib        {}", p.lib));
    }
    if !p.run.is_empty() {
        lines.push(format!("run        {}", p.run.join(" ")));
    }
    // What the plugin actually has live in this server — the answer to "is it
    // loaded, and what did it give me".
    if let Some(v) = crate::extensions::plugin_host::loaded_version(&p.name) {
        lines.push(format!("loaded     {v}"));
        let (cmds, fmts, hooks) = crate::extensions::plugin_host::registrations(&p.name);
        if !cmds.is_empty() {
            lines.push(format!("commands   {}", cmds.join(" ")));
        }
        if !fmts.is_empty() {
            lines.push(format!("formats    {}", fmts.join(" ")));
        }
        if !hooks.is_empty() {
            lines.push(format!("hooks      {}", hooks.join(" ")));
        }
    }
    Ok(Outcome {
        lines,
        queue: Vec::new(),
    })
}

/// `znative load [SPEC]` — load one plugin, or every installed plugin when no
/// argument is given. Zero network for anything already in the store: this is
/// what a `.tmux.conf` calls at server start.
pub(crate) fn load(spec: Option<&str>) -> PkgResult<Outcome> {
    let store = Store::user_default()?;
    let index = InstalledIndex::load_from(&store)?;
    match spec {
        Some(n) => {
            // 1. Already installed under this name → load from the store.
            if let Some(entry) = index.find(n) {
                return load_entry(&store, entry);
            }
            // 2. `n` is a SOURCE spec — is a plugin from that source already
            //    installed? The index keys on the source label, since a repo
            //    basename usually differs from the plugin's declared name
            //    (`tmux-resurrect` → `resurrect`).
            if let Some(label) = resolver::source_label(n) {
                if let Some(entry) = index.packages.iter().find(|p| p.source == label) {
                    return load_entry(&store, entry);
                }
                // 3. Not in the store yet → install on first use, then load.
                //    This is what makes `znative load owner/repo` in
                //    `.tmux.conf` self-install on the first server start and
                //    load fast on every one after.
                return add(n);
            }
            Err(PkgError::Other(format!("{n} is not installed")))
        }
        None => {
            let mut outcome = Outcome::default();
            let mut errs = Vec::new();
            for p in &index.packages {
                match load_entry(&store, p) {
                    Ok(o) => outcome.absorb(o),
                    Err(e) => errs.push(format!("{}: {e}", p.name)),
                }
            }
            if errs.is_empty() {
                Ok(outcome)
            } else {
                Err(PkgError::Other(errs.join("; ")))
            }
        }
    }
}

/// `znative update [NAME]` — re-resolve + reinstall from the recorded source.
pub(crate) fn update(name: Option<&str>) -> PkgResult<Outcome> {
    let store = Store::user_default()?;
    let index = InstalledIndex::load_from(&store)?;
    let targets: Vec<String> = match name {
        Some(n) => vec![n.to_string()],
        None => index.packages.iter().map(|p| p.name.clone()).collect(),
    };
    if targets.is_empty() {
        return Ok(Outcome::say("znative: no plugins installed"));
    }
    let mut outcome = Outcome::default();
    for n in targets {
        let Some(p) = index.find(&n) else {
            return Err(PkgError::Other(format!("{n} is not installed")));
        };
        // A native plugin's cdylib is mapped into this server; the reinstall
        // has to release it before overwriting, or the running code and the
        // file on disk silently disagree.
        if p.kind == "native" {
            let _ = crate::extensions::plugin_host::unload(&n);
        }
        outcome.absorb(add(&source_to_spec(&p.source))?);
    }
    Ok(outcome)
}

/// `znative gc [--dry-run]` — remove every `store/<name>@<version>/`
/// directory not pinned by `installed.toml` (orphans left by old versions or
/// failed installs), plus the `git/` clone scratch.
pub(crate) fn gc(dry_run: bool) -> PkgResult<Outcome> {
    let store = Store::user_default()?;
    let index = InstalledIndex::load_from(&store)?;
    let pinned: std::collections::HashSet<String> = index
        .packages
        .iter()
        .map(|p| format!("{}@{}", p.name, p.version))
        .collect();

    let mut lines = Vec::new();
    let mut freed: u64 = 0;
    let mut count: usize = 0;

    // 1. Orphan store/<name>@<version> directories.
    if let Ok(entries) = std::fs::read_dir(store.store_dir()) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.path().is_dir() && !pinned.contains(&name) {
                let bytes = dir_size(&entry.path());
                if dry_run {
                    lines.push(format!(
                        "znative gc: would remove {name} ({} KB)",
                        kb(bytes)
                    ));
                } else {
                    std::fs::remove_dir_all(entry.path())
                        .map_err(|e| PkgError::Io(format!("remove {name}: {e}")))?;
                    lines.push(format!("znative gc: removed {name} ({} KB)", kb(bytes)));
                }
                freed += bytes;
                count += 1;
            }
        }
    }

    // 2. git/ clone scratch — the store holds the copied working tree, so the
    //    clone under git/ is dead weight after install.
    let git = store.git_dir();
    let git_bytes = dir_size(&git);
    if git_bytes > 0 {
        if dry_run {
            lines.push(format!(
                "znative gc: would clear git cache ({} KB)",
                kb(git_bytes)
            ));
        } else {
            let _ = std::fs::remove_dir_all(&git);
            lines.push(format!(
                "znative gc: cleared git cache ({} KB)",
                kb(git_bytes)
            ));
        }
        freed += git_bytes;
        count += 1;
    }

    if count == 0 {
        lines.push("znative gc: nothing to collect".into());
    } else {
        let verb = if dry_run { "would free" } else { "freed" };
        lines.push(format!("znative gc: {verb} {} KB total", kb(freed)));
    }
    Ok(Outcome {
        lines,
        queue: Vec::new(),
    })
}

/// `znative clean` — clear the scratch directories (`git/`, `cache/`, `bin/`)
/// that installs accumulate but that are not needed to load. The store and
/// index are left intact.
pub(crate) fn clean() -> PkgResult<Outcome> {
    let store = Store::user_default()?;
    let mut freed: u64 = 0;
    for d in [store.git_dir(), store.cache_dir(), store.bin_dir()] {
        if d.exists() {
            freed += dir_size(&d);
            std::fs::remove_dir_all(&d)
                .map_err(|e| PkgError::Io(format!("remove {}: {e}", d.display())))?;
        }
    }
    Ok(Outcome::say(format!(
        "znative clean: cleared {} KB of scratch",
        kb(freed)
    )))
}

/// Load one installed plugin: native by `dlopen`ing its cdylib, script by
/// queueing its `*.tmux` files the way TPM runs them.
fn load_entry(store: &Store, p: &InstalledPlugin) -> PkgResult<Outcome> {
    let dir = store.package_dir(&p.name, &p.version);
    if !store.has_package(&p.name, &p.version) {
        return Err(PkgError::Other(format!(
            "{}: store copy is missing ({}); run `znative update {}`",
            p.name,
            dir.display(),
            p.name
        )));
    }
    match p.kind.as_str() {
        "native" => {
            // Loading twice would refuse on the duplicate name; a second
            // `znative load` of an already-live plugin is a no-op, which is
            // what a re-sourced `.tmux.conf` needs.
            if crate::extensions::plugin_host::loaded_version(&p.name).is_some() {
                return Ok(Outcome::default());
            }
            let lib = dir.join(&p.lib);
            crate::extensions::plugin_host::load(&lib.to_string_lossy())
                .map(|_| Outcome::default())
                .map_err(PkgError::Resolve)
        }
        "script" => {
            // A TPM plugin talks to the server by running `tmux`; the shim
            // directory makes that this ztmux. See `Store::ensure_tmux_shim`.
            let shim = store.ensure_tmux_shim()?;
            let mut queue = Vec::with_capacity(p.run.len());
            for f in &p.run {
                let path = dir.join(f);
                if !path.is_file() {
                    return Err(PkgError::Other(format!(
                        "{}: {} is missing from the store",
                        p.name,
                        path.display()
                    )));
                }
                // `ZTMUX_SOCKET` is what points the shim at THIS server; it
                // is absent only before the server has a socket, where the
                // shim's default (the standard socket) is already right.
                let socket = match crate::extensions::plugin_host::socket_path() {
                    Some(s) => format!("ZTMUX_SOCKET={} ", sh_quote(&s)),
                    None => String::new(),
                };
                queue.push(format!(
                    "PATH={}:\"$PATH\" {}{}",
                    sh_quote(&shim.to_string_lossy()),
                    socket,
                    sh_quote(&path.to_string_lossy())
                ));
            }
            Ok(Outcome {
                lines: Vec::new(),
                queue,
            })
        }
        other => Err(PkgError::Other(format!(
            "{}: unknown plugin kind '{other}'",
            p.name
        ))),
    }
}

/// Convert a recorded provenance label back to a `znative add` spec.
fn source_to_spec(source: &str) -> String {
    match source.strip_prefix("path+file://") {
        Some(rest) => format!("path:{rest}"),
        // `github:owner/repo` and `git+URL` are already valid `add` specs.
        None => source.to_string(),
    }
}

/// Build a native plugin's cdylib into the tree root so the store copy
/// carries it (the store copy skips `target/`). If a `lib*.{dylib,so}`
/// already sits at the root, use it as-is. Runs `cargo build --release` when
/// a `Cargo.toml` exists and building is not disabled.
fn prepare_native(dir: &Path, spec: &super::manifest::NativeSpec, name: &str) -> PkgResult<()> {
    if find_cdylib(dir).is_some() {
        return Ok(()); // prebuilt cdylib already at the root.
    }
    let has_cargo = dir.join("Cargo.toml").is_file();
    let want_build = spec.build.unwrap_or(has_cargo);
    if !want_build {
        return Err(PkgError::Resolve(format!(
            "{name}: native plugin has no prebuilt cdylib and build is disabled"
        )));
    }
    if !has_cargo {
        return Err(PkgError::Resolve(format!(
            "{name}: native plugin has neither a cdylib nor a Cargo.toml to build"
        )));
    }
    let out = std::process::Command::new("cargo")
        .current_dir(dir)
        .arg("build")
        .arg("--release")
        .output()
        .map_err(|e| PkgError::Resolve(format!("cargo build: {e} (is cargo installed?)")))?;
    if !out.status.success() {
        return Err(PkgError::Resolve(format!(
            "{name}: cargo build failed:\n{}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    // Copy the built cdylib from target/release to the tree root.
    let rel = dir.join("target").join("release");
    let built = find_cdylib(&rel).ok_or_else(|| {
        PkgError::Resolve(format!(
            "{name}: cargo build produced no cdylib in {} (need crate-type=[\"cdylib\"])",
            rel.display()
        ))
    })?;
    std::fs::copy(rel.join(&built), dir.join(&built))
        .map_err(|e| PkgError::Io(format!("stage cdylib: {e}")))?;
    Ok(())
}

/// Find a `lib*.{dylib,so}` (or `*.dll`) filename directly inside `dir`.
fn find_cdylib(dir: &Path) -> Option<String> {
    let suffix = std::env::consts::DLL_SUFFIX; // .dylib / .so / .dll
    let rd = std::fs::read_dir(dir).ok()?;
    for entry in rd.flatten() {
        let n = entry.file_name().to_string_lossy().into_owned();
        if n.ends_with(suffix) && n.starts_with(std::env::consts::DLL_PREFIX) {
            return Some(n);
        }
    }
    None
}

/// Recursive byte size of a directory tree (0 if unreadable).
fn dir_size(path: &Path) -> u64 {
    let mut total: u64 = 0;
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            total += dir_size(&entry.path());
        } else if meta.is_file() {
            total += meta.len();
        }
    }
    total
}

/// Bytes as rounded kilobytes, for the `gc`/`clean` reports.
fn kb(bytes: u64) -> u64 {
    bytes.div_ceil(1024)
}

/// POSIX single-quoting, so a store path with a space or a shell
/// metacharacter survives `/bin/sh -c`. `'` closes the quote, escapes
/// itself, and reopens.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sh_quote_survives_metacharacters() {
        assert_eq!(sh_quote("/a/b c"), "'/a/b c'");
        assert_eq!(sh_quote("/it's"), r"'/it'\''s'");
        assert_eq!(sh_quote("/a$b;rm -rf /"), "'/a$b;rm -rf /'");
    }

    #[test]
    fn source_spec_round_trips() {
        assert_eq!(source_to_spec("github:o/r"), "github:o/r");
        assert_eq!(source_to_spec("git+https://x/y.git"), "git+https://x/y.git");
        assert_eq!(source_to_spec("path+file:///tmp/p"), "path:/tmp/p");
    }

    /// `gc` reports whole kilobytes; a byte of garbage must not round to
    /// "0 KB freed" and read as "nothing happened".
    #[test]
    fn kb_rounds_up() {
        assert_eq!(kb(0), 0);
        assert_eq!(kb(1), 1);
        assert_eq!(kb(1024), 1);
        assert_eq!(kb(1025), 2);
    }
}
