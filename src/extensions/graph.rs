//! `ztmux graph` — render the server tree as a diagram.
//!
//! A one-shot client subcommand (like [`super::tree`]) built from the
//! `list-* -o json` query layer. It emits the session→window→pane tree as
//! Graphviz DOT, Mermaid, or a self-contained HTML page (Mermaid + CDN script)
//! for docs and screenshots. Select the format with `-o dot|mermaid|html`
//! (default `mermaid`).

use super::tmux_query::{Snapshot, poll};

#[derive(Clone, Copy)]
enum Format {
    Dot,
    Mermaid,
    Html,
}

pub(crate) fn run(socket: &str) -> i32 {
    let fmt = match parse_format() {
        Ok(f) => f,
        Err(bad) => {
            eprintln!("ztmux graph: unknown format '{bad}' (want dot|mermaid|html)");
            return 1;
        }
    };
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux graph: {e}");
        return 1;
    }
    print!(
        "{}",
        match fmt {
            Format::Dot => render_dot(&snap),
            Format::Mermaid => render_mermaid(&snap),
            Format::Html => render_html(&snap),
        }
    );
    0
}

/// Read `-o <fmt>` / `--<fmt>` from argv; default Mermaid.
fn parse_format() -> Result<Format, String> {
    let args: Vec<String> = std::env::args().collect();
    for (i, a) in args.iter().enumerate() {
        let val = if a == "-o" {
            args.get(i + 1).map(String::as_str)
        } else {
            a.strip_prefix("--")
        };
        if let Some(v) = val {
            return match v {
                "dot" | "gv" | "graphviz" => Ok(Format::Dot),
                "mermaid" | "mmd" => Ok(Format::Mermaid),
                "html" => Ok(Format::Html),
                other => Err(other.to_string()),
            };
        }
    }
    Ok(Format::Mermaid)
}

/// A node id safe for DOT/Mermaid (alphanumeric + underscore).
fn nid(prefix: &str, raw: &str) -> String {
    // Build a graph node id from a tmux object id. The tmux sigils ($ session,
    // @ window, % pane) are dropped so "$0" becomes "s_0", "@3" -> "w_3", etc.;
    // the prefix already namespaces the number, so the bare digits are unique
    // and read cleanly in the emitted DOT/Mermaid.
    let mut s = String::from(prefix);
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        }
    }
    s
}

/// Iterate the tree as (session, its windows, and each window's panes) so every
/// renderer shares the same walk order.
fn walk(
    snap: &Snapshot,
) -> impl Iterator<Item = (&super::tmux_query::Session, Vec<&super::tmux_query::Window>)> {
    snap.sessions.iter().map(move |s| {
        let mut wins: Vec<_> = snap
            .windows
            .iter()
            .filter(|w| w.session == s.name)
            .collect();
        wins.sort_by_key(|w| w.index);
        (s, wins)
    })
}

fn panes_of<'a>(
    snap: &'a Snapshot,
    session: &str,
    window: i64,
) -> Vec<&'a super::tmux_query::Pane> {
    let mut ps: Vec<_> = snap
        .panes
        .iter()
        .filter(|p| p.session == session && p.window == window)
        .collect();
    ps.sort_by_key(|p| p.index);
    ps
}

