//! `ztmux repl` — an interactive console over every ztmux verb.
//!
//! tmux only offers the in-client command prompt (`prefix + :`): one line, no
//! history file, and completion limited to command names. This is the other
//! end of the same idea — a standalone [`reedline`] console that runs every
//! line exactly as `ztmux <line>` would, against the same socket, so the whole
//! command surface (ported tmux commands *and* the ztmux extensions) is
//! reachable with real line editing.
//!
//! On a terminal it opens with a banner (the ZTMUX logo, live session/window/
//! pane counts, verb totals) and edits the line with Tab completion of:
//!   * the command word — every `cmd_entry` name and alias, every extension
//!     verb, and the console builtins;
//!   * any `-`-prefixed word — the verb's flags, taken straight off its
//!     `args_parse` template (ported commands) or off the shipped zsh
//!     completion (extensions), so neither can drift from the parser;
//!   * an option's value where the vocabulary is fixed (`-o <Tab>` →
//!     json/jsonl/csv/…, `graph -o <Tab>` → dot/mermaid/html);
//!   * an extension's first positional where the vocabulary is fixed
//!     (`triggers <Tab>` → list/arm/disarm/…).
//!
//! Builtins: `verbs [filter]` lists every verb with its description, `help`
//! shows the keys, `banner` redraws the opening banner, `clear` clears the
//! screen, `exit`/`quit` (or Ctrl-D) leave. On top of those come the shell
//! builtins ([`super::shell`]) — `cd`, `pwd`, `dir`, `cat`, `echo`, `export`,
//! `printenv`, `unset`, `mkdir`, `touch`, `rm`, `cp`, `mv`, `ln` — which run in
//! this process, so the directory and environment they change are inherited by
//! every line spawned afterwards. Their path arguments complete against the
//! filesystem.
//! Ctrl-C abandons the current line. History persists to `~/.ztmux/repl_history`.
//!
//! The line editor is keyed emacs or vi, resolved at startup by the first of
//! these that names a mode (see [`repl_keys`]):
//!   1. `$ZTMUX_REPL_EDIT_MODE` — `vi` or `emacs`;
//!   2. `@ztmux-repl-edit-mode` on the server (`set -g @ztmux-repl-edit-mode
//!      vi`), the console's own option;
//!   3. `status-keys` on the server, the option tmux keys its own command
//!      prompt with, so the console follows the same config;
//!   4. `$VISUAL`/`$EDITOR` naming a vi editor — the test tmux itself applies
//!      when it picks `status-keys` at startup.
//!
//! Piped stdin falls back to a plain line reader — no banner, no completion —
//! so `echo list-sessions | ztmux repl` stays scriptable.

use std::borrow::Cow;
use std::io::{BufRead, IsTerminal, Write};

use reedline::{
    ColumnarMenu, Completer, EditMode, Emacs, FileBackedHistory, KeyCode, KeyModifiers,
    Keybindings, MenuBuilder, Prompt, PromptEditMode, PromptHistorySearch,
    PromptHistorySearchStatus, PromptViMode, Reedline, ReedlineEvent, ReedlineMenu, Signal, Span,
    Suggestion, Vi, default_emacs_keybindings, default_vi_insert_keybindings,
    default_vi_normal_keybindings,
};

use super::completion_spec::EXTENSION_SPEC;
use super::shell::{self, Paths};
use super::tmux_query::{query_lines, ztmux_cmd};
use super::verbs::{Verb, all_verbs, colored, paint, print_verbs};
use crate::cmd_::cmd_find;

/// The console's own commands, which never reach the server. `verbs` and
/// `banner` are real subcommands too (`ztmux verbs`, `ztmux banner`); in here
/// they run in-process, so neither costs a spawn.
pub(crate) const BUILTINS: &[(&str, &str)] = &[
    ("banner", "redraw the opening banner (console builtin)"),
    ("clear", "clear the screen (console builtin)"),
    ("exit", "leave the console (console builtin)"),
    (
        "help",
        "show the console keys and builtins (console builtin)",
    ),
    ("quit", "leave the console (console builtin)"),
    (
        "verbs",
        "list every verb, optionally filtered (console builtin)",
    ),
];

