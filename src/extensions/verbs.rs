//! `ztmux verbs` — list every verb ztmux answers to, with its description.
//!
//! ztmux answers to far more than tmux does: the ported commands and their
//! aliases, plus the ztmux extensions. `ztmux list-commands` covers the first
//! group only (and prints usage strings, not descriptions), so there was no way
//! to see the whole surface at once. This is that listing, grouped by kind and
//! filterable:
//!
//! ```text
//! ztmux verbs            # everything
//! ztmux verbs pane       # verbs whose name or description mentions "pane"
//! ztmux verbs -o json    # machine-readable
//! ```
//!
//! The table is built from `CMD_TABLE` and `EXTENSION_COMMANDS` themselves, so
//! it cannot list a verb that does not run or miss one that does; descriptions
//! come from the harvested zsh completion ([`super::completion_spec`]), falling
//! back to the command's own usage string. [`super::repl`] lists the same table
//! (its `verbs` builtin) and completes against it.

use std::borrow::Cow;
use std::io::IsTerminal;

use super::completion_spec::VERB_DESCRIPTIONS;
use crate::cmd_::CMD_TABLE;

/// What a verb is — the listing's grouping, and its `kind` in JSON.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Command,
    Alias,
    Extension,
    Builtin,
}

impl Kind {
    /// Section heading, JSON name, and heading colour.
    const fn label(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Kind::Command => ("COMMANDS", "command", "36"),
            Kind::Alias => ("ALIASES", "alias", "34"),
            Kind::Extension => ("EXTENSIONS", "extension", "35"),
            Kind::Builtin => ("CONSOLE", "console", "33"),
        }
    }
}

/// One listable (and completable) verb.
pub(crate) struct Verb {
    pub(crate) name: &'static str,
    pub(crate) kind: Kind,
    /// The command an alias stands for; the description otherwise.
    pub(crate) description: Cow<'static, str>,
}

/// The order the listing prints its groups in.
const GROUPS: [Kind; 4] = [Kind::Command, Kind::Alias, Kind::Extension, Kind::Builtin];

pub(crate) fn run(_socket: &str) -> i32 {
    let args: Vec<String> = std::env::args().collect();
    let json = args.iter().any(|a| a == "--json")
        || args.windows(2).any(|w| w[0] == "-o" && w[1] == "json");

    if json {
        print!("{}", render_json(filter_arg(&args)));
    } else {
        print_verbs(filter_arg(&args));
    }
    0
}

/// The filter: the first word after `verbs` that is not a flag or a flag's
/// value, so `verbs json` filters on "json" while `verbs -o json` does not.
fn filter_arg(args: &[String]) -> Option<&str> {
    let mut rest = args.iter().skip_while(|a| *a != "verbs").skip(1);
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            // The only value-taking flag; its value is the output format.
            "-o" => {
                rest.next();
            }
            flag if flag.starts_with('-') => {}
            word => return Some(word),
        }
    }
    None
}

/// Every verb ztmux answers to: the ported commands, their aliases, the
/// extensions, and the console builtins. Sourced from `CMD_TABLE` and
/// `EXTENSION_COMMANDS`, so nothing here can drift from what actually runs.
pub(crate) fn all_verbs() -> Vec<Verb> {
    let mut verbs: Vec<Verb> = Vec::new();
    for entry in CMD_TABLE {
        verbs.push(Verb {
            name: entry.name,
            kind: Kind::Command,
            description: Cow::Borrowed(describe(entry.name).unwrap_or(entry.usage)),
        });
        if let Some(alias) = entry.alias {
            verbs.push(Verb {
                name: alias,
                kind: Kind::Alias,
                description: Cow::Owned(format!("alias for {}", entry.name)),
            });
        }
    }
    for &name in super::EXTENSION_COMMANDS {
        verbs.push(Verb {
            name,
            kind: Kind::Extension,
            description: Cow::Borrowed(describe(name).unwrap_or("ztmux extension")),
        });
    }
    // Console builtins: the console's own commands and the shell builtins.
    // `verbs` and `banner` are both builtins and extension verbs; list each
    // once, as the extension, since that is what runs everywhere.
    let builtins = super::repl::BUILTINS
        .iter()
        .map(|&(name, description)| (name, description))
        .chain(super::shell::described());
    for (name, description) in builtins {
        if super::EXTENSION_COMMANDS.contains(&name) {
            continue;
        }
        verbs.push(Verb {
            name,
            kind: Kind::Builtin,
            description: Cow::Borrowed(description),
        });
    }
    verbs.sort_by_key(|v| v.name);
    verbs
}

