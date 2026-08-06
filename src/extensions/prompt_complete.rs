//! Tab completion for the floating command box (`super::ratatui_ui`).
//!
//! Upstream's `prompt_complete_commands` only ever offers command names and
//! `command-alias` entries, because upstream completes inline on the status row
//! where a long list has nowhere to go. The floating palette has room, so it
//! completes the whole command line: the verb, its flags, and the *value* each
//! slot wants — targets, buffers, options and their values, layouts, key names,
//! paths. This is extension chrome, not a port: the ported prompt still
//! completes exactly what `prompt.c` completes.
//!
//! Every slot is derived from data the port already carries, never a hand-kept
//! list: which flags exist comes from the command's `args_parse` template (the
//! string the parser itself validates against), what each flag and positional
//! *means* comes from its usage string (`"[-b buffer-name] path"` → `-b` wants
//! a buffer, the first positional a path), and the values come from the live
//! server (sessions, panes, buffers, key tables) or the options table. A
//! command gains completion the moment it is ported, with nothing to update
//! here.

use std::ffi::CStr;
use std::ptr::{NonNull, null_mut};

use crate::compat::queue::tailq_foreach;
use crate::compat::tree::rb_foreach;
use crate::{
    OPTIONS_TABLE, OPTIONS_TABLE_IS_HOOK, OPTIONS_TABLE_PANE, OPTIONS_TABLE_SERVER,
    OPTIONS_TABLE_SESSION, OPTIONS_TABLE_WINDOW, cmd_entry, discr_entry, options_table_type,
};

/// The layouts `select-layout` names, from `layout-set.c`'s table.
const LAYOUTS: [&str; 7] = [
    "even-horizontal",
    "even-vertical",
    "main-horizontal",
    "main-horizontal-mirrored",
    "main-vertical",
    "main-vertical-mirrored",
    "tiled",
];

/// Every option scope, for the commands (and positions) that may name any
/// option.
const ANY_SCOPE: i32 =
    OPTIONS_TABLE_SERVER | OPTIONS_TABLE_SESSION | OPTIONS_TABLE_WINDOW | OPTIONS_TABLE_PANE;

/// Candidates for the word under the cursor, given the words of the command
/// being typed before it (`["set-window-option", "-g", "mode-keys"]`) and the
/// word itself (possibly empty — Tab on a fresh word offers the whole slot).
pub(crate) fn candidates(before: &[&str], word: &str) -> Vec<String> {
    let Some(&command) = before.first() else {
        // The first word: nothing to narrow the whole command table with until
        // something is typed.
        return if word.is_empty() {
            Vec::new()
        } else {
            finish(command_names(), word)
        };
    };

    // A `-` word is a flag of the command being typed, and nothing else can
    // match there.
    if word.starts_with('-') {
        return finish(super::repl::command_flags(command), word);
    }

    if let Ok(entry) = crate::cmd_::cmd_find(command)
        && let Some(slot) = slot(entry, before)
    {
        let list = finish(slot_candidates(&slot, entry.name, before, word), word);
        if !list.is_empty() {
            return list;
        }
    }

    // ztmux's own verbs are not in the command table; their vocabulary is the
    // one harvested from the shipped zsh completion, so the command box offers
    // exactly what `ztmux <verb> <Tab>` offers in the shell.
    if let Some((_, _, positional)) = super::repl::extension_spec(command) {
        if let Some(previous) = before.last().filter(|w| w.starts_with('-')) {
            let values = super::repl::option_values(command, previous);
            if !values.is_empty() {
                return finish(values, word);
            }
        }
        if before.len() == 1 && !positional.is_empty() {
            return finish(positional.iter().map(|p| (*p).to_string()).collect(), word);
        }
    }

    // Nothing typed, and the slot had no vocabulary (or the usage named none):
    // the flags are the one thing the command certainly accepts here.
    if word.is_empty() {
        return finish(super::repl::command_flags(command), word);
    }

    // Something typed with no slot to match it against - an unported verb, or
    // an argument past the ones the usage names. The broad vocabulary is what a
    // `bind-key`-style trailing command line wants anyway.
    let mut list = command_names();
    list.extend(scoped_options(ANY_SCOPE));
    list.extend(LAYOUTS.iter().map(|l| (*l).to_string()));
    finish(list, word)
}