pub(crate) fn run(socket: &str) -> i32 {
    // Both ends must be a terminal: reedline reads keys from stdin and draws
    // the prompt and menu on stdout, so a redirected *either* way (a pipeline,
    // `ztmux repl | less`) takes the plain reader instead.
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        run_interactive(socket)
    } else {
        run_piped(socket)
    }
}

/// What an extension verb completes: `(options, option values, positional
/// vocabulary)` — a [`super::completion_spec::ExtensionSpec`] without the verb
/// it is keyed by.
type Vocabulary = (
    &'static [&'static str],
    &'static [(&'static str, &'static [&'static str])],
    &'static [&'static str],
);

/// The harvested vocabulary for an extension verb; `None` for anything else.
fn extension_spec(verb: &str) -> Option<Vocabulary> {
    EXTENSION_SPEC
        .binary_search_by(|(v, _, _, _)| (*v).cmp(verb))
        .ok()
        .map(|i| {
            (
                EXTENSION_SPEC[i].1,
                EXTENSION_SPEC[i].2,
                EXTENSION_SPEC[i].3,
            )
        })
}

/// Every flag `verb` accepts. Shell builtins hand back their own flag list,
/// ported commands their `args_parse` template (`"af:t:"` → `-a -f -t`), which
/// is the exact string the parser validates against; extension verbs fall back
/// to the harvested zsh completion. Unknown verbs offer nothing.
fn options(verb: &str) -> Vec<String> {
    if shell::lookup(verb).is_some() {
        return shell::flags(verb)
            .iter()
            .map(|o| (*o).to_string())
            .collect();
    }
    if let Some((opts, _, _)) = extension_spec(verb) {
        return opts.iter().map(|o| (*o).to_string()).collect();
    }
    let Ok(entry) = cmd_find(verb) else {
        return Vec::new();
    };
    entry
        .args
        .template
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| format!("-{c}"))
        .collect()
}

/// The fixed value vocabulary of `verb`'s `option`, if it has one: harvested
/// from the zsh completion for extensions, and read off the usage string for
/// ported commands (`"[-o json|jsonl|csv]"` → json/jsonl/csv).
fn option_values(verb: &str, option: &str) -> Vec<String> {
    if let Some((_, values, _)) = extension_spec(verb) {
        return values
            .iter()
            .find(|(name, _)| *name == option)
            .map_or_else(Vec::new, |(_, v)| {
                v.iter().map(|s| (*s).to_string()).collect()
            });
    }
    let Ok(entry) = cmd_find(verb) else {
        return Vec::new();
    };
    usage_values(entry.usage, option)
}

/// Pull `option`'s alternatives out of a usage string: the `[-o a|b|c]` form
/// tmux uses for a flag with a fixed set of names. Anything else (`[-t
/// target-pane]`, `[-a]`) has no vocabulary to offer.
fn usage_values(usage: &str, option: &str) -> Vec<String> {
    for group in usage.split('[') {
        let Some(group) = group.split(']').next() else {
            continue;
        };
        let mut words = group.split_whitespace();
        if words.next() != Some(option) {
            continue;
        }
        let Some(alternatives) = words.next() else {
            continue;
        };
        if !alternatives.contains('|') {
            continue;
        }
        return alternatives.split('|').map(str::to_string).collect();
    }
    Vec::new()
}

