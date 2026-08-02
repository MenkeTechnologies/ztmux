// ztmux has no C libraries to find or link: the event loop lives in
// src/extensions/event_loop (Rust, replacing libevent) and terminfo is read by
// terminfo-lean. All this build script does is generate the command grammar and
// the REPL's completion tables, so a build needs nothing but a Rust toolchain —
// no pkg-config probe, no Homebrew prefix, no -dev packages.

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    println!("cargo::rerun-if-changed=src/cmd_parse.lalrpop");
    lalrpop::process_root().unwrap();
    completions::generate();
}

/// Harvest the shipped zsh completion (`completions/_ztmux`) into Rust tables
/// used by `ztmux repl`'s Tab completion (`src/extensions/repl.rs`).
///
/// The zsh completion is itself generated from `scripts/gen_zsh_completion.py`,
/// so reading it here keeps one source of truth for verb descriptions and for
/// the extension verbs' options: what `ztmux <Tab>` offers in the shell is
/// exactly what the REPL offers. (The *ported* commands' flags are not taken
/// from here — the REPL reads those straight off each `cmd_entry`'s own
/// `args_parse` template, which cannot drift from the parser at all.)
mod completions {
    use super::*;

    /// Everything harvested for one `_ztmux-<verb>()` block.
    #[derive(Default)]
    struct Verb {
        name: String,
        description: String,
        /// `-o`, `--json`, … in the order the completion lists them.
        options: Vec<String>,
        /// Literal value vocabulary per option, e.g. `-o` → `json`.
        option_values: Vec<(String, Vec<String>)>,
        /// Literal vocabulary for a positional argument, e.g. `triggers` →
        /// list/arm/disarm/….
        positional: Vec<String>,
    }

    pub fn generate() {
        let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
        let path = Path::new(&manifest).join("completions").join("_ztmux");
        println!("cargo::rerun-if-changed=completions/_ztmux");

        // A source tree without the completion file (it is packaged, but be
        // forgiving) degrades to empty tables rather than failing the build.
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let verbs = parse(&text);

        let out = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("completion_spec.rs");
        std::fs::write(out, render(&verbs)).expect("write completion_spec.rs");
    }

    /// Split the completion file into `_ztmux-<verb>()` blocks and harvest each.
    /// Specs are only read from the ztmux-extension section (after the marker
    /// comment): the upstream tmux blocks use the full `_arguments` grammar
    /// (exclusion groups, `_alternative` payloads) whose flags the REPL takes
    /// from `CMD_TABLE` instead. Descriptions are harvested from every block.
    fn parse(text: &str) -> Vec<Verb> {
        const MARKER: &str = "# ── ztmux client extensions";

        let mut verbs: Vec<Verb> = Vec::new();
        let mut extensions = false;
        let mut current: Option<Verb> = None;

        for line in text.lines() {
            if line.starts_with(MARKER) {
                extensions = true;
            }
            if let Some(rest) = line.strip_prefix("_ztmux-")
                && let Some(name) = rest.strip_suffix("() {")
            {
                current = Some(Verb {
                    name: name.to_string(),
                    ..Verb::default()
                });
                continue;
            }
            let Some(verb) = current.as_mut() else {
                continue;
            };
            if line == "}" {
                verbs.push(current.take().expect("checked above"));
                continue;
            }
            if verb.description.is_empty()
                && let Some(d) = description(line)
            {
                verb.description = d;
                continue;
            }
            if extensions {
                for spec in quoted(line) {
                    harvest_spec(verb, &spec);
                }
            }
        }

        verbs.sort_by(|a, b| a.name.cmp(&b.name));
        verbs.dedup_by(|a, b| a.name == b.name);
        verbs
    }

    /// The description a block prints for `$tmux_describe`:
    /// `[[ -n ${tmux_describe} ]] && print "…" && return`.
    fn description(line: &str) -> Option<String> {
        let after = line.split_once("print \"")?.1;
        let (text, _) = after.split_once('"')?;
        Some(text.to_string())
    }