/// Prefix-filter, sort and dedupe a candidate list — done once, here, so every
/// slot below can just collect its vocabulary in whatever order it comes.
fn finish(list: Vec<String>, word: &str) -> Vec<String> {
    let mut list: Vec<String> = list
        .into_iter()
        .filter(|candidate| candidate.starts_with(word))
        .collect();
    list.sort();
    list.dedup();
    list
}

/// Every command name and alias, plus ztmux's extension verbs.
fn command_names() -> Vec<String> {
    let mut list: Vec<String> = Vec::new();
    for cmdent in crate::CMD_TABLE {
        list.push(cmdent.name.to_string());
        if let Some(alias) = cmdent.alias {
            list.push(alias.to_string());
        }
    }
    list.extend(
        crate::extensions::EXTENSION_COMMANDS
            .iter()
            .map(|name| (*name).to_string()),
    );
    list
}

/// What the word under the cursor is: the placeholder name the command's usage
/// gives that position (`"target-pane"`, `"buffer-name"`, `"option"`).
type Slot = String;

/// The usage-derived meaning of the word under the cursor, or `None` when the
/// usage says nothing about that position.
fn slot(entry: &cmd_entry, before: &[&str]) -> Option<Slot> {
    let usage = parse_usage(entry.usage);
    let template = entry.args.template;

    let mut positional = 0usize;
    // The flag whose value is the *next* word (`-t` typed alone, its target
    // still to come).
    let mut pending: Option<char> = None;

    for word in &before[1..] {
        if pending.take().is_some() {
            continue; // this word is the pending flag's value
        }
        let Some(cluster) = word.strip_prefix('-').filter(|c| !c.is_empty()) else {
            positional += 1;
            continue;
        };
        // `-abct foo` — only the first value-taking flag of a cluster can have
        // one, and it swallows the rest of the word when there is any.
        for (i, ch) in cluster.char_indices() {
            if takes_value(template, ch) {
                if cluster[i + ch.len_utf8()..].is_empty() {
                    pending = Some(ch);
                }
                break;
            }
        }
    }

    if let Some(flag) = pending {
        return usage
            .flags
            .iter()
            .find(|(ch, _)| *ch == flag)
            .map(|(_, placeholder)| placeholder.clone());
    }
    match usage.positionals.get(positional) {
        Some(placeholder) => Some(placeholder.clone()),
        // A variadic tail (`path ...`, `key ...`) keeps meaning the same thing.
        None if usage.variadic => usage.positionals.last().cloned(),
        None => None,
    }
}

/// Whether `flag` takes a value, per the `args_parse` template (`"t:"` → yes).
fn takes_value(template: &str, flag: char) -> bool {
    let bytes = template.as_bytes();
    bytes
        .iter()
        .position(|ch| *ch as char == flag)
        .is_some_and(|i| bytes.get(i + 1) == Some(&b':'))
}

/// A command's usage string, split into what each value-taking flag wants and
/// what its positional arguments want.
#[derive(Default, Debug, PartialEq, Eq)]
struct Usage {
    flags: Vec<(char, String)>,
    positionals: Vec<String>,
    /// The last positional repeats (`path ...`).
    variadic: bool,
}

/// Read a usage string as slots: `"[-a] [-b buffer-name] path"` → `-b` wants a
/// `buffer-name`, the first positional a `path`. Bracket groups without a
/// space after the flag letters (`"[-aCg]"`) are boolean flags with nothing to
/// complete; a bare `-t target-pane` (a mandatory flag, as `send-keys` has) is
/// read the same way as the bracketed form.
fn parse_usage(usage: &str) -> Usage {
    // Left to right, because the order of the positionals is the whole point:
    // `option [value]` means the value comes second. Each unit is either a
    // bracket group (taken whole, nesting and all) or a bare word.
    let mut units: Vec<&str> = Vec::new();
    let mut rest = usage;
    while let Some(open) = rest.find('[') {
        units.extend(rest[..open].split_whitespace());
        let mut depth = 0usize;
        let mut close = rest.len() - 1;
        for (i, ch) in rest[open..].char_indices() {
            match ch {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        close = open + i;
                        break;
                    }
                }
                _ => {}
            }
        }
        units.push(&rest[open..=close]);
        rest = rest.get(close + 1..).unwrap_or("");
    }
    units.extend(rest.split_whitespace());

    let mut out = Usage::default();
    let mut i = 0;
    while i < units.len() {
        let unit = units[i];
        if let Some(group) = unit.strip_prefix('[').and_then(|g| g.strip_suffix(']')) {
            read_group(group, &mut out);
        } else if unit == "..." {
            out.variadic = true;
        } else if let Some(flag) = unit.strip_prefix('-').and_then(|f| f.chars().next()) {
            // A mandatory flag, as `send-keys` has: `-t target-pane key ...`.
            if let Some(placeholder) = units
                .get(i + 1)
                .filter(|next| !next.starts_with(['-', '[']))
            {
                out.flags.push((flag, (*placeholder).to_string()));
                i += 1;
            }
        } else {
            out.positionals.push(unit.to_string());
        }
        i += 1;
    }
    out
}

