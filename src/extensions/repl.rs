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
//! shows the keys, `clear` clears the screen, `exit`/`quit` (or Ctrl-D) leave.
//! Ctrl-C abandons the current line. History persists to `~/.ztmux/repl_history`.
//! Emacs keys by default, vi keys when `$VISUAL`/`$EDITOR` names a vi editor
//! (the same test tmux uses to pick its own mode keys). Piped stdin falls back
//! to a plain line reader — no banner, no completion — so `echo list-sessions |
//! ztmux repl` stays scriptable.

use std::borrow::Cow;
use std::io::{BufRead, IsTerminal, Write};

use reedline::{
    ColumnarMenu, Completer, EditMode, Emacs, FileBackedHistory, KeyCode, KeyModifiers, Keybindings,
    MenuBuilder, Prompt, PromptEditMode, PromptHistorySearch, PromptHistorySearchStatus,
    PromptViMode, Reedline, ReedlineEvent, ReedlineMenu, Signal, Span, Suggestion, Vi,
    default_emacs_keybindings, default_vi_insert_keybindings, default_vi_normal_keybindings,
};

use crate::cmd_::{CMD_TABLE, cmd_find};
use crate::tmux::getversion;

use super::completion_spec::EXTENSION_SPEC;
use super::tmux_query::{Snapshot, poll, ztmux_cmd};
use super::verbs::{Verb, all_verbs, colored, paint, print_verbs, strip_ansi};

/// The console's own commands, which never reach the server. `verbs` is also a
/// real subcommand (`ztmux verbs`); in here it runs in-process, so the listing
/// costs nothing.
pub(crate) const BUILTINS: &[(&str, &str)] = &[
    ("clear", "clear the screen (console builtin)"),
    ("exit", "leave the console (console builtin)"),
    ("help", "show the console keys and builtins (console builtin)"),
    ("quit", "leave the console (console builtin)"),
    ("verbs", "list every verb, optionally filtered (console builtin)"),
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
        .map(|i| (EXTENSION_SPEC[i].1, EXTENSION_SPEC[i].2, EXTENSION_SPEC[i].3))
}

/// Every flag `verb` accepts. Ported commands hand back their own
/// `args_parse` template (`"af:t:"` → `-a -f -t`), which is the exact string
/// the parser validates against; extension verbs fall back to the harvested
/// zsh completion. Unknown verbs offer nothing.
fn options(verb: &str) -> Vec<String> {
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
            .map_or_else(Vec::new, |(_, v)| v.iter().map(|s| (*s).to_string()).collect());
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
fn run_one(socket: &str, line: &str) -> bool {
    let words = split_words(line);
    let Some((verb, rest)) = words.split_first() else {
        return true;
    };

    match verb.as_str() {
        "exit" | "quit" => return false,
        "help" => {
            print_help();
            return true;
        }
        "verbs" => {
            print_verbs(rest.first().map(String::as_str));
            return true;
        }
        "clear" => {
            print!("\x1b[H\x1b[2J");
            let _ = std::io::stdout().flush();
            return true;
        }
        _ => {}
    }

    let args: Vec<&str> = words.iter().map(String::as_str).collect();
    match ztmux_cmd(socket, &args).status() {
        Ok(_) => {}
        Err(e) => eprintln!("ztmux: repl: {verb}: {e}"),
    }
    true
}

/// Non-tty stdin: a plain line reader, since reedline needs a terminal.
fn run_piped(socket: &str) -> i32 {
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if !run_one(socket, &line) {
            break;
        }
    }
    0
}

/// The startup banner: the ZTMUX logo, a boxed summary line (version and verb
/// totals), and the live server counts for the socket the console targets.
fn print_banner(socket: &str) {
    let color = colored();
    let logo = super::help::LOGO;
    if color {
        print!("{logo}");
    } else {
        print!("{}", strip_ansi(logo));
    }

    let commands = CMD_TABLE.len();
    let extensions = super::EXTENSION_COMMANDS.len();
    let summary = format!(
        " REPL // v{} // {commands} commands // {extensions} extensions ",
        getversion()
    );
    for line in box_lines(&summary, color) {
        println!("{line}");
    }

    let snap = poll(socket);
    println!("{}", server_line(socket, &snap, color));
    println!(
        "{}",
        paint(
            " Tab completes verbs and options — `verbs` lists them, `help` for keys, Ctrl-D to exit",
            "2",
            color,
        )
    );
    println!();
}