/// Split a console line into words the way a shell would: single quotes are
/// literal, double quotes keep `\"` and `\\` escapes, and a backslash outside
/// quotes escapes the next character. An unterminated quote simply runs to the
/// end of the line — the command then fails on the server's own parse error
/// rather than here.
fn split_words(line: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut chars = line.chars();

    while let Some(next) = chars.next() {
        match next {
            ws if ws.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            '\'' => {
                started = true;
                for quoted in chars.by_ref() {
                    if quoted == '\'' {
                        break;
                    }
                    word.push(quoted);
                }
            }
            '"' => {
                started = true;
                while let Some(quoted) = chars.next() {
                    match quoted {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some(escaped @ ('"' | '\\')) => word.push(escaped),
                            Some(other) => {
                                word.push('\\');
                                word.push(other);
                            }
                            None => word.push('\\'),
                        },
                        plain => word.push(plain),
                    }
                }
            }
            '\\' => {
                started = true;
                match chars.next() {
                    Some(escaped) => word.push(escaped),
                    None => word.push('\\'),
                }
            }
            plain => {
                started = true;
                word.push(plain);
            }
        }
    }
    if started {
        words.push(word);
    }
    words
}

/// Run one console line. Returns `false` to leave the console. Server-bound
/// lines re-invoke this binary against the same socket, so they are parsed and
/// dispatched by exactly the code path `ztmux <line>` uses.
fn run_one(socket: &str, line: &str, mode: Option<KeyMode>) -> bool {
    let words = split_words(line);
    let Some((verb, rest)) = words.split_first() else {
        return true;
    };

    match verb.as_str() {
        "exit" | "quit" => return false,
        "help" => {
            print_help(mode);
            return true;
        }
        "verbs" => {
            print_verbs(rest.first().map(String::as_str));
            return true;
        }
        "banner" => {
            show_banner(socket);
            return true;
        }
        "clear" => {
            print!("\x1b[H\x1b[2J");
            let _ = std::io::stdout().flush();
            return true;
        }
        _ => {}
    }

    // A shell builtin runs here rather than as a spawned line: `cd` and
    // `export` would otherwise move a child's directory and environment and
    // then exit, changing nothing the console can see.
    if let Some(builtin) = shell::lookup(verb) {
        if let Err(e) = builtin(rest) {
            eprintln!("ztmux: repl: {verb}: {e}");
        }
        return true;
    }

    let args: Vec<&str> = words.iter().map(String::as_str).collect();
    match ztmux_cmd(socket, &args).status() {
        Ok(_) => {}
        Err(e) => eprintln!("ztmux: repl: {verb}: {e}"),
    }
    true
}

/// Non-tty stdin: a plain line reader, since reedline needs a terminal. There
/// is no line editor here, so `help` reports no key mode.
fn run_piped(socket: &str) -> i32 {
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if !run_one(socket, &line, None) {
            break;
        }
    }
    0
}

/// The console's opening screen: the banner ([`super::banner`], also the
/// `ztmux banner` verb) and the one-line key hint under it. Printed at startup
/// and again by the `banner` builtin, once output has scrolled it away.
fn show_banner(socket: &str) {
    super::banner::print_banner(socket);
    println!(
        "{}",
        paint(
            " Tab completes verbs and options — `verbs` lists them, `help` for keys, Ctrl-D to exit",
            "2",
            colored(),
        )
    );
    println!();
}

/// `help` — the console's own keys and builtins.
fn print_help(mode: Option<KeyMode>) {
    let color = colored();
    // Pad before colouring: SGR escapes have no printed width, so padding a
    // coloured string would leave the second column ragged.
    let key = |s: &str| paint(&format!("{s:<8}"), "33", color);

    println!(
        "{}",
        paint("Every line runs as `ztmux <line>`.", "1", color)
    );
    println!();
    for &(name, description) in BUILTINS {
        println!("  {} {description}", key(name));
    }
    println!();
    println!(
        "{}",
        paint(
            "These run in the console itself, so the directory and environment they set are inherited by every later line:",
            "2",
            color
        )
    );
    for (name, description) in shell::described() {
        println!("  {} {description}", key(name));
    }
    println!();
    println!("  {} complete the verb, option or value", key("Tab"));
    println!("  {} abandon the current line", key("Ctrl-C"));
    println!("  {} leave the console", key("Ctrl-D"));
    println!("  {} search history", key("Ctrl-R"));
    if let Some(mode) = mode {
        if mode.keys == ReplKeys::Vi {
            println!(
                "  {} leave insert mode (the prompt becomes `:`)",
                key("Esc")
            );
        }
        println!();
        for line in key_mode_lines(mode) {
            println!("  {} {line}", key(""));
        }
    }
}

