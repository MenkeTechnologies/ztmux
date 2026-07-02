//! `ztmux disk` — the filesystem usage behind every pane's working directory.
//!
//! For each live pane's cwd it runs `df` and reports the filesystem, how full it
//! is, and how much is free, fullest first — the "which pane is sitting on a disk
//! about to fill up" view. Where [`super::git`] reports the repo a pane is in and
//! [`super::dedup`] groups by directory, `disk` reports the *storage* under each
//! pane. Unique directories are resolved once and it degrades quietly (panes with
//! no directory, or when `df` fails, are omitted). Output is a table (coloured on
//! a TTY) or a JSON array with `-o json` / `--json`, sorted by capacity used then
//! location.

use std::collections::HashMap;
use std::io::IsTerminal;
use std::process::Command;

use super::tmux_query::{Pane, poll};

/// What `df` reports about the filesystem holding one directory.
#[derive(Clone)]
struct DiskInfo {
    filesystem: String,
    used_pct: String, // e.g. "88%"
    avail: String,    // human size, e.g. "200Gi"
    mount: String,
}

/// One output row: a pane and the storage under its working directory.
struct Row {
    id: String,
    location: String,
    path: String,
    filesystem: String,
    used_pct: String,
    avail: String,
    mount: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux disk: {e}");
        return 1;
    }
    let info = resolve_all(&snap.panes);
    let rows = build_rows(&snap.panes, &info);
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

fn location(p: &Pane) -> String {
    format!("{}:{}.{}", p.session, p.window, p.index)
}

/// Resolve each unique live-pane directory once, mapping it to its filesystem
/// usage (or `None` when `df` fails). The `None` is cached so repeated paths do
/// not re-spawn `df`.
fn resolve_all(panes: &[Pane]) -> HashMap<String, Option<DiskInfo>> {
    let mut map: HashMap<String, Option<DiskInfo>> = HashMap::new();
    for p in panes {
        if p.dead || p.path.is_empty() || map.contains_key(&p.path) {
            continue;
        }
        map.insert(p.path.clone(), resolve_df(&p.path));
    }
    map
}

