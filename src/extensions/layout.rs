//! `ztmux layout` — apply a named layout preset to a window.
//!
//! A client subcommand that expands a preset name into a sequence of ztmux
//! commands (`split-window` / `select-pane` / `select-layout`) applied to the
//! target window. It covers tmux's five built-in layouts plus a few composite
//! presets (dev, ide, grid) that create panes. It is **dry-run by default**
//! (prints the commands); pass `-f`/`--apply` to run them.
//!
//! Usage: `ztmux layout <name> [-t target-window] [-f]` · `ztmux layout list`.

use super::tmux_query::ztmux_cmd;

/// Every preset name with a one-line description.
const PRESETS: &[(&str, &str)] = &[
    ("even-h", "built-in even-horizontal"),
    ("even-v", "built-in even-vertical"),
    ("main-h", "built-in main-horizontal"),
    ("main-v", "built-in main-vertical"),
    ("tiled", "built-in tiled"),
    ("dev", "editor (65%) + side terminal (35%)"),
    ("ide", "editor + two stacked side panes"),
    ("grid", "four tiled panes"),
];

pub(crate) fn run(socket: &str) -> i32 {
    let args: Vec<String> = std::env::args().collect();
    let name = args.iter().skip_while(|a| a.as_str() != "layout").nth(1);
    let name = match name {
        Some(n) if !n.starts_with('-') => n.as_str(),
        _ => {
            eprintln!("usage: ztmux layout <name> [-t window] [-f] | ztmux layout list");
            return 2;
        }
    };
    if name == "list" {
        for (n, d) in PRESETS {
            println!("{n:<8} {d}");
        }
        return 0;
    }

    let target = args
        .iter()
        .position(|a| a == "-t")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let apply = args.iter().any(|a| a == "-f" || a == "--apply");

    let Some(cmds) = preset_cmds(name, target.as_deref()) else {
        eprintln!("ztmux layout: unknown preset '{name}' (try `ztmux layout list`)");
        return 1;
    };

    if !apply {
        println!("# would apply preset '{name}' ({} commands):", cmds.len());
        for c in &cmds {
            println!("ztmux {}", c.join(" "));
        }
        println!("# re-run with -f/--apply to run them");
        return 0;
    }

    for c in &cmds {
        let argv: Vec<&str> = c.iter().map(String::as_str).collect();
        match ztmux_cmd(socket, &argv).output() {
            Ok(o) if o.status.success() => {}
            Ok(o) => {
                eprintln!(
                    "ztmux layout: `{}` failed: {}",
                    c.join(" "),
                    String::from_utf8_lossy(&o.stderr).trim()
                );
                return 1;
            }
            Err(e) => {
                eprintln!("ztmux layout: `{}` failed: {e}", c.join(" "));
                return 1;
            }
        }
    }
    0
}

/// Expand a preset into command argv vectors. `target` (a window target) is
/// appended as `-t <target>` to each command when given. Returns None for an
/// unknown preset.
fn preset_cmds(name: &str, target: Option<&str>) -> Option<Vec<Vec<String>>> {
    let s = |parts: &[&str]| {
        parts
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<String>>()
    };
    let builtin = |layout: &str| vec![s(&["select-layout", layout])];

    let base: Vec<Vec<String>> = match name {
        "even-h" => builtin("even-horizontal"),
        "even-v" => builtin("even-vertical"),
        "main-h" => builtin("main-horizontal"),
        "main-v" => builtin("main-vertical"),
        "tiled" => builtin("tiled"),
        "dev" => vec![s(&["split-window", "-h", "-l", "35%"])],
        "ide" => vec![
            s(&["split-window", "-h", "-l", "35%"]),
            s(&["split-window", "-v", "-l", "50%"]),
            s(&["select-pane", "-L"]),
        ],
        "grid" => vec![
            s(&["split-window", "-h"]),
            s(&["split-window", "-v"]),
            s(&["select-pane", "-L"]),
            s(&["split-window", "-v"]),
            s(&["select-layout", "tiled"]),
        ],
        _ => return None,
    };

    // Thread the target window through as `-t <target>` on each command.
    let cmds = match target {
        None => base,
        Some(t) => base
            .into_iter()
            .map(|mut c| {
                c.push("-t".to_string());
                c.push(t.to_string());
                c
            })
            .collect(),
    };
    Some(cmds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_preset_is_a_single_select_layout() {
        let cmds = preset_cmds("main-v", None).unwrap();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0], vec!["select-layout", "main-vertical"]);
    }

    #[test]
    fn composite_preset_creates_panes() {
        let cmds = preset_cmds("ide", None).unwrap();
        assert_eq!(cmds.len(), 3);
        assert_eq!(cmds[0][0], "split-window");
        assert!(cmds.iter().any(|c| c[0] == "select-pane"));
    }

    #[test]
    fn target_is_threaded_onto_every_command() {
        let cmds = preset_cmds("dev", Some("work:1")).unwrap();
        assert!(cmds.iter().all(|c| c.contains(&"-t".to_string())));
        assert!(cmds[0].ends_with(&["-t".to_string(), "work:1".to_string()]));
    }

    #[test]
    fn unknown_preset_is_none() {
        assert!(preset_cmds("nope", None).is_none());
    }

    // Every built-in alias maps to exactly one select-layout with the tmux name.
    #[test]
    fn all_builtins_map_to_select_layout() {
        for (name, layout) in [
            ("even-h", "even-horizontal"),
            ("even-v", "even-vertical"),
            ("main-h", "main-horizontal"),
            ("main-v", "main-vertical"),
            ("tiled", "tiled"),
        ] {
            let cmds = preset_cmds(name, None).unwrap();
            assert_eq!(
                cmds,
                vec![vec!["select-layout".to_string(), layout.to_string()]]
            );
        }
    }

    // grid: three splits create four panes, closed by a tiled select-layout.
    #[test]
    fn grid_preset_tiles_four_panes() {
        let cmds = preset_cmds("grid", None).unwrap();
        assert_eq!(cmds.len(), 5);
        assert_eq!(cmds.iter().filter(|c| c[0] == "split-window").count(), 3);
        assert_eq!(*cmds.last().unwrap(), vec!["select-layout", "tiled"]);
    }

    // dev is a single horizontal split sizing the side pane to 35%.
    #[test]
    fn dev_preset_is_a_single_horizontal_split() {
        let cmds = preset_cmds("dev", None).unwrap();
        assert_eq!(cmds, vec![vec!["split-window", "-h", "-l", "35%"]]);
    }

    // Every advertised preset name actually expands (list ↔ preset_cmds parity).
    #[test]
    fn every_listed_preset_expands() {
        for (name, _) in PRESETS {
            assert!(preset_cmds(name, None).is_some(), "{name} should expand");
        }
    }
}
