//! `ztmux keys` — how many key bindings live in each key table.
//!
//! tmux groups key bindings into tables — `root` (no prefix), `prefix` (after
//! the prefix key), `copy-mode`/`copy-mode-vi` (inside copy mode), and any custom
//! tables you create with `bind-key -T`. A raw `list-keys` dumps hundreds of
//! bindings at once; `keys` collapses that into a summary: one row per table with
//! its binding count, busiest first. It surfaces the shape of a configuration —
//! how heavily each table is customised, and which custom tables exist at all —
//! without scrolling the whole list. With `-o json` / `--json` it emits the same
//! rows as a machine-readable array.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// One output row: a key table and how many bindings it holds.
struct Row {
    table: String,
    bindings: usize,
}

pub(crate) fn run(socket: &str) -> i32 {
    let lines = query_lines(socket, &["list-keys"]);
    let rows = build_rows(&lines);
    let json = std::env::args().any(|a| a == "--json")
        || std::env::args()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|w| w[0] == "-o" && w[1] == "json");
    if json {
        print!("{}", render_json(&rows));
    } else {
        print!("{}", render_text(&rows, std::io::stdout().is_terminal()));
    }
    0
}

/// Extract a binding's key table from one `list-keys` line: the token
/// immediately after the *first* `-T`. Only the first `-T` names the binding's
/// table; a later `-T` inside the bound command (e.g. a status format) is
/// ignored.
fn table_of(line: &str) -> Option<String> {
    let toks: Vec<&str> = line.split_whitespace().collect();
    let i = toks.iter().position(|t| *t == "-T")?;
    toks.get(i + 1).map(|t| t.to_string())
}

/// Count bindings per table, busiest table first; ties break by table name.
fn build_rows(lines: &[String]) -> Vec<Row> {
    use std::collections::BTreeMap;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for line in lines {
        if let Some(table) = table_of(line) {
            *counts.entry(table).or_default() += 1;
        }
    }
    let mut rows: Vec<Row> = counts
        .into_iter()
        .map(|(table, bindings)| Row { table, bindings })
        .collect();
    rows.sort_by(|a, b| b.bindings.cmp(&a.bindings).then(a.table.cmp(&b.table)));
    rows
}

fn render_text(rows: &[Row], color: bool) -> String {
    let paint = |s: &str, code: &str| -> String {
        if color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };
    let mut out = String::new();
    out.push_str(&format!(
        "{}\n",
        paint(&format!("{:>8} {}", "BINDINGS", "TABLE"), "1")
    ));
    for r in rows {
        out.push_str(&format!("{:>8} {}\n", r.bindings, r.table));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "table": r.table,
                "bindings": r.bindings,
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

    #[test]
    fn table_is_taken_from_the_first_dash_t() {
        assert_eq!(
            table_of("bind-key -T prefix c new-window"),
            Some("prefix".to_string())
        );
        // A -T inside the bound command must not override the binding's table.
        assert_eq!(
            table_of("bind-key -T root M-x run-shell 'x -T inner'"),
            Some("root".to_string())
        );
        assert_eq!(table_of("not a binding line"), None);
    }

    #[test]
    fn counts_bindings_per_table_busiest_first() {
        let lines = vec![
            "bind-key -T prefix c new-window".to_string(),
            "bind-key -T prefix d detach-client".to_string(),
            "bind-key -T root MouseDown1Pane select-pane".to_string(),
            "bind-key -T mytable x display-message hi".to_string(),
        ];
        let rows = build_rows(&lines);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].table, "prefix");
        assert_eq!(rows[0].bindings, 2);
        // root and mytable each have 1; tie breaks by table name.
        assert_eq!(rows[1].table, "mytable");
        assert_eq!(rows[2].table, "root");
    }

    #[test]
    fn no_bindings_renders_header_only() {
        let rows = build_rows(&[]);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("BINDINGS") && s.contains("TABLE"));
    }

    #[test]
    fn json_carries_table_and_count() {
        let lines = vec![
            "bind-key -T prefix c new-window".to_string(),
            "bind-key -T prefix d detach-client".to_string(),
        ];
        let rows = build_rows(&lines);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["table"], "prefix");
        assert_eq!(v[0]["bindings"], 2);
    }
}