/// The `help` lines naming the key set in use, where it came from, and how to
/// change it — the resolution order is invisible otherwise.
fn key_mode_lines(mode: KeyMode) -> [String; 2] {
    let name = match mode.keys {
        ReplKeys::Vi => "vi",
        ReplKeys::Emacs => "emacs",
    };
    let other = match mode.keys {
        ReplKeys::Vi => "emacs",
        ReplKeys::Emacs => "vi",
    };
    [
        format!("{name} keys, from {}", mode.source),
        format!("`set -g {REPL_KEYS_OPT} {other}` or {REPL_KEYS_ENV}={other} to change"),
    ]
}

/// Byte index of the word under the cursor and that word's text.
fn completion_word_start(line: &str, pos: usize) -> (usize, &str) {
    let pos = pos.min(line.len());
    let before = line.get(..pos).unwrap_or("");
    let start = before
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map_or(0, |(i, c)| i + c.len_utf8());
    (start, line.get(start..pos).unwrap_or(""))
}

/// Tab completion: the command word against every verb, a `-`-prefixed word
/// against that verb's flags, the word after a value-taking option against
/// that option's vocabulary, and an extension's first argument against its
/// subcommand list.
struct ReplCompleter {
    verbs: Vec<Verb>,
}

/// Prefix-filtered, sorted suggestions replacing `span`.
fn suggestions<'a>(
    candidates: impl Iterator<Item = (&'a str, Option<String>)>,
    prefix: &str,
    span: Span,
) -> Vec<Suggestion> {
    let mut out: Vec<Suggestion> = candidates
        .filter(|(c, _)| c.starts_with(prefix))
        .map(|(c, description)| Suggestion {
            value: c.to_string(),
            description,
            style: None,
            extra: None,
            span,
            append_whitespace: true,
            display_override: None,
            match_indices: None,
        })
        .collect();
    out.sort_by(|a, b| a.value.cmp(&b.value));
    out
}

/// Suggestions over plain strings, with no description.
fn plain_suggestions(candidates: Vec<String>, prefix: &str, span: Span) -> Vec<Suggestion> {
    let owned: Vec<String> = candidates;
    suggestions(owned.iter().map(|c| (c.as_str(), None)), prefix, span)
}

/// Complete a shell builtin's path argument against the filesystem: the
/// entries of the directory the typed word names, directories only for `cd`.
/// The suggestion keeps whatever the word already said (`~/`, `../src/`) and
/// replaces only its last component, and a directory completes without a
/// trailing space so the next component can be typed straight on.
fn path_suggestions(prefix: &str, span: Span, paths: Paths) -> Vec<Suggestion> {
    // The word splits at its last `/`: everything up to and including it names
    // the directory to read, the rest is the prefix to match entries against.
    let (typed_dir, base) = match prefix.rfind('/') {
        Some(i) => prefix.split_at(i + 1),
        None => ("", prefix),
    };
    let dir = match typed_dir {
        "" => std::path::PathBuf::from("."),
        "~/" => match std::env::var_os("HOME") {
            Some(home) => std::path::PathBuf::from(home),
            None => return Vec::new(),
        },
        other => match other.strip_prefix("~/") {
            Some(rest) => match std::env::var_os("HOME") {
                Some(home) => std::path::PathBuf::from(home).join(rest),
                None => return Vec::new(),
            },
            None => std::path::PathBuf::from(other),
        },
    };

    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<Suggestion> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(base) {
            continue;
        }
        // Dotfiles stay out of the way until a `.` is typed, as a shell does.
        if !base.starts_with('.') && name.starts_with('.') {
            continue;
        }
        // Directory-ness follows symlinks: a symlink to a directory is one to
        // `cd` into, and gets the same trailing slash.
        let is_dir = entry.path().is_dir();
        if paths == Paths::Directories && !is_dir {
            continue;
        }
        let slash = if is_dir { "/" } else { "" };
        out.push(Suggestion {
            value: format!("{typed_dir}{name}{slash}"),
            description: None,
            style: None,
            extra: None,
            span,
            append_whitespace: !is_dir,
            display_override: None,
            match_indices: None,
        });
    }
    out.sort_by(|a, b| a.value.cmp(&b.value));
    out
}

