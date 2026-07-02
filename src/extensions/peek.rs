//! `ztmux peek` — dump the visible contents of every pane at once.
//!
//! A one-shot client subcommand that captures each pane
//! (via [`super::tmux_query::capture_pane`]) and prints it under a header
//! identifying the pane — the "show me what is on every screen right now" view,
//! handy when driving many panes at once. `-t <substr>` limits the dump to
//! panes whose `session:window.pane` location contains the substring. With
//! `-o json` / `--json` it emits an array of `{pane, location, command,
//! content}` objects instead.

use std::io::IsTerminal;

use super::tmux_query::{Pane, Snapshot, capture_pane, poll};

/// One captured pane: its identity plus the screen text.
struct Peek {
    id: String,
    loc: String,
    command: String,
    content: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let filter = target_arg();
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux peek: {e}");
        return 1;
    }
    let peeks = capture(&snap, filter.as_deref(), |p| {
        capture_pane(socket, &p.id, false).unwrap_or_default()
    });
    let json = std::env::args().any(|a| a == "--json")
        || std::env::args()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|w| w[0] == "-o" && w[1] == "json");
    if json {
        print!("{}", render_json(&peeks));
    } else {
        print!("{}", render_text(&peeks, std::io::stdout().is_terminal()));
    }
    0
}

/// The value of `-t <substr>`, if present.
fn target_arg() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.windows(2)
        .find(|w| w[0] == "-t")
        .map(|w| w[1].clone())
}

/// Build the ordered list of panes to dump, running `grab` for each pane's
/// text. Split from `run` so ordering/filtering is unit-testable without a
/// live server (the test passes a stub `grab`).
fn capture<F: Fn(&Pane) -> String>(snap: &Snapshot, filter: Option<&str>, grab: F) -> Vec<Peek> {
    let mut panes: Vec<&Pane> = snap
        .panes
        .iter()
        .filter(|p| filter.is_none_or(|f| loc(p).contains(f)))
        .collect();
    panes.sort_by_key(|p| loc(p));
    panes
        .into_iter()
        .map(|p| Peek {
            id: p.id.clone(),
            loc: loc(p),
            command: p.command.clone(),
            content: grab(p),
        })
        .collect()
}

fn loc(p: &Pane) -> String {
    format!("{}:{}.{}", p.session, p.window, p.index)
}

fn render_text(peeks: &[Peek], color: bool) -> String {
    let paint = |s: &str, code: &str| -> String {
        if color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };
    let mut out = String::new();
    for p in peeks {
        out.push_str(&format!(
            "{}\n",
            paint(&format!("── {} {} [{}] ──", p.id, p.loc, p.command), "1;36")
        ));
        out.push_str(p.content.trim_end_matches('\n'));
        out.push('\n');
    }
    out
}

fn render_json(peeks: &[Peek]) -> String {
    let arr: Vec<serde_json::Value> = peeks
        .iter()
        .map(|p| {
            serde_json::json!({
                "pane": p.id,
                "location": p.loc,
                "command": p.command,
                "content": p.content,
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

    fn snap() -> Snapshot {
        Snapshot {
            panes: vec![
                Pane {
                    id: "%1".into(),
                    session: "work".into(),
                    window: 1,
                    index: 0,
                    command: "nvim".into(),
                    ..Default::default()
                },
                Pane {
                    id: "%0".into(),
                    session: "work".into(),
                    window: 0,
                    index: 0,
                    command: "zsh".into(),
                    ..Default::default()
                },
                Pane {
                    id: "%2".into(),
                    session: "ops".into(),
                    window: 0,
                    index: 0,
                    command: "top".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    // Stub capture: each pane yields "screen of <id>".
    fn grab(p: &Pane) -> String {
        format!("screen of {}", p.id)
    }

    #[test]
    fn captures_every_pane_sorted_by_location() {
        let peeks = capture(&snap(), None, grab);
        let locs: Vec<&str> = peeks.iter().map(|p| p.loc.as_str()).collect();
        assert_eq!(locs, vec!["ops:0.0", "work:0.0", "work:1.0"]);
    }

    #[test]
    fn target_filter_limits_to_matching_locations() {
        let peeks = capture(&snap(), Some("work:"), grab);
        assert_eq!(peeks.len(), 2);
        assert!(peeks.iter().all(|p| p.loc.starts_with("work:")));
    }

    #[test]
    fn text_has_a_header_and_content_per_pane() {
        let peeks = capture(&snap(), Some("ops"), grab);
        let s = render_text(&peeks, false);
        assert!(s.contains("── %2 ops:0.0 [top] ──"));
        assert!(s.contains("screen of %2"));
    }

    #[test]
    fn json_is_an_array_of_pane_screens() {
        let peeks = capture(&snap(), None, grab);
        let v: serde_json::Value = serde_json::from_str(&render_json(&peeks)).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 3);
        assert_eq!(v[0]["location"], "ops:0.0");
        assert_eq!(v[0]["content"], "screen of %2");
    }

    // Trailing blank capture lines are trimmed so headers stay flush.
    #[test]
    fn trailing_newlines_trimmed_in_text() {
        let peeks = capture(&snap(), Some("ops"), |_| "a\nb\n\n\n".into());
        let s = render_text(&peeks, false);
        assert!(s.ends_with("a\nb\n"));
        assert!(!s.ends_with("\n\n"));
    }
}