/// One-line server summary under the banner: the socket and its live counts,
/// or the reason the counts are missing.
fn server_line(socket: &str, snap: &Snapshot, color: bool) -> String {
    let socket = if socket.is_empty() { "default" } else { socket };
    let head = paint(&format!(" socket {socket}"), "2", color);
    match &snap.error {
        Some(_) => format!("{head}  {}", paint("// no server running", "31", color)),
        None => format!(
            "{head}  {}  {}  {}  {}",
            count(snap.sessions.len(), "session"),
            count(snap.windows.len(), "window"),
            count(snap.panes.len(), "pane"),
            count(snap.clients.len(), "client"),
        ),
    }
}

/// `1 session` / `2 sessions` — every count in the banner is plural-correct.
fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("{n} {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// The three lines of a single-line box around `line`, sized to its *printed*
/// width (colour escapes excluded), so the borders stay aligned however long
/// the version or the verb counts get.
fn box_lines(line: &str, color: bool) -> [String; 3] {
    let rule: String = "─".repeat(strip_ansi(line).chars().count());
    let border = |s: &str| paint(s, "36", color);
    [
        border(&format!(" ┌{rule}┐")),
        format!("{}{line}{}", border(" │"), border("│")),
        border(&format!(" └{rule}┘")),
    ]
}

/// `help` — the console's own keys and builtins.
fn print_help() {
    let color = colored();
    // Pad before colouring: SGR escapes have no printed width, so padding a
    // coloured string would leave the second column ragged.
    let key = |s: &str| paint(&format!("{s:<8}"), "33", color);

    println!("{}", paint("Every line runs as `ztmux <line>`.", "1", color));
    println!();
    for &(name, description) in BUILTINS {
        println!("  {} {description}", key(name));
    }
    println!();
    println!("  {} complete the verb, option or value", key("Tab"));
    println!("  {} abandon the current line", key("Ctrl-C"));
    println!("  {} leave the console", key("Ctrl-D"));
    println!("  {} search history", key("Ctrl-R"));
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
    keybindings.add_binding(KeyModifiers::SHIFT, KeyCode::BackTab, ReedlineEvent::MenuPrevious);
    keybindings.add_binding(KeyModifiers::NONE, KeyCode::BackTab, ReedlineEvent::MenuPrevious);
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

fn run_interactive(socket: &str) -> i32 {
    print_banner(socket);

    let completer = Box::new(ReplCompleter {
        verbs: all_verbs(),
    });
    let menu = ColumnarMenu::default()
        .with_name("completion_menu")
        .with_columns(4)
        .with_column_padding(2);

    let edit_mode: Box<dyn EditMode> = if vi_mode() {
        let mut insert = default_vi_insert_keybindings();
        install_menu_bindings(&mut insert);
        Box::new(Vi::new(insert, default_vi_normal_keybindings()))
    } else {
        let mut emacs = default_emacs_keybindings();
        install_menu_bindings(&mut emacs);
        Box::new(Emacs::new(emacs))
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
                if !run_one(socket, &line) {
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

    fn values(line: &str) -> Vec<String> {
        let mut completer = ReplCompleter {
            verbs: all_verbs(),
        };
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
        assert_eq!(split_words("  new-window  -n build "), ["new-window", "-n", "build"]);
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
    fn banner_box_borders_match_the_content_width() {
        // The box is measured on printed width, so neither a longer version
        // string nor the colour escapes can push the right border out of line.
        for content in [
            " REPL // v3.7.32 // 91 commands // 111 extensions ",
            " REPL // v10.100.1000 // 1 command // 1 extension ",
            "",
        ] {
            for color in [false, true] {
                let widths: Vec<usize> = box_lines(content, color)
                    .iter()
                    .map(|l| strip_ansi(l).chars().count())
                    .collect();
                assert_eq!(
                    widths[0], widths[1],
                    "top border and content disagree for {content:?} (color: {color})"
                );
                assert_eq!(
                    widths[1], widths[2],
                    "content and bottom border disagree for {content:?} (color: {color})"
                );
            }
        }
        assert_eq!(strip_ansi("\x1b[36m├─┤\x1b[0m").chars().count(), 3);
    }

    #[test]
    fn server_line_reports_a_dead_server_instead_of_zero_counts() {
        let snap = Snapshot {
            error: Some("no server running".into()),
            ..Default::default()
        };
        assert!(server_line("/tmp/sock", &snap, false).contains("no server running"));
        let mut live = Snapshot::default();
        assert!(server_line("", &live, false).contains("0 sessions"));
        live.sessions.push(super::super::tmux_query::Session::default());
        assert!(server_line("", &live, false).contains("1 session  "));
    }
}