impl Completer for ReplCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        let (start, prefix) = completion_word_start(line, pos);
        let span = Span::new(start, pos);
        let before = line.get(..start).unwrap_or("").trim();

        if before.is_empty() {
            return suggestions(
                self.verbs
                    .iter()
                    .map(|v| (v.name, Some(v.description.to_string()))),
                prefix,
                span,
            );
        }
        let words: Vec<&str> = before.split_whitespace().collect();
        let verb = words[0];

        if prefix.starts_with('-') {
            return plain_suggestions(options(verb), prefix, span);
        }
        // A shell builtin's arguments are paths — completed off the filesystem
        // rather than out of a fixed vocabulary.
        if let Some(paths) = shell::path_arguments(verb) {
            return path_suggestions(prefix, span, paths);
        }
        // The word after an option that takes a fixed set of values.
        if let Some(previous) = words.last().filter(|w| w.starts_with('-')) {
            let values = option_values(verb, previous);
            if !values.is_empty() {
                return plain_suggestions(values, prefix, span);
            }
        }
        // An extension verb's first argument, where it names a subcommand.
        if words.len() == 1
            && let Some((_, _, positional)) = extension_spec(verb)
        {
            return suggestions(positional.iter().map(|p| (*p, None)), prefix, span);
        }
        Vec::new()
    }
}

/// Bind Tab to open / advance the completion menu and Shift-Tab to step back,
/// on whichever insert keymap is in use.
fn install_menu_bindings(keybindings: &mut Keybindings) {
    keybindings.add_binding(
        KeyModifiers::NONE,
        KeyCode::Tab,
        ReedlineEvent::UntilFound(vec![
            ReedlineEvent::Menu("completion_menu".to_string()),
            ReedlineEvent::MenuNext,
        ]),
    );
    keybindings.add_binding(
        KeyModifiers::SHIFT,
        KeyCode::BackTab,
        ReedlineEvent::MenuPrevious,
    );
    keybindings.add_binding(
        KeyModifiers::NONE,
        KeyCode::BackTab,
        ReedlineEvent::MenuPrevious,
    );
}

/// Vi keys when `$VISUAL`/`$EDITOR` names a vi editor — the same test tmux
/// applies when it picks `status-keys`/`mode-keys` at startup.
fn vi_mode() -> bool {
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_default();
    let editor = editor.rsplit('/').next().unwrap_or(&editor);
    editor.contains("vi")
}

/// The console's own edit-mode option, read off the server so it can be set
/// from `.tmux.conf` like every other ztmux option.
const REPL_KEYS_OPT: &str = "@ztmux-repl-edit-mode";

/// Overrides both options, for a one-off console without touching the server.
const REPL_KEYS_ENV: &str = "ZTMUX_REPL_EDIT_MODE";

/// Which key set the line editor uses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ReplKeys {
    Emacs,
    Vi,
}

/// A resolved key set and the config that named it, so `help` can say where
/// the mode came from instead of leaving the user to guess.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct KeyMode {
    keys: ReplKeys,
    source: &'static str,
}

/// A configured value naming a key set. `vi`/`vim`/`nvim` and `emacs` are
/// accepted in any case; anything else (including the empty string an unset
/// `show-options -q` returns) names no mode and defers to the next source.
fn parse_keys(value: &str) -> Option<ReplKeys> {
    match value.trim().to_ascii_lowercase().as_str() {
        "vi" | "vim" | "nvim" => Some(ReplKeys::Vi),
        "emacs" => Some(ReplKeys::Emacs),
        _ => None,
    }
}

