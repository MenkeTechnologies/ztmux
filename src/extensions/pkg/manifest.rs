//! A plugin's optional `ztnative.toml`, declaring how the plugin is loaded.
//!
//! Ported from zshrs's `pkg/manifest.rs`, retargeted to the two ztmux plugin
//! kinds. When a plugin repo ships no `ztnative.toml`, [`PluginKind::detect`]
//! infers the kind from the tree — so an ordinary TPM plugin (a repo with a
//! `*.tmux` file and nothing else) installs with no metadata at all.
//!
//! Schema:
//! ```toml
//! [plugin]
//! name = "battery"
//! version = "0.1.0"
//! description = "battery status in the status line"
//!
//! # Native (Rust cdylib) plugin — dlopened through the ztnative ABI:
//! [native]
//! lib = "battery"          # produces lib<lib>.{dylib,so}
//!
//! # …OR script (TPM) plugin — the *.tmux files the server runs:
//! # [script]
//! # run = ["battery.tmux"]
//! ```

use serde::{Deserialize, Serialize};
use std::path::Path;

use super::{PkgError, PkgResult};

/// Manifest filename, at the root of a plugin's tree.
pub(crate) const MANIFEST_FILE: &str = "ztnative.toml";

/// Parsed `ztnative.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct PluginManifest {
    /// `[plugin]` metadata.
    #[serde(default)]
    pub(crate) plugin: PluginMeta,
    /// `[native]` — present for Rust cdylib plugins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) native: Option<NativeSpec>,
    /// `[script]` — present for TPM-style script plugins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) script: Option<ScriptSpec>,
}

/// `[plugin]` table.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct PluginMeta {
    /// `name` — defaults to the source basename when absent.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) name: String,
    /// `version` — defaults to `"0.0.0"` when absent.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) version: String,
    /// One-line description (shown by `znative list`/`info`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) description: String,
}

/// `[native]` — a Rust cdylib plugin using the `ztnative` SDK.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct NativeSpec {
    /// Library file stem — produces `lib<lib>.{dylib,so}`. When empty the
    /// installer infers it from the built artifact.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) lib: String,
    /// When true, run `cargo build --release` in the staged tree before
    /// looking for the cdylib. Defaults to true when a `Cargo.toml` exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) build: Option<bool>,
}

/// `[script]` — a TPM-style script plugin.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ScriptSpec {
    /// Files to run, in order, relative to the plugin root. TPM's contract:
    /// each is an executable that drives the server through `tmux …` calls.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) run: Vec<String>,
}

/// Resolved plugin kind — either an explicit `ztnative.toml` or an inferred
/// layout.
#[derive(Debug, Clone)]
pub(crate) enum PluginKind {
    /// Rust cdylib: the `[native]` spec.
    Native(NativeSpec),
    /// TPM script plugin: the `*.tmux` files to run.
    Script(ScriptSpec),
}

impl PluginManifest {
    /// Parse a `ztnative.toml` string.
    pub(crate) fn parse(s: &str) -> PkgResult<PluginManifest> {
        toml::from_str::<PluginManifest>(s)
            .map_err(|e| PkgError::Manifest(format!("{MANIFEST_FILE}: {}", e.message())))
    }

    /// Load a plugin's `ztnative.toml` if present at `dir/ztnative.toml`.
    pub(crate) fn load(dir: &Path) -> PkgResult<Option<PluginManifest>> {
        let path = dir.join(MANIFEST_FILE);
        if !path.is_file() {
            return Ok(None);
        }
        let s = std::fs::read_to_string(&path)
            .map_err(|e| PkgError::Io(format!("read {}: {e}", path.display())))?;
        Ok(Some(PluginManifest::parse(&s)?))
    }
}