/// The one-line description harvested for `name`, if the zsh completion has one.
pub(crate) fn describe(name: &str) -> Option<&'static str> {
    VERB_DESCRIPTIONS
        .binary_search_by(|(v, _)| (*v).cmp(name))
        .ok()
        .map(|i| VERB_DESCRIPTIONS[i].1)
}

/// Whether `filter` (a substring of the name or the description) keeps `verb`.
fn matches(verb: &Verb, filter: Option<&str>) -> bool {
    match filter {
        Some(f) => verb.name.contains(f) || verb.description.contains(f),
        None => true,
    }
}

/// Print every verb grouped by kind, name column sized to the widest match.
pub(crate) fn print_verbs(filter: Option<&str>) {
    let color = colored();
    let verbs = all_verbs();
    let width = verbs
        .iter()
        .filter(|v| matches(v, filter))
        .map(|v| v.name.len())
        .max()
        .unwrap_or(0);

    let mut total = 0;
    for kind in GROUPS {
        let group: Vec<&Verb> = verbs
            .iter()
            .filter(|v| v.kind == kind && matches(v, filter))
            .collect();
        if group.is_empty() {
            continue;
        }
        total += group.len();
        let (label, _, code) = kind.label();
        println!(
            "{}",
            paint(&format!("{label} ({})", group.len()), code, color)
        );
        for v in group {
            println!(
                "  {:<width$}  {}",
                v.name,
                paint(&v.description, "2", color)
            );
        }
        println!();
    }
    if total == 0 {
        println!("no verb matches {}", filter.unwrap_or(""));
    }
}

/// The same listing as a JSON array of `{verb, kind, description}`.
fn render_json(filter: Option<&str>) -> String {
    let rows: Vec<serde_json::Value> = all_verbs()
        .iter()
        .filter(|v| matches(v, filter))
        .map(|v| {
            serde_json::json!({
                "verb": v.name,
                "kind": v.kind.label().1,
                "description": v.description,
            })
        })
        .collect();
    format!(
        "{}\n",
        serde_json::to_string_pretty(&serde_json::Value::Array(rows)).unwrap_or_default()
    )
}

/// Whether to colour output: a terminal, and `NO_COLOR` unset. Shared with
/// [`super::repl`], whose banner and help screen colour the same way.
pub(crate) fn colored() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

