//! `ztmux grep` — search the live contents of every pane.
//!
//! Where [`super::find`] matches pane *metadata* (command, path, title, window
//! name), this matches what is actually *on the screen*: it captures each pane
//! (via [`super::tmux_query::capture_pane`]) and prints the lines containing the
//! query. Matching is a case-insensitive substring test. By default only the
//! visible screen is scanned; `-a` / `--history` extends the search to the full
//! scrollback. Output is `location:line: text` (grep-like, coloured when stdout
//! is a TTY) or a JSON array with `-o json` / `--json`. It exits non-zero when
//! nothing matched, so it composes in shell `if` tests.

use std::io::IsTerminal;

use super::tmux_query::{Pane, Snapshot, capture_pane, poll};

/// One matched line within a pane.
struct Match {
    id: String,
    loc: String,
    lineno: usize,
    text: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let Some(query) = query_arg() else {
        eprintln!("usage: ztmux grep <pattern> [-a] [-o json]");
        return 2;
    };
    let history = std::env::args().any(|a| a == "-a" || a == "--history");
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux grep: {e}");
        return 1;
    }
    // Capture every pane, then match. Kept separate from search() so the
    // matcher is unit-testable without a live server.
    let captures: Vec<(Pane, String)> = snap
        .panes
        .iter()
        .filter_map(|p| capture_pane(socket, &p.id, history).map(|c| (p.clone(), c)))
        .collect();
    let matches = search(&captures, &query);
    let json = std::env::args().any(|a| a == "--json")
        || std::env::args()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|w| w[0] == "-o" && w[1] == "json");
    if json {
        print!("{}", render_json(&matches));
    } else {
        print!("{}", render_text(&matches, std::io::stdout().is_terminal()));
    }
    i32::from(matches.is_empty())
}

/// The first positional argument after the `grep` subcommand, skipping flags
/// and the `json` value of `-o json`.
fn query_arg() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let start = args.iter().position(|a| a == "grep")? + 1;
    args[start..]
        .iter()
        .find(|a| !a.starts_with('-') && a.as_str() != "json")
        .cloned()
}

/// Scan each pane's captured text for lines containing `query`, oldest pane
/// (by location) first, in on-screen line order.
fn search(captures: &[(Pane, String)], query: &str) -> Vec<Match> {
    let needle = query.to_lowercase();
    let mut ordered: Vec<&(Pane, String)> = captures.iter().collect();
    ordered.sort_by(|a, b| loc(&a.0).cmp(&loc(&b.0)));
    let mut out = Vec::new();
    for (p, content) in ordered {
        for (i, line) in content.lines().enumerate() {
            if line.to_lowercase().contains(&needle) {
                out.push(Match {
                    id: p.id.clone(),
                    loc: loc(p),
                    lineno: i + 1,
                    text: line.to_string(),
                });
            }
        }
    }
    out
}

fn loc(p: &Pane) -> String {
    format!("{}:{}.{}", p.session, p.window, p.index)
}

fn render_text(matches: &[Match], color: bool) -> String {
    let paint = |s: &str, code: &str| -> String {
        if color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };
    let mut out = String::new();
    for m in matches {
        out.push_str(&format!(
            "{}:{}: {}\n",
            paint(&m.loc, "36"),
            paint(&m.lineno.to_string(), "32"),
            m.text,
        ));
    }
    out
}

fn render_json(matches: &[Match]) -> String {
    let arr: Vec<serde_json::Value> = matches
        .iter()
        .map(|m| {
            serde_json::json!({
                "pane": m.id,
                "location": m.loc,
                "line": m.lineno,
                "text": m.text,
            })
        })
        .collect();
    format!(
        "{}\n",
        serde_json::to_string_pretty(&serde_json::Value::Array(arr)).unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(id: &str, session: &str, window: i64, index: i64) -> Pane {
        Pane {
            id: id.into(),
            session: session.into(),
            window,
            index,
            ..Default::default()
        }
    }

    fn captures() -> Vec<(Pane, String)> {
        vec![
            (
                pane("%1", "work", 0, 1),
                "cargo build\nerror[E0308]: mismatched types\n  --> src/main.rs".into(),
            ),
            (
                pane("%0", "work", 0, 0),
                "$ ls\nCargo.toml\nsrc\nERROR: nothing here\n".into(),
            ),
        ]
    }

    #[test]
    fn matches_lines_case_insensitively_across_panes() {
        let m = search(&captures(), "error");
        // "error[E0308]" in %1 and "ERROR: nothing" in %0 both match.
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn results_ordered_by_location_then_line() {
        let m = search(&captures(), "error");
        // %0 (work:0.0) sorts before %1 (work:0.1) despite input order.
        assert_eq!(m[0].loc, "work:0.0");
        assert_eq!(m[0].lineno, 4);
        assert_eq!(m[1].loc, "work:0.1");
        assert_eq!(m[1].lineno, 2);
    }

    #[test]
    fn no_match_yields_empty() {
        assert!(search(&captures(), "kubernetes").is_empty());
    }

    #[test]
    fn line_numbers_are_one_based() {
        let m = search(&captures(), "cargo build");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].lineno, 1);
        assert_eq!(m[0].id, "%1");
    }

    #[test]
    fn json_carries_pane_location_line_and_text() {
        let m = search(&captures(), "mismatched");
        let v: serde_json::Value = serde_json::from_str(&render_json(&m)).unwrap();
        assert_eq!(v[0]["pane"], "%1");
        assert_eq!(v[0]["location"], "work:0.1");
        assert_eq!(v[0]["line"], 2);
        assert!(v[0]["text"].as_str().unwrap().contains("mismatched"));
    }

    #[test]
    fn text_render_is_grep_like_without_color() {
        let m = search(&captures(), "Cargo.toml");
        let s = render_text(&m, false);
        assert_eq!(s, "work:0.0:2: Cargo.toml\n");
    }
}