/// One bracket group of a usage string.
fn read_group(group: &str, out: &mut Usage) {
    let Some(after_dash) = group.strip_prefix('-') else {
        // An optional positional: `[layout-name]`, `[shell-command [argument
        // ...]]` — the nested part is the variadic tail of the same slot.
        let mut words = group.split_whitespace();
        if let Some(first) = words.next() {
            out.positionals.push(first.trim_matches('[').to_string());
        }
        if group.contains("...") {
            out.variadic = true;
        }
        return;
    };
    // `[-b buffer-name]` is one flag with a value; `[-aCg]` is a cluster of
    // boolean flags, with nothing to complete after them.
    if let Some((flag, placeholder)) = after_dash.split_once(char::is_whitespace)
        && let Some(ch) = flag.chars().next()
    {
        out.flags.push((ch, placeholder.trim().to_string()));
    }
}

/// The vocabulary a usage placeholder names. Unknown placeholders (formats,
/// styles, free text) complete to nothing rather than to noise.
fn slot_candidates(slot: &str, command: &str, before: &[&str], word: &str) -> Vec<String> {
    // `[-o json|jsonl|csv]` spells its own values out.
    if slot.contains('|') {
        return slot.split('|').map(str::to_string).collect();
    }
    match slot {
        "target-pane" | "src-pane" | "dst-pane" | "target" => panes(),
        "target-window" | "src-window" | "dst-window" => windows(),
        "target-session" | "src-session" | "dst-session" => sessions(),
        "target-client" => clients(),
        "buffer-name" | "new-buffer-name" => buffers(),
        "path" => paths(word, super::shell::Paths::Any),
        "start-directory" | "working-directory" | "directory" => {
            paths(word, super::shell::Paths::Directories)
        }
        "option" => scoped_options(option_scope(command).unwrap_or(ANY_SCOPE)),
        "value" => option_values(before),
        "layout-name" => LAYOUTS.iter().map(|l| (*l).to_string()).collect(),
        "key" => keys(before),
        "key-table" => key_tables(),
        "hook" => hooks(),
        "command" | "shell-command" | "template" => command_names(),
        "variable" => variables(),
        _ => Vec::new(),
    }
}

/// The option scope `command` (a canonical command name) names options in, as
/// an `OPTIONS_TABLE_*` mask; `None` for commands that name any option.
fn option_scope(command: &str) -> Option<i32> {
    match command {
        "set-window-option" | "show-window-options" => {
            Some(OPTIONS_TABLE_WINDOW | OPTIONS_TABLE_PANE)
        }
        _ => None,
    }
}

/// Option names in `scope`, hooks excluded (they have their own commands and
/// would bury the ordinary options).
fn scoped_options(scope: i32) -> Vec<String> {
    OPTIONS_TABLE
        .iter()
        .filter(|oe| oe.scope & scope != 0 && oe.flags & OPTIONS_TABLE_IS_HOOK == 0)
        .map(|oe| oe.name.to_string())
        .collect()
}

/// Hook names, for `set-hook` / `show-hooks`.
fn hooks() -> Vec<String> {
    OPTIONS_TABLE
        .iter()
        .filter(|oe| oe.flags & OPTIONS_TABLE_IS_HOOK != 0)
        .map(|oe| oe.name.to_string())
        .collect()
}

/// The values the option named earlier on the line accepts: `on`/`off` for a
/// flag, its choice list for a choice, nothing for free-form options.
fn option_values(before: &[&str]) -> Vec<String> {
    let Some(name) = before.iter().rev().find(|w| !w.starts_with('-')) else {
        return Vec::new();
    };
    // `mode-keys[0]` and `@user-option` never reach the table by that spelling.
    let name = name.split_once('[').map_or(*name, |(base, _)| base);
    let Some(oe) = OPTIONS_TABLE.iter().find(|oe| oe.name == name) else {
        return Vec::new();
    };
    match oe.type_ {
        options_table_type::OPTIONS_TABLE_FLAG => {
            vec!["on".to_string(), "off".to_string()]
        }
        options_table_type::OPTIONS_TABLE_CHOICE => {
            oe.choices.iter().map(|c| (*c).to_string()).collect()
        }
        _ => Vec::new(),
    }
}