impl PluginKind {
    /// Determine the plugin kind for a staged tree. Prefers an explicit
    /// `ztnative.toml` (`[native]` beats `[script]` when both are present),
    /// then falls back to layout detection:
    ///
    /// 1. A prebuilt `lib*.{dylib,so}` at the root, or a `Cargo.toml` whose
    ///    crate-type mentions `cdylib` → [`PluginKind::Native`].
    /// 2. Any `*.tmux` file at the root → [`PluginKind::Script`], which is
    ///    every TPM plugin ever published.
    ///
    /// Returns [`PkgError::Unknown`] when nothing matches.
    pub(crate) fn detect(dir: &Path, manifest: Option<&PluginManifest>) -> PkgResult<PluginKind> {
        if let Some(m) = manifest {
            if let Some(n) = &m.native {
                return Ok(PluginKind::Native(n.clone()));
            }
            if let Some(s) = &m.script {
                return Ok(PluginKind::Script(s.clone()));
            }
        }
        // Layout detection — native first: a repo can carry both a build tree
        // and helper scripts, but if it declares a cdylib it is native.
        if has_cdylib(dir) || cargo_is_cdylib(dir) {
            return Ok(PluginKind::Native(NativeSpec::default()));
        }
        let mut run: Vec<String> = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let name = entry?.file_name().to_string_lossy().into_owned();
            if name.ends_with(".tmux") {
                run.push(name);
            }
        }
        if run.is_empty() {
            return Err(PkgError::Unknown(format!(
                "could not determine plugin kind: no {MANIFEST_FILE}, no *.tmux, \
                 and no Rust cdylib/Cargo.toml"
            )));
        }
        run.sort();
        Ok(PluginKind::Script(ScriptSpec { run }))
    }
}

/// True if a `lib*.{dylib,so}` exists at the tree root (a prebuilt cdylib).
fn has_cdylib(dir: &Path) -> bool {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in rd.flatten() {
        let n = entry.file_name();
        let n = n.to_string_lossy();
        if n.starts_with("lib") && (n.ends_with(".dylib") || n.ends_with(".so")) {
            return true;
        }
    }
    false
}

/// True if `Cargo.toml` declares a `cdylib` crate-type (so `cargo build`
/// produces a dlopen-able library).
fn cargo_is_cdylib(dir: &Path) -> bool {
    let Ok(s) = std::fs::read_to_string(dir.join("Cargo.toml")) else {
        return false;
    };
    s.contains("cdylib")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_native_manifest() {
        let m = PluginManifest::parse("[plugin]\nname='x'\nversion='0.1.0'\n[native]\nlib='foo'\n")
            .unwrap();
        assert_eq!(m.plugin.name, "x");
        assert_eq!(m.native.unwrap().lib, "foo");
    }

    #[test]
    fn parses_script_manifest() {
        let m =
            PluginManifest::parse("[plugin]\nname='y'\n[script]\nrun=['y.tmux','z.tmux']\n").unwrap();
        assert_eq!(m.script.unwrap().run, vec!["y.tmux", "z.tmux"]);
    }

    /// An unmodified TPM plugin — a repo with one `*.tmux` file and no
    /// metadata — must detect as a script plugin, since that is the entire
    /// published tmux plugin ecosystem.
    #[test]
    fn detects_unmodified_tpm_plugin() {
        let dir = super::super::store::tests_support::tmp_dir("manifest-tpm");
        std::fs::write(dir.join("fzf-url.tmux"), b"#!/usr/bin/env bash\n").unwrap();
        std::fs::write(dir.join("fzf-url.sh"), b"#!/usr/bin/env bash\n").unwrap();
        std::fs::write(dir.join("README.md"), b"docs").unwrap();
        match PluginKind::detect(&dir, None).unwrap() {
            PluginKind::Script(s) => assert_eq!(s.run, vec!["fzf-url.tmux"]),
            PluginKind::Native(_) => panic!("TPM plugin detected as native"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A cdylib crate detects as native even though it also ships a `*.tmux`
    /// helper — the compiled plugin is the real entry point.
    #[test]
    fn cdylib_beats_tmux_file() {
        let dir = super::super::store::tests_support::tmp_dir("manifest-native");
        std::fs::write(dir.join("Cargo.toml"), b"[lib]\ncrate-type=[\"cdylib\"]\n").unwrap();
        std::fs::write(dir.join("extra.tmux"), b"#!/bin/sh\n").unwrap();
        assert!(matches!(
            PluginKind::detect(&dir, None).unwrap(),
            PluginKind::Native(_)
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A repo with neither entry point is a hard error, not a silent
    /// half-install.
    #[test]
    fn undeterminable_kind_is_an_error() {
        let dir = super::super::store::tests_support::tmp_dir("manifest-empty");
        std::fs::write(dir.join("README.md"), b"docs").unwrap();
        assert!(PluginKind::detect(&dir, None).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