/// Wrap `s` in SGR `code` when colouring.
pub(crate) fn paint(s: &str, code: &str, color: bool) -> String {
    if color {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

/// Strip SGR sequences, for measuring a coloured string's printed width.
pub(crate) fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            out.push(c);
            continue;
        }
        // CSI …m — drop everything through the final byte.
        for c in chars.by_ref() {
            if c.is_ascii_alphabetic() {
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_alias_and_extension_is_listed_exactly_once() {
        let verbs = all_verbs();
        for entry in CMD_TABLE {
            assert_eq!(
                verbs.iter().filter(|v| v.name == entry.name).count(),
                1,
                "{} missing or duplicated",
                entry.name
            );
        }
        for &name in super::super::EXTENSION_COMMANDS {
            assert_eq!(
                verbs.iter().filter(|v| v.name == name).count(),
                1,
                "{name} missing or duplicated"
            );
        }
        // `verbs` and `banner` are both extensions and console builtins:
        // listed once, as the extension, so the count above holds and the
        // kinds do not clash.
        for name in ["verbs", "banner"] {
            let entry = verbs.iter().find(|v| v.name == name).expect(name);
            assert!(
                entry.kind == Kind::Extension,
                "{name} is listed as a builtin"
            );
        }
        // A console-only builtin still shows up, shell builtins included.
        assert!(
            verbs
                .iter()
                .any(|v| v.name == "help" && v.kind == Kind::Builtin)
        );
        assert!(
            verbs
                .iter()
                .any(|v| v.name == "cd" && v.kind == Kind::Builtin)
        );
    }

    #[test]
    fn every_extension_verb_has_a_description() {
        // The listing, the console banner and Tab completion all read
        // descriptions out of the harvested table; a missing one means
        // completions/_ztmux has no block for that verb — it completes nothing
        // in the shell either.
        let missing: Vec<&str> = super::super::EXTENSION_COMMANDS
            .iter()
            .copied()
            .filter(|name| describe(name).is_none())
            .collect();
        assert!(missing.is_empty(), "no zsh completion for: {missing:?}");
    }

    #[test]
    fn aliases_point_at_their_command_and_commands_carry_descriptions() {
        let verbs = all_verbs();
        let killp = verbs.iter().find(|v| v.name == "killp").expect("killp");
        assert_eq!(killp.description, "alias for kill-pane");
        assert!(killp.kind == Kind::Alias);
        let kill_pane = verbs
            .iter()
            .find(|v| v.name == "kill-pane")
            .expect("kill-pane");
        assert_eq!(kill_pane.description, "destroy a given pane");
    }

    #[test]
    fn filter_matches_name_or_description() {
        let verbs = all_verbs();
        // By name.
        assert!(matches(
            verbs.iter().find(|v| v.name == "kill-pane").unwrap(),
            Some("kill")
        ));
        // By description only — `solo` never spells "pane" in its name.
        let solo = verbs.iter().find(|v| v.name == "solo").unwrap();
        assert!(!solo.name.contains("pane"));
        assert!(matches(solo, Some("pane")));
        assert!(!matches(solo, Some("no-such-thing")));
    }

    #[test]
    fn json_carries_verb_kind_and_description() {
        let v: serde_json::Value = serde_json::from_str(&render_json(Some("kill-pane"))).unwrap();
        let row = v
            .as_array()
            .expect("array")
            .iter()
            .find(|r| r["verb"] == "kill-pane")
            .expect("kill-pane row");
        assert_eq!(row["kind"], "command");
        assert_eq!(row["description"], "destroy a given pane");
        // A filter that matches nothing is an empty array, not an error.
        let empty: serde_json::Value =
            serde_json::from_str(&render_json(Some("zzz-no-such-verb"))).unwrap();
        assert_eq!(empty.as_array().expect("array").len(), 0);
    }

    #[test]
    fn the_filter_is_the_first_word_that_is_not_a_flag_or_a_flags_value() {
        let argv = |args: &[&str]| -> Vec<String> {
            std::iter::once("ztmux")
                .chain(args.iter().copied())
                .map(str::to_string)
                .collect()
        };
        assert_eq!(filter_arg(&argv(&["verbs"])), None);
        assert_eq!(filter_arg(&argv(&["verbs", "pane"])), Some("pane"));
        // `-o json` is the output format, not a filter — but a bare `json` is.
        assert_eq!(filter_arg(&argv(&["verbs", "-o", "json"])), None);
        assert_eq!(filter_arg(&argv(&["verbs", "json"])), Some("json"));
        assert_eq!(
            filter_arg(&argv(&["verbs", "--json", "pane"])),
            Some("pane")
        );
        // Words before the verb (socket flags) are never the filter.
        assert_eq!(filter_arg(&argv(&["-S", "/tmp/s", "verbs"])), None);
    }

    #[test]
    fn strip_ansi_measures_printed_width() {
        assert_eq!(strip_ansi(&paint("verbs", "36", true)), "verbs");
        assert_eq!(paint("verbs", "36", false), "verbs");
    }
}