/// Pick the key set from the first source that names one: `$ZTMUX_REPL_EDIT_MODE`,
/// `@ztmux-repl-edit-mode`, `status-keys`, then the `$VISUAL`/`$EDITOR` test.
fn repl_keys(socket: &str) -> KeyMode {
    let option = |name: &str| {
        query_lines(socket, &["show-options", "-gqv", name])
            .into_iter()
            .next()
    };
    if let Some(keys) = std::env::var(REPL_KEYS_ENV)
        .ok()
        .as_deref()
        .and_then(parse_keys)
    {
        return KeyMode {
            keys,
            source: REPL_KEYS_ENV,
        };
    }
    // Each option is only asked for if the one before it named no mode, so a
    // console started against a dead server costs at most the two queries.
    for source in [REPL_KEYS_OPT, "status-keys"] {
        if let Some(keys) = option(source).as_deref().and_then(parse_keys) {
            return KeyMode { keys, source };
        }
    }
    KeyMode {
        keys: if vi_mode() {
            ReplKeys::Vi
        } else {
            ReplKeys::Emacs
        },
        source: "$VISUAL/$EDITOR",
    }
}

fn run_interactive(socket: &str) -> i32 {
    show_banner(socket);

    let completer = Box::new(ReplCompleter { verbs: all_verbs() });
    let menu = ColumnarMenu::default()
        .with_name("completion_menu")
        .with_columns(4)
        .with_column_padding(2);

    let mode = repl_keys(socket);
    let edit_mode: Box<dyn EditMode> = match mode.keys {
        ReplKeys::Vi => {
            let mut insert = default_vi_insert_keybindings();
            install_menu_bindings(&mut insert);
            Box::new(Vi::new(insert, default_vi_normal_keybindings()))
        }
        ReplKeys::Emacs => {
            let mut emacs = default_emacs_keybindings();
            install_menu_bindings(&mut emacs);
            Box::new(Emacs::new(emacs))
        }
    };

    let mut editor = Reedline::create()
        .with_completer(completer)
        .with_menu(ReedlineMenu::EngineCompleter(Box::new(menu)))
        .with_edit_mode(edit_mode);

    // History is a convenience: a home directory that cannot be written falls
    // back to the in-memory history rather than refusing to start the console.
    if let Ok(history) =
        FileBackedHistory::with_file(1000, super::diagnostics::dir().join("repl_history"))
    {
        editor = editor.with_history(Box::new(history));
    }

    let prompt = ReplPrompt;
    loop {
        match editor.read_line(&prompt) {
            Ok(Signal::Success(line)) | Ok(Signal::HostCommand(line)) => {
                if !run_one(socket, &line, Some(mode)) {
                    break;
                }
            }
            // Ctrl-C, or an external break: drop the line, keep the console.
            Ok(Signal::CtrlC) | Ok(Signal::ExternalBreak(_)) => continue,
            Ok(Signal::CtrlD) => break,
            // `Signal` is #[non_exhaustive]; treat anything new as Ctrl-C.
            Ok(_) => continue,
            Err(e) => {
                eprintln!("ztmux: repl: {e}");
                return 1;
            }
        }
    }
    0
}

/// The `ztmux> ` prompt.
struct ReplPrompt;