/// Session names, as `target-session` accepts them.
fn sessions() -> Vec<String> {
    unsafe {
        rb_foreach(&raw mut crate::session_::SESSIONS)
            .map(|s| (*s.as_ptr()).name.to_string())
            .collect()
    }
}

/// Window targets: `session:index` and `session:name` for every window.
fn windows() -> Vec<String> {
    let mut list = Vec::new();
    unsafe {
        for s in rb_foreach(&raw mut crate::session_::SESSIONS).map(NonNull::as_ptr) {
            let name = (*s).name.to_string();
            for wl in rb_foreach(&raw mut (*s).windows).map(NonNull::as_ptr) {
                list.push(format!("{name}:{}", (*wl).idx));
                let w = (*wl).window;
                if !w.is_null()
                    && let Some(wname) = (*w).name.as_deref().and_then(|n| n.to_str().ok())
                    && !wname.is_empty()
                {
                    list.push(format!("{name}:{wname}"));
                }
            }
        }
    }
    list
}

/// Pane targets: the `%id` of every pane plus its `session:window.index`.
fn panes() -> Vec<String> {
    let mut list = Vec::new();
    unsafe {
        for s in rb_foreach(&raw mut crate::session_::SESSIONS).map(NonNull::as_ptr) {
            let name = (*s).name.to_string();
            for wl in rb_foreach(&raw mut (*s).windows).map(NonNull::as_ptr) {
                let w = (*wl).window;
                if w.is_null() {
                    continue;
                }
                for wp in tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).map(NonNull::as_ptr)
                {
                    list.push(format!("%{}", (*wp).id));
                    let mut idx = 0u32;
                    if crate::window_::window_pane_index(wp, &raw mut idx) == 0 {
                        list.push(format!("{name}:{}.{idx}", (*wl).idx));
                    }
                }
            }
        }
    }
    list
}

/// Attached client names (the tty each client is on).
fn clients() -> Vec<String> {
    let mut list = Vec::new();
    unsafe {
        for c in tailq_foreach::<_, ()>(&raw mut crate::server::CLIENTS).map(NonNull::as_ptr) {
            if !(*c).name.is_null()
                && let Ok(name) = CStr::from_ptr((*c).name.cast()).to_str()
            {
                list.push(name.to_string());
            }
        }
    }
    list
}

/// Paste buffer names, newest first as `paste_walk` walks them.
fn buffers() -> Vec<String> {
    let mut list = Vec::new();
    unsafe {
        let mut pb = crate::paste::paste_walk(null_mut());
        while let Some(buffer) = NonNull::new(pb) {
            list.push(crate::paste::paste_buffer_name(buffer).to_string());
            pb = crate::paste::paste_walk(pb);
        }
    }
    list
}

/// Key names, taken from what is bound in the table the line names (`-T`) or
/// the prefix table otherwise — the keys a `bind-key` / `unbind-key` line is
/// most likely to mean.
fn keys(before: &[&str]) -> Vec<String> {
    let table = before
        .iter()
        .position(|w| *w == "-T")
        .and_then(|i| before.get(i + 1))
        .copied()
        .unwrap_or("prefix");
    let Ok(name) = std::ffi::CString::new(table) else {
        return Vec::new();
    };
    let mut list = Vec::new();
    unsafe {
        let mut kt = crate::key_bindings_::key_bindings_first_table();
        while !kt.is_null() {
            if (*kt).name.as_c_str() == name.as_c_str() {
                let mut bd = crate::key_bindings_::key_bindings_first(kt);
                while !bd.is_null() {
                    let s = crate::key_string::key_string_lookup_key((*bd).key, 0);
                    if !s.is_null()
                        && let Ok(key) = CStr::from_ptr(s.cast()).to_str()
                    {
                        list.push(key.to_string());
                    }
                    bd = crate::key_bindings_::key_bindings_next(kt, bd);
                }
                break;
            }
            kt = crate::key_bindings_::key_bindings_next_table(kt);
        }
    }
    list
}