    /// Every single- or double-quoted token on a line, unquoted. `_arguments`
    /// specs are always quoted, one per line, in the generated file.
    fn quoted(line: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = line;
        while let Some(open) = rest.find(['\'', '"']) {
            let quote = rest.as_bytes()[open] as char;
            let after = &rest[open + 1..];
            let Some(close) = after.find(quote) else {
                break;
            };
            out.push(after[..close].to_string());
            rest = &after[close + 1..];
        }
        out
    }

    /// Read one `_arguments` spec into `verb`.
    ///
    /// The forms the extension specs use are:
    /// `-o[description]:format:(json)`, `--json[description]`,
    /// `-t[target window]:target`, `:subcommand:(list arm …)`, `:query:`.
    /// Leading `*` (repeatable) and `(…)` exclusion groups are stripped first;
    /// a trailing `+`/`=` on the option name (takes a value) is dropped.
    fn harvest_spec(verb: &mut Verb, spec: &str) {
        let spec = spec.trim_start_matches('*');
        let spec = match spec.strip_prefix('(') {
            Some(rest) => rest.split_once(')').map_or(rest, |(_, tail)| tail),
            None => spec,
        };
        let values = literal_values(spec);

        if let Some(body) = spec.strip_prefix('-') {
            let end = body.find(['[', ':', '+', '=']).unwrap_or(body.len());
            let name = format!("-{}", &body[..end]);
            if name == "-" || verb.options.contains(&name) {
                return;
            }
            verb.options.push(name.clone());
            if !values.is_empty() {
                verb.option_values.push((name, values));
            }
        } else if spec.starts_with(':') && verb.positional.is_empty() {
            verb.positional = values;
        }
    }

    /// The literal vocabulary of a spec: the words inside its final `(…)`.
    /// Returns nothing for action payloads (`:pane:__ztmux-panes`) and for the
    /// nested `((k\:desc …))` form, which lists no plain literals.
    fn literal_values(spec: &str) -> Vec<String> {
        let Some(open) = spec.rfind(":(") else {
            return Vec::new();
        };
        let inner = &spec[open + 2..];
        let Some(close) = inner.find(')') else {
            return Vec::new();
        };
        inner[..close]
            .split_whitespace()
            .filter(|w| !w.contains('\\') && !w.contains('('))
            .map(str::to_string)
            .collect()
    }

    /// Emit the two tables, sorted by verb so the REPL can binary-search them.
    fn render(verbs: &[Verb]) -> String {
        let mut out = String::from(
            "// @generated by build.rs from completions/_ztmux — do not edit.\n\n\
             /// One-line description of every `ztmux` verb, sorted by name.\n\
             pub(crate) static VERB_DESCRIPTIONS: &[(&str, &str)] = &[\n",
        );
        for v in verbs {
            if v.description.is_empty() {
                continue;
            }
            let _ = writeln!(
                out,
                "    ({:?}, {:?}),",
                v.name.as_str(),
                v.description.as_str()
            );
        }
        out.push_str(
            "];\n\n\
             /// `(verb, options, per-option values, positional vocabulary)`.\n\
             pub(crate) type ExtensionSpec = (\n    \
                 &'static str,\n    \
                 &'static [&'static str],\n    \
                 &'static [(&'static str, &'static [&'static str])],\n    \
                 &'static [&'static str],\n\
             );\n\n\
             /// Completion vocabulary of the ztmux extension verbs, sorted by verb.\n\
             pub(crate) static EXTENSION_SPEC: &[ExtensionSpec] = &[\n",
        );
        for v in verbs {
            if v.options.is_empty() && v.positional.is_empty() {
                continue;
            }
            let _ = write!(out, "    ({:?}, &[", v.name.as_str());
            for o in &v.options {
                let _ = write!(out, "{:?}, ", o.as_str());
            }
            out.push_str("], &[");
            for (name, values) in &v.option_values {
                let _ = write!(out, "({:?}, &[", name.as_str());
                for value in values {
                    let _ = write!(out, "{:?}, ", value.as_str());
                }
                out.push_str("]), ");
            }
            out.push_str("], &[");
            for p in &v.positional {
                let _ = write!(out, "{:?}, ", p.as_str());
            }
            out.push_str("]),\n");
        }
        out.push_str("];\n");
        out
    }
}