fn esc_dot(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn render_dot(snap: &Snapshot) -> String {
    let mut out =
        String::from("digraph ztmux {\n  rankdir=LR;\n  node [shape=box, style=rounded];\n");
    out.push_str("  server [label=\"ztmux server\", shape=doubleoctagon];\n");
    for (s, wins) in walk(snap) {
        let sn = nid("s_", &s.id);
        let mark = if s.attached { " *" } else { "" };
        out.push_str(&format!(
            "  {sn} [label=\"{}{}\"];\n  server -> {sn};\n",
            esc_dot(&s.name),
            mark
        ));
        for w in wins {
            let wn = nid("w_", &w.id);
            out.push_str(&format!(
                "  {wn} [label=\"{}: {}\"];\n  {sn} -> {wn};\n",
                w.index,
                esc_dot(&w.name)
            ));
            for p in panes_of(snap, &s.name, w.index) {
                let pn = nid("p_", &p.id);
                out.push_str(&format!(
                    "  {pn} [label=\"{} {}\"];\n  {wn} -> {pn};\n",
                    esc_dot(&p.id),
                    esc_dot(&p.command)
                ));
            }
        }
    }
    out.push_str("}\n");
    out
}

fn esc_mermaid(s: &str) -> String {
    // Mermaid node text in quotes; escape quotes as HTML entity.
    s.replace('"', "&quot;")
}

fn render_mermaid(snap: &Snapshot) -> String {
    let mut out = String::from("graph LR\n  server[\"ztmux server\"]\n");
    for (s, wins) in walk(snap) {
        let sn = nid("s_", &s.id);
        let mark = if s.attached { " *" } else { "" };
        out.push_str(&format!(
            "  {sn}[\"{}{}\"]\n  server --> {sn}\n",
            esc_mermaid(&s.name),
            mark
        ));
        for w in wins {
            let wn = nid("w_", &w.id);
            out.push_str(&format!(
                "  {wn}[\"{}: {}\"]\n  {sn} --> {wn}\n",
                w.index,
                esc_mermaid(&w.name)
            ));
            for p in panes_of(snap, &s.name, w.index) {
                let pn = nid("p_", &p.id);
                out.push_str(&format!(
                    "  {pn}[\"{} {}\"]\n  {wn} --> {pn}\n",
                    esc_mermaid(&p.id),
                    esc_mermaid(&p.command)
                ));
            }
        }
    }
    out
}

fn render_html(snap: &Snapshot) -> String {
    let diagram = render_mermaid(snap);
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
<title>ztmux graph</title>\n\
<script src=\"https://cdn.jsdelivr.net/npm/mermaid/dist/mermaid.min.js\"></script>\n\
<script>mermaid.initialize({{ startOnLoad: true }});</script>\n\
</head>\n<body>\n<pre class=\"mermaid\">\n{diagram}</pre>\n</body>\n</html>\n"
    )
}

#[cfg(test)]
mod tests {
    use super::super::tmux_query::{Pane, Session, Window};
    use super::*;

    fn snap() -> Snapshot {
        Snapshot {
            sessions: vec![Session {
                name: "work".into(),
                id: "$0".into(),
                attached: true,
                ..Default::default()
            }],
            windows: vec![Window {
                session: "work".into(),
                index: 1,
                name: "editor".into(),
                id: "@3".into(),
                ..Default::default()
            }],
            panes: vec![Pane {
                session: "work".into(),
                window: 1,
                id: "%7".into(),
                command: "nvim".into(),
                ..Default::default()
            }],
            clients: vec![],
            error: None,
        }
    }

    #[test]
    fn dot_has_nodes_and_edges() {
        let d = render_dot(&snap());
        assert!(d.starts_with("digraph ztmux"));
        assert!(d.contains("server -> s_"));
        assert!(d.contains("nvim"));
        assert!(d.contains("editor"));
    }

    #[test]
    fn mermaid_has_graph_header_and_edges() {
        let m = render_mermaid(&snap());
        assert!(m.starts_with("graph LR"));
        assert!(m.contains("server --> s_0")); // $0 -> s_0
        assert!(m.contains("--> w_3")); // @3 -> w_3
        assert!(m.contains("--> p_7")); // %7 -> p_7
    }

    #[test]
    fn html_embeds_mermaid() {
        let h = render_html(&snap());
        assert!(h.contains("<!doctype html>"));
        assert!(h.contains("class=\"mermaid\""));
        assert!(h.contains("graph LR"));
    }

    // nid drops the tmux sigils ($, @, %) and any punctuation, keeping the
    // prefix plus the alphanumerics, so ids stay unique and graph-safe.
    #[test]
    fn nid_strips_non_alphanumeric_sigils() {
        assert_eq!(nid("s_", "$0"), "s_0");
        assert_eq!(nid("w_", "@12"), "w_12");
        assert_eq!(nid("p_", "%7"), "p_7");
        assert_eq!(nid("x_", "a-b.c"), "x_abc");
    }

    // esc_dot backslash-escapes backslash first, then double-quote.
    #[test]
    fn esc_dot_escapes_backslash_and_quote() {
        assert_eq!(esc_dot(r#"a"b\c"#), r#"a\"b\\c"#);
    }

    // esc_mermaid replaces double-quotes with the &quot; HTML entity.
    #[test]
    fn esc_mermaid_escapes_quotes_as_entity() {
        assert_eq!(esc_mermaid(r#"a"b"#), "a&quot;b");
    }

    // An attached session's DOT node label ends with " *".
    #[test]
    fn dot_marks_attached_session_with_star() {
        let d = render_dot(&snap());
        assert!(d.contains("s_0 [label=\"work *\"]"), "dot was:\n{d}");
    }
}