/// Key table names, the built-in ones plus whatever `bind-key -T` has created.
fn key_tables() -> Vec<String> {
    let mut list = Vec::new();
    unsafe {
        let mut kt = crate::key_bindings_::key_bindings_first_table();
        while !kt.is_null() {
            if let Ok(name) = (*kt).name.to_str() {
                list.push(name.to_string());
            }
            kt = crate::key_bindings_::key_bindings_next_table(kt);
        }
    }
    list
}

/// Environment variable names, as `set-environment` / `show-environment` name
/// them.
fn variables() -> Vec<String> {
    let mut list = Vec::new();
    unsafe {
        let env = crate::tmux::GLOBAL_ENVIRON;
        if env.is_null() {
            return list;
        }
        let mut envent = crate::environ_::environ_first(env);
        while !envent.is_null() {
            if let Ok(name) = CStr::from_ptr((*envent).name_ptr().cast()).to_str() {
                list.push(name.to_string());
            }
            envent = crate::environ_::environ_next(envent);
        }
    }
    list
}

/// Filesystem candidates for a path slot, read off the directory the typed
/// word names ([`super::repl::path_candidates`] does the walking for both
/// surfaces).
fn paths(word: &str, paths: super::shell::Paths) -> Vec<String> {
    super::repl::path_candidates(word, paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The usage strings are the whole source of slot meaning, so the parser has
    // to read the real shapes: bracketed value flags, bare mandatory flags,
    // boolean clusters, optional and variadic positionals.
    #[test]
    fn usage_parses_flags_and_positionals() {
        let u = parse_usage("[-a] [-b buffer-name] path");
        assert_eq!(u.flags, [('b', "buffer-name".to_string())]);
        assert_eq!(u.positionals, ["path"]);
        assert!(!u.variadic);

        // A mandatory (unbracketed) flag binds its value the same way, and the
        // trailing `...` marks the positional as repeating.
        let u = parse_usage("[-FHKlMRX] [-c target-client] -t target-pane key ...");
        assert!(u.flags.contains(&('t', "target-pane".to_string())));
        assert!(u.flags.contains(&('c', "target-client".to_string())));
        assert_eq!(u.positionals, ["key"]);
        assert!(u.variadic);

        // Option commands: the value slot follows the option name slot.
        let u = parse_usage("[-aFgopqsuUw] [-t target-pane] option [value]");
        assert_eq!(u.positionals, ["option", "value"]);
        assert_eq!(u.flags, [('t', "target-pane".to_string())]);
    }

    // Which slot the cursor is in comes from walking the words already typed:
    // flags with values swallow the next word, everything else is positional.
    #[test]
    fn slot_follows_the_words_already_typed() {
        let resize = crate::cmd_::cmd_find("resize-pane").unwrap();
        assert_eq!(
            slot(resize, &["resize-pane", "-t"]),
            Some("target-pane".to_string())
        );
        // `-Z` takes no value, and resize-pane's usage names no positional, so
        // the word after it means nothing in particular.
        assert_eq!(slot(resize, &["resize-pane", "-Z"]), None);

        // `send-keys ... -t target-pane key ...`: the target follows `-t`, and
        // an inline value (`-t%1`) is already complete, so the next word is the
        // (variadic) key positional.
        let send = crate::cmd_::cmd_find("send-keys").unwrap();
        assert_eq!(
            slot(send, &["send-keys", "-t"]),
            Some("target-pane".to_string())
        );
        assert_eq!(slot(send, &["send-keys", "-t%1"]), Some("key".to_string()));
        assert_eq!(
            slot(send, &["send-keys", "-t", "%1", "C-c"]),
            Some("key".to_string())
        );

        let set = crate::cmd_::cmd_find("set-option").unwrap();
        assert_eq!(slot(set, &["set"]), Some("option".to_string()));
        assert_eq!(
            slot(set, &["set", "-g", "mode-keys"]),
            Some("value".to_string())
        );
    }

    // `:resize-pane -<Tab>` - the reported case: a `-` word completes flags.
    #[test]
    fn dash_word_completes_the_commands_flags() {
        assert_eq!(
            candidates(&["resize-pane"], "-"),
            ["-D", "-L", "-M", "-R", "-T", "-U", "-Z", "-t", "-x", "-y"]
        );
        assert_eq!(candidates(&["resize-pane"], "-Z"), ["-Z"]);
        assert!(candidates(&["resize-pane"], "-q").is_empty());
        // An alias resolves to the same entry as the full name.
        assert_eq!(
            candidates(&["resizep"], "-"),
            candidates(&["resize-pane"], "-")
        );
    }

    // `:setw <Tab>` - the reported case: an empty word offers the option names
    // that command sets, and only the ones in its scope.
    #[test]
    fn option_commands_complete_their_own_options() {
        let setw = candidates(&["setw"], "");
        assert!(setw.iter().any(|o| o == "main-pane-width"), "got {setw:?}");
        assert!(
            !setw.iter().any(|o| o == "buffer-limit"), // server scope
            "server option leaked into setw: {setw:?}"
        );
        // `set-option` reaches every scope.
        assert!(
            candidates(&["set"], "buffer-")
                .iter()
                .any(|o| o == "buffer-limit")
        );
        // Hooks live behind their own command rather than burying the options.
        assert!(!candidates(&["set"], "").iter().any(|o| o == "pane-died"));
        assert!(
            candidates(&["set-hook"], "")
                .iter()
                .any(|o| o == "pane-died")
        );
    }

    // The value slot knows the option named before it.
    #[test]
    fn option_values_come_from_the_option_named() {
        assert_eq!(candidates(&["set", "-g", "mode-keys"], ""), ["emacs", "vi"]);
        assert_eq!(candidates(&["setw", "monitor-bell"], ""), ["off", "on"]);
        // An array subscript still names the same option.
        assert_eq!(
            candidates(&["set", "-g", "status-keys[0]"], ""),
            ["emacs", "vi"]
        );
        // A free-form option has no vocabulary, so the flags stand in.
        assert!(
            !candidates(&["set", "-g", "status-left"], "")
                .iter()
                .any(|v| v == "on")
        );
    }

    // Slots whose usage spells its own values out (`[-o json|jsonl|csv]`).
    #[test]
    fn inline_alternatives_complete_from_the_usage() {
        let out = candidates(&["list-panes", "-o"], "");
        assert!(out.contains(&"json".to_string()), "got {out:?}");
        assert!(out.contains(&"yaml".to_string()), "got {out:?}");
    }

    #[test]
    fn layout_and_command_slots() {
        assert_eq!(candidates(&["select-layout"], "")[0], "even-horizontal");
        assert_eq!(candidates(&["select-layout"], "tile"), ["tiled"]);
        // `bind-key key command` - the command slot offers command names.
        let bound = candidates(&["bind-key", "M-x"], "new-w");
        assert!(bound.contains(&"new-window".to_string()), "got {bound:?}");
    }

    // The first word still completes command names, and an empty prompt has
    // nothing to narrow the table with.
    #[test]
    fn first_word_completes_commands() {
        let out = candidates(&[], "resize-p");
        assert_eq!(out, ["resize-pane"]);
        assert!(candidates(&[], "").is_empty());
        // Extension verbs are offered next to the ported commands.
        assert!(!candidates(&[], "z").is_empty());
    }

    // ztmux's own verbs complete from the harvested zsh completion: long
    // options, their values, and the verb's subcommand vocabulary.
    #[test]
    fn extension_verbs_complete_from_their_harvested_spec() {
        let flags = candidates(&["buffers"], "--");
        assert_eq!(flags, ["--json"], "got {flags:?}");
        assert_eq!(
            candidates(&["triggers"], ""),
            ["add", "arm", "disarm", "list", "test", "wizard"]
        );
        assert_eq!(candidates(&["triggers"], "w"), ["wizard"]);
    }

    // The live slots read the server's own trees, which are empty in a unit
    // test - they must come back empty rather than walk off a zeroed head.
    #[test]
    fn live_slots_survive_an_empty_server() {
        // Each falls back to the command's flags once its slot comes up empty.
        assert_eq!(candidates(&["kill-pane", "-t"], ""), ["-a", "-f", "-t"]); // panes
        assert!(!candidates(&["paste-buffer", "-b"], "").is_empty()); // buffers
        assert!(!candidates(&["attach-session", "-t"], "").is_empty()); // sessions
        assert!(!candidates(&["bind-key"], "").is_empty()); // keys
        assert!(!candidates(&["display-message", "-c"], "").is_empty()); // clients
        assert!(!candidates(&["set-environment"], "").is_empty()); // variables
    }

    // A command whose usage names no positional falls back to its flags rather
    // than to an empty palette.
    #[test]
    fn empty_word_falls_back_to_flags() {
        assert_eq!(candidates(&["kill-pane"], ""), ["-a", "-f", "-t"]);
    }
}