impl Prompt for ReplPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed("ztmux")
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, mode: PromptEditMode) -> Cow<'_, str> {
        match mode {
            PromptEditMode::Vi(PromptViMode::Normal) => Cow::Borrowed(": "),
            _ => Cow::Borrowed("> "),
        }
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("... ")
    }

    fn render_prompt_history_search_indicator(&self, search: PromptHistorySearch) -> Cow<'_, str> {
        let failing = match search.status {
            PromptHistorySearchStatus::Passing => "",
            PromptHistorySearchStatus::Failing => "failing ",
        };
        Cow::Owned(format!("({failing}reverse-search: {}) ", search.term))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd_::CMD_TABLE;

    fn values(line: &str) -> Vec<String> {
        let mut completer = ReplCompleter { verbs: all_verbs() };
        completer
            .complete(line, line.len())
            .into_iter()
            .map(|s| s.value)
            .collect()
    }

    #[test]
    fn command_word_completes_commands_aliases_extensions_and_builtins() {
        assert!(values("kill-p").contains(&"kill-pane".to_string()));
        // Aliases are completable too — `killp` is what gets typed in practice.
        assert!(values("killp").contains(&"killp".to_string()));
        assert!(values("dash").contains(&"dashboard".to_string()));
        assert!(values("ver").contains(&"verbs".to_string()));
        // Every suggestion still starts with what was typed.
        assert!(values("list-").iter().all(|v| v.starts_with("list-")));
    }

    #[test]
    fn options_come_from_the_commands_own_args_template() {
        // cmd_kill_pane.rs: args_parse::new("af:t:", …) — no more, no less.
        assert_eq!(values("kill-pane -"), vec!["-a", "-f", "-t"]);
        // Sourced through cmd_find, so an alias resolves to the same flags.
        assert_eq!(values("killp -"), vec!["-a", "-f", "-t"]);
        // The template drives it even where the shipped zsh completion (which
        // predates ztmux's structured output) lists no -o/-O at all.
        let list_sessions = values("list-sessions -");
        for flag in ["-F", "-O", "-f", "-o", "-r"] {
            assert!(
                list_sessions.contains(&flag.to_string()),
                "list-sessions missing {flag}: {list_sessions:?}"
            );
        }
    }

    #[test]
    fn extension_options_and_subcommands_come_from_the_harvested_spec() {
        let prune = values("prune --");
        for flag in ["--dead", "--empty", "--force", "--idle", "--json"] {
            assert!(
                prune.contains(&flag.to_string()),
                "prune missing {flag}: {prune:?}"
            );
        }
        // A verb whose first argument is a fixed subcommand list.
        let triggers = values("triggers ");
        for sub in ["list", "arm", "disarm", "add", "wizard", "test"] {
            assert!(
                triggers.contains(&sub.to_string()),
                "triggers missing {sub}: {triggers:?}"
            );
        }
        assert_eq!(values("triggers di"), vec!["disarm"]);
    }

    #[test]
    fn option_values_complete_for_both_ported_and_extension_verbs() {
        // Ported: read off the usage string "[-o json|jsonl|csv|tsv|table|yaml]".
        assert_eq!(
            values("list-sessions -o "),
            vec!["csv", "json", "jsonl", "table", "tsv", "yaml"]
        );
        // Extension: harvested from the zsh completion's `:format:(…)`.
        assert_eq!(values("graph -o "), vec!["dot", "html", "mermaid"]);
        // A flag with no fixed vocabulary offers nothing.
        assert!(values("kill-pane -t ").is_empty());
    }

    #[test]
    fn usage_values_only_reads_alternation_groups() {
        assert_eq!(
            usage_values("[-o json|jsonl] [-t target-pane]", "-o"),
            vec!["json", "jsonl"]
        );
        assert!(usage_values("[-o json|jsonl] [-t target-pane]", "-t").is_empty());
        assert!(usage_values("[-a]", "-a").is_empty());
    }

    #[test]
    fn unknown_verbs_and_deep_positions_offer_nothing() {
        assert!(values("not-a-command -").is_empty());
        assert!(values("kill-pane -a ").is_empty());
        assert!(values("dashboard ").is_empty());
    }

    #[test]
    fn lines_split_like_a_shell() {
        assert_eq!(
            split_words("  new-window  -n build "),
            ["new-window", "-n", "build"]
        );
        assert_eq!(
            split_words("run-shell 'echo hello world'"),
            ["run-shell", "echo hello world"]
        );
        assert_eq!(
            split_words(r#"display-message "a \"quoted\" word""#),
            ["display-message", "a \"quoted\" word"]
        );
        assert_eq!(split_words(r"send-keys C-c\ x"), ["send-keys", "C-c x"]);
        // An empty quoted word survives as an empty argument.
        assert_eq!(split_words("rename-session ''"), ["rename-session", ""]);
        assert!(split_words("   ").is_empty());
    }

    #[test]
    fn edit_mode_values_parse_the_way_the_options_spell_them() {
        // What `show-options -gqv status-keys` prints for the CHOICE option,
        // and what `set -g @ztmux-repl-edit-mode …` is documented to take.
        assert_eq!(parse_keys("vi"), Some(ReplKeys::Vi));
        assert_eq!(parse_keys("emacs"), Some(ReplKeys::Emacs));
        // A `$EDITOR`-shaped value in the env override still names vi.
        assert_eq!(parse_keys("vim"), Some(ReplKeys::Vi));
        assert_eq!(parse_keys("nvim"), Some(ReplKeys::Vi));
        assert_eq!(parse_keys(" Vi\n"), Some(ReplKeys::Vi));
        // Unset (`-q` prints nothing) and junk must defer to the next source
        // rather than silently picking a mode.
        assert_eq!(parse_keys(""), None);
        assert_eq!(parse_keys("   "), None);
        assert_eq!(parse_keys("on"), None);
        assert_eq!(parse_keys("vi-insert"), None);
    }

    #[test]
    fn help_names_the_key_set_its_source_and_the_other_mode() {
        let vi = key_mode_lines(KeyMode {
            keys: ReplKeys::Vi,
            source: "status-keys",
        });
        assert_eq!(vi[0], "vi keys, from status-keys");
        // The suggested value is the mode not in use, so copying the line
        // actually changes something.
        assert!(vi[1].contains("@ztmux-repl-edit-mode emacs"), "{}", vi[1]);
        assert!(vi[1].contains("ZTMUX_REPL_EDIT_MODE=emacs"), "{}", vi[1]);

        let emacs = key_mode_lines(KeyMode {
            keys: ReplKeys::Emacs,
            source: "$VISUAL/$EDITOR",
        });
        assert_eq!(emacs[0], "emacs keys, from $VISUAL/$EDITOR");
        assert!(
            emacs[1].contains("@ztmux-repl-edit-mode vi"),
            "{}",
            emacs[1]
        );
    }

    #[test]
    fn no_console_builtin_shadows_a_command_alias_or_extension() {
        // Every line that is not a builtin is spawned as `ztmux <line>`, so a
        // builtin named after a real verb would make that verb unreachable
        // from the console. `ls` is why the listing is `dir`: it is already the
        // alias for list-sessions.
        for (name, _) in shell::described() {
            for entry in CMD_TABLE {
                assert_ne!(
                    name, entry.name,
                    "{name} shadows the command {}",
                    entry.name
                );
                assert_ne!(
                    Some(name),
                    entry.alias,
                    "{name} shadows an alias of {}",
                    entry.name
                );
            }
            assert!(
                !super::super::EXTENSION_COMMANDS.contains(&name),
                "{name} shadows the extension verb {name}"
            );
        }
        assert!(CMD_TABLE.iter().any(|e| e.alias == Some("ls")));
        assert!(shell::lookup("ls").is_none());
    }

    #[test]
    fn shell_builtins_complete_their_flags_and_their_paths() {
        // Flags come from the builtin's own list, not from any command table.
        assert_eq!(values("rm -"), vec!["-f", "-r"]);
        assert_eq!(values("dir -"), vec!["-a", "-l", "-r", "-t"]);
        // A path argument completes off the filesystem. `/` is the one
        // directory every platform this builds on has.
        let root = values("cd /");
        assert!(!root.is_empty(), "cd / completed nothing");
        assert!(
            root.iter().all(|v| v.starts_with('/') && v.ends_with('/')),
            "cd must offer directories only, each keeping the typed prefix: {root:?}"
        );
        // `cd` is directories-only; `cat` takes any entry, so at the same
        // prefix it can only offer more, never less.
        let any = values("cat /");
        assert!(
            any.len() >= root.len(),
            "cat / offered fewer entries than cd /: {any:?}"
        );
        // A prefix narrows to what starts with it, and a builtin that takes no
        // paths completes nothing.
        assert!(values("cd /de").iter().all(|v| v.starts_with("/de")));
        assert!(values("export ").is_empty());
    }
}