/// One `df -Ph <path>` invocation, parsed. `-P` forces one line per filesystem
/// (POSIX), `-h` renders human sizes. `None` on any failure.
fn resolve_df(path: &str) -> Option<DiskInfo> {
    let out = Command::new("df").args(["-Ph", path]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_df(&String::from_utf8_lossy(&out.stdout))
}

/// Parse `df -Ph` output: skip the header, take the first data line, and pull the
/// POSIX columns (`Filesystem Size Used Avail Capacity Mounted-on`). The mount
/// point is the 6th field onward (it may contain spaces).
fn parse_df(text: &str) -> Option<DiskInfo> {
    let line = text.lines().skip(1).find(|l| !l.trim().is_empty())?;
    let f: Vec<&str> = line.split_whitespace().collect();
    if f.len() < 6 {
        return None;
    }
    Some(DiskInfo {
        filesystem: f[0].to_string(),
        avail: f[3].to_string(),
        used_pct: f[4].to_string(),
        mount: f[5..].join(" "),
    })
}

/// The numeric percentage in a `df` capacity field (`"88%"` → 88), for sorting.
/// Unparseable fields sort as 0.
fn pct(used: &str) -> u32 {
    used.trim_end_matches('%').parse().unwrap_or(0)
}

/// One row per live pane whose directory resolved, sorted fullest-first then by
/// location for a stable, greppable board.
fn build_rows(panes: &[Pane], info: &HashMap<String, Option<DiskInfo>>) -> Vec<Row> {
    let mut rows: Vec<Row> = panes
        .iter()
        .filter(|p| !p.dead)
        .filter_map(|p| {
            let d = info.get(&p.path).and_then(|o| o.as_ref())?;
            Some(Row {
                id: p.id.clone(),
                location: location(p),
                path: p.path.clone(),
                filesystem: d.filesystem.clone(),
                used_pct: d.used_pct.clone(),
                avail: d.avail.clone(),
                mount: d.mount.clone(),
            })
        })
        .collect();
    // Fullest first; ties by location.
    rows.sort_by(|a, b| {
        pct(&b.used_pct)
            .cmp(&pct(&a.used_pct))
            .then(a.location.cmp(&b.location))
    });
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
        paint(
            &format!(
                "{:<8} {:<16} {:>5} {:>8} {:<12} {}",
                "PANE", "LOCATION", "USE%", "AVAIL", "FS", "PATH"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<8} {:<16} {:>5} {:>8} {:<12} {}\n",
            r.id, r.location, r.used_pct, r.avail, r.filesystem, r.path,
        ));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "location": r.location,
                "path": r.path,
                "filesystem": r.filesystem,
                "used_pct": r.used_pct,
                "avail": r.avail,
                "mount": r.mount,
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

    fn pane(id: &str, sess: &str, win: i64, idx: i64, path: &str) -> Pane {
        Pane {
            session: sess.into(),
            window: win,
            index: idx,
            id: id.into(),
            command: "zsh".into(),
            path: path.into(),
            ..Default::default()
        }
    }

    fn info(
        pairs: &[(&str, Option<(&str, &str, &str, &str)>)],
    ) -> HashMap<String, Option<DiskInfo>> {
        pairs
            .iter()
            .map(|(path, d)| {
                (
                    (*path).to_string(),
                    d.map(|(fs, used, avail, mount)| DiskInfo {
                        filesystem: fs.to_string(),
                        used_pct: used.to_string(),
                        avail: avail.to_string(),
                        mount: mount.to_string(),
                    }),
                )
            })
            .collect()
    }

    #[test]
    fn parse_df_pulls_posix_columns() {
        let text = "Filesystem Size Used Avail Capacity Mounted on\n\
                    /dev/disk3s5 926Gi 10Gi 200Gi 88% /System/Volumes/Data\n";
        let d = parse_df(text).unwrap();
        assert_eq!(d.filesystem, "/dev/disk3s5");
        assert_eq!(d.avail, "200Gi");
        assert_eq!(d.used_pct, "88%");
        assert_eq!(d.mount, "/System/Volumes/Data");
    }

    #[test]
    fn parse_df_none_when_no_data_line() {
        assert!(parse_df("Filesystem Size Used Avail Capacity Mounted on\n").is_none());
    }

    #[test]
    fn pct_extracts_number() {
        assert_eq!(pct("88%"), 88);
        assert_eq!(pct("0%"), 0);
        assert_eq!(pct("weird"), 0);
    }

    #[test]
    fn fullest_filesystem_sorts_first() {
        let panes = vec![
            pane("%1", "a", 0, 0, "/low"),
            pane("%2", "a", 1, 0, "/high"),
        ];
        let map = info(&[
            ("/low", Some(("/dev/a", "12%", "800Gi", "/low"))),
            ("/high", Some(("/dev/b", "95%", "5Gi", "/high"))),
        ]);
        let rows = build_rows(&panes, &map);
        assert_eq!(rows[0].id, "%2");
        assert_eq!(rows[0].used_pct, "95%");
        assert_eq!(rows[1].id, "%1");
    }

    #[test]
    fn dead_pathless_and_unresolved_panes_drop_out() {
        let mut dead = pane("%2", "a", 1, 0, "/x");
        dead.dead = true;
        let panes = vec![
            pane("%1", "a", 0, 0, "/x"),
            dead,
            pane("%3", "a", 2, 0, ""),      // no path
            pane("%4", "a", 3, 0, "/gone"), // resolved to None
        ];
        let map = info(&[
            ("/x", Some(("/dev/a", "50%", "100Gi", "/x"))),
            ("/gone", None),
        ]);
        let rows = build_rows(&panes, &map);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "%1");
    }

    #[test]
    fn text_has_header_and_usage() {
        let panes = vec![pane("%1", "a", 0, 0, "/x")];
        let map = info(&[("/x", Some(("/dev/a", "77%", "40Gi", "/x")))]);
        let s = render_text(&build_rows(&panes, &map), false);
        assert!(s.contains("USE%") && s.contains("AVAIL") && s.contains("FS"));
        assert!(s.lines().any(|l| l.contains("77%") && l.contains("/dev/a")));
    }

    #[test]
    fn json_carries_usage_fields() {
        let panes = vec![pane("%1", "a", 0, 0, "/x")];
        let map = info(&[("/x", Some(("/dev/a", "77%", "40Gi", "/mnt/x")))]);
        let v: serde_json::Value =
            serde_json::from_str(&render_json(&build_rows(&panes, &map))).unwrap();
        assert_eq!(v[0]["used_pct"], "77%");
        assert_eq!(v[0]["avail"], "40Gi");
        assert_eq!(v[0]["filesystem"], "/dev/a");
        assert_eq!(v[0]["mount"], "/mnt/x");
    }
}
