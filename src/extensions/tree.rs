//! `ztmux tree` — print the whole server as an ASCII session→window→pane tree.
//!
//! Unlike the interactive [`super::dashboard`], this is a one-shot, pipeable
//! dump to stdout (coloured when stdout is a TTY, plain otherwise), built from
//! the same `list-* -o json` query layer ([`super::tmux_query`]).

use std::io::IsTerminal;

use super::tmux_query::{Pane, Snapshot, Window, poll};

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux tree: {e}");
        return 1;
    }
    print!("{}", render(&snap, std::io::stdout().is_terminal()));
    0
}

/// Render the tree. `color` toggles ANSI escapes so the pure text form stays
/// testable and pipes cleanly.
fn render(snap: &Snapshot, color: bool) -> String {
    let paint = |s: &str, code: &str| -> String {
        if color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };

    let mut out = String::new();
    for s in &snap.sessions {
        let marker = if s.attached {
            paint(" *", "35")
        } else {
            String::new()
        };
        out.push_str(&format!(
            "{} {} {}{}\n",
            paint("●", "36"),
            paint(&s.name, "1;36"),
            paint(&format!("({} win)", s.windows), "2"),
            marker,
        ));

        let mut wins: Vec<&Window> = snap
            .windows
            .iter()
            .filter(|w| w.session == s.name)
            .collect();
        wins.sort_by_key(|w| w.index);
        for (wi, w) in wins.iter().enumerate() {
            let last_win = wi + 1 == wins.len();
            let wbr = if last_win { "└─" } else { "├─" };
            let act = if w.active {
                paint(" ●", "32")
            } else {
                String::new()
            };
            out.push_str(&format!(
                "{} {}: {} {}{}\n",
                wbr,
                w.index,
                paint(&w.name, "32"),
                paint(&format!("[{} panes]", w.panes), "2"),
                act,
            ));

            let mut panes: Vec<&Pane> = snap
                .panes
                .iter()
                .filter(|p| p.session == w.session && p.window == w.index)
                .collect();
            panes.sort_by_key(|p| p.index);
            let cont = if last_win { "   " } else { "│  " };
            for (pi, p) in panes.iter().enumerate() {
                let last_pane = pi + 1 == panes.len();
                let pbr = if last_pane { "└─" } else { "├─" };
                let flag = if p.active {
                    paint(" ●", "32")
                } else if p.dead {
                    paint(" ✝", "31")
                } else {
                    String::new()
                };
                out.push_str(&format!(
                    "{}{} {} {}{}\n",
                    cont,
                    pbr,
                    p.id,
                    paint(&p.command, "33"),
                    flag,
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::tmux_query::{Pane, Session, Window};
    use super::*;

    fn snap() -> Snapshot {
        Snapshot {
            sessions: vec![
                Session {
                    name: "work".into(),
                    windows: 2,
                    attached: true,
                    ..Default::default()
                },
                Session {
                    name: "scratch".into(),
                    windows: 1,
                    ..Default::default()
                },
            ],
            windows: vec![
                Window {
                    session: "work".into(),
                    index: 0,
                    name: "zsh".into(),
                    panes: 2,
                    ..Default::default()
                },
                Window {
                    session: "work".into(),
                    index: 1,
                    name: "editor".into(),
                    active: true,
                    panes: 1,
                    ..Default::default()
                },
                Window {
                    session: "scratch".into(),
                    index: 0,
                    name: "zsh".into(),
                    panes: 1,
                    ..Default::default()
                },
            ],
            panes: vec![
                Pane {
                    session: "work".into(),
                    window: 0,
                    index: 0,
                    id: "%0".into(),
                    command: "zsh".into(),
                    ..Default::default()
                },
                Pane {
                    session: "work".into(),
                    window: 0,
                    index: 1,
                    id: "%1".into(),
                    command: "vim".into(),
                    ..Default::default()
                },
                Pane {
                    session: "work".into(),
                    window: 1,
                    index: 0,
                    id: "%2".into(),
                    command: "nvim".into(),
                    active: true,
                    ..Default::default()
                },
                Pane {
                    session: "scratch".into(),
                    window: 0,
                    index: 0,
                    id: "%3".into(),
                    command: "htop".into(),
                    ..Default::default()
                },
            ],
            clients: vec![],
            error: None,
        }
    }

    #[test]
    fn renders_a_plain_box_drawing_tree() {
        assert_eq!(
            render(&snap(), false),
            "\
● work (2 win) *
├─ 0: zsh [2 panes]
│  ├─ %0 zsh
│  └─ %1 vim
└─ 1: editor [1 panes] ●
   └─ %2 nvim ●
● scratch (1 win)
└─ 0: zsh [1 panes]
   └─ %3 htop
"
        );
    }

    #[test]
    fn color_wraps_in_ansi_escapes() {
        let c = render(&snap(), true);
        assert!(c.contains("\x1b["));
        assert!(c.contains("\x1b[0m"));
        // The plain and coloured forms carry the same visible tokens.
        assert!(c.contains("work") && c.contains("nvim") && c.contains("%3"));
    }

    // A dead (non-active) pane renders with the ✝ marker.
    #[test]
    fn dead_pane_gets_a_cross_marker() {
        let mut sn = snap();
        sn.panes.push(Pane {
            session: "scratch".into(),
            window: 0,
            index: 1,
            id: "%9".into(),
            command: "gdb".into(),
            dead: true,
            ..Default::default()
        });
        let out = render(&sn, false);
        assert!(out.contains("%9 gdb ✝"), "output was:\n{out}");
    }

    // Windows render in index order even when the snapshot lists them reversed.
    #[test]
    fn windows_render_sorted_by_index() {
        let mut sn = snap();
        sn.windows.reverse();
        let out = render(&sn, false);
        let zero = out.find("0: zsh").unwrap();
        let one = out.find("1: editor").unwrap();
        assert!(zero < one, "window 0 must render before window 1");
    }
}
