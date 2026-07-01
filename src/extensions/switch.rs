//! `ztmux switch` — an interactive fuzzy picker to jump to any session, window,
//! or pane. Type to filter; Enter runs the matching `switch-client` /
//! `select-window` / `select-pane` against the selected socket; Esc cancels.
//!
//! Like the dashboard it is a client subcommand that reads the server via
//! `list-* -o json` (see [`super::tmux_query`]); it is not a server command.
//! Mouse: wheel scrolls, left-click selects an item, right-click chooses it.

use ratatui::Frame;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use super::tmux_query::{Snapshot, poll, ztmux_cmd};

/// One jump target: a display label and the ordered commands that select it.
struct Target {
    kind: &'static str,
    label: String,
    cmds: Vec<Vec<String>>,
}

/// Flatten a snapshot into pickable targets: every session, window, and pane.
fn build_targets(snap: &Snapshot) -> Vec<Target> {
    let mut out = Vec::new();
    for s in &snap.sessions {
        out.push(Target {
            kind: "session",
            label: s.name.clone(),
            cmds: vec![vec!["switch-client".into(), "-t".into(), s.name.clone()]],
        });
    }
    for w in &snap.windows {
        let win = format!("{}:{}", w.session, w.index);
        out.push(Target {
            kind: "window",
            label: format!("{win} {}", w.name),
            cmds: vec![
                vec!["switch-client".into(), "-t".into(), w.session.clone()],
                vec!["select-window".into(), "-t".into(), win],
            ],
        });
    }
    for p in &snap.panes {
        let win = format!("{}:{}", p.session, p.window);
        out.push(Target {
            kind: "pane",
            label: format!("{}.{} {} {}", win, p.index, p.id, p.command),
            cmds: vec![
                vec!["switch-client".into(), "-t".into(), p.session.clone()],
                vec!["select-window".into(), "-t".into(), win],
                vec!["select-pane".into(), "-t".into(), p.id.clone()],
            ],
        });
    }
    out
}

/// Case-insensitive subsequence ("fuzzy") match.
fn fuzzy(hay: &str, needle: &str) -> bool {
    let h: Vec<char> = hay.to_lowercase().chars().collect();
    let mut hi = 0usize;
    for nc in needle.to_lowercase().chars() {
        loop {
            if hi >= h.len() {
                return false;
            }
            let matched = h[hi] == nc;
            hi += 1;
            if matched {
                break;
            }
        }
    }
    true
}

/// Indices of targets whose "kind label" matches the query.
fn filter(targets: &[Target], query: &str) -> Vec<usize> {
    targets
        .iter()
        .enumerate()
        .filter(|(_, t)| query.is_empty() || fuzzy(&format!("{} {}", t.kind, t.label), query))
        .map(|(i, _)| i)
        .collect()
}

struct Picker {
    socket: String,
    targets: Vec<Target>,
    filtered: Vec<usize>,
    query: String,
    list: ListState,
    list_area: Rect, // last rendered rect of the results list (for mouse hit-testing)
    error: Option<String>,
    status: String,
    quit: bool,
}

impl Picker {
    fn new(socket: String) -> Self {
        let snap = poll(&socket);
        let error = snap.error.clone();
        let targets = build_targets(&snap);
        let filtered = filter(&targets, "");
        let mut list = ListState::default();
        if !filtered.is_empty() {
            list.select(Some(0));
        }
        Picker {
            socket,
            targets,
            filtered,
            query: String::new(),
            list,
            list_area: Rect::default(),
            error,
            status: String::new(),
            quit: false,
        }
    }

    fn refilter(&mut self) {
        self.filtered = filter(&self.targets, &self.query);
        let sel = if self.filtered.is_empty() {
            None
        } else {
            Some(
                self.list
                    .selected()
                    .unwrap_or(0)
                    .min(self.filtered.len() - 1),
            )
        };
        self.list.select(sel);
    }

    fn move_sel(&mut self, delta: isize) {
        if self.filtered.is_empty() {
            return;
        }
        let n = self.filtered.len() as isize;
        let cur = self.list.selected().unwrap_or(0) as isize;
        self.list.select(Some((cur + delta).rem_euclid(n) as usize));
    }

    /// Run the selected target's commands, then quit.
    fn choose(&mut self) {
        let Some(&ti) = self.list.selected().and_then(|i| self.filtered.get(i)) else {
            return;
        };
        for cmd in &self.targets[ti].cmds {
            let args: Vec<&str> = cmd.iter().map(String::as_str).collect();
            if let Ok(o) = ztmux_cmd(&self.socket, &args).output()
                && !o.status.success()
            {
                self.status = String::from_utf8_lossy(&o.stderr).trim().to_string();
            }
        }
        self.quit = true;
    }
}

pub(crate) fn run(socket: &str) -> i32 {
    let mut terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableMouseCapture);
    let mut app = Picker::new(socket.to_string());
    let result = (|| -> std::io::Result<()> {
        while !app.quit {
            terminal.draw(|f| ui(f, &mut app))?;
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => {
                    if k.modifiers.contains(KeyModifiers::CONTROL)
                        && matches!(k.code, KeyCode::Char('c'))
                    {
                        app.quit = true;
                        continue;
                    }
                    match k.code {
                        KeyCode::Esc => app.quit = true,
                        KeyCode::Enter => app.choose(),
                        KeyCode::Down => app.move_sel(1),
                        KeyCode::Up => app.move_sel(-1),
                        KeyCode::Backspace => {
                            app.query.pop();
                            app.refilter();
                        }
                        KeyCode::Char(c) => {
                            app.query.push(c);
                            app.refilter();
                        }
                        _ => {}
                    }
                }
                Event::Mouse(m) => handle_mouse(&mut app, m),
                _ => {}
            }
        }
        Ok(())
    })();
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("switch error: {e}");
            1
        }
    }
}

/// Map a mouse position to a filtered-list index, if it lands inside the list.
fn row_at(app: &Picker, m: MouseEvent) -> Option<usize> {
    let a = app.list_area;
    let inside =
        m.column >= a.x && m.column < a.x + a.width && m.row > a.y && m.row + 1 < a.y + a.height;
    if !inside {
        return None;
    }
    let idx = (m.row - a.y - 1) as usize + app.list.offset();
    (idx < app.filtered.len()).then_some(idx)
}

/// Mouse: wheel scrolls, left-click selects an item, right-click chooses it.
fn handle_mouse(app: &mut Picker, m: MouseEvent) {
    match m.kind {
        MouseEventKind::ScrollDown => app.move_sel(1),
        MouseEventKind::ScrollUp => app.move_sel(-1),
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some(idx) = row_at(app, m) {
                app.list.select(Some(idx));
            }
        }
        MouseEventKind::Down(MouseButton::Right) => {
            if let Some(idx) = row_at(app, m) {
                app.list.select(Some(idx));
                app.choose();
            }
        }
        _ => {}
    }
}

fn ui(f: &mut Frame, app: &mut Picker) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // query box
            Constraint::Min(3),    // results
            Constraint::Length(1), // help
        ])
        .split(f.area());

    let title = if let Some(e) = &app.error {
        format!(" no server: {e} ")
    } else {
        format!(" {} matches ", app.filtered.len())
    };
    let query = Paragraph::new(Line::from(vec![
        Span::styled("❯ ", Style::default().fg(Color::Cyan)),
        Span::raw(app.query.clone()),
        Span::styled("▏", Style::default().fg(Color::DarkGray)),
    ]))
    .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(query, rows[0]);

    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .map(|&i| {
            let t = &app.targets[i];
            let (tag_color, w) = match t.kind {
                "session" => (Color::Cyan, 8),
                "window" => (Color::Green, 8),
                _ => (Color::Yellow, 8),
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:<w$}", t.kind), Style::default().fg(tag_color)),
                Span::raw(t.label.clone()),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" switch to "))
        .highlight_style(
            Style::default()
                .bg(Color::Indexed(237))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    app.list_area = rows[1]; // remember for mouse hit-testing
    f.render_stateful_widget(list, rows[1], &mut app.list);

    let help = if app.status.is_empty() {
        " type to filter · ↑/↓ move · enter/right-click switch · esc cancel".to_string()
    } else {
        format!(" {}", app.status)
    };
    f.render_widget(
        Paragraph::new(Span::styled(help, Style::default().fg(Color::DarkGray))),
        rows[2],
    );
}

#[cfg(test)]
mod tests {
    use super::super::tmux_query::{Pane, Session, Window};
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn snap() -> Snapshot {
        Snapshot {
            sessions: vec![
                Session {
                    name: "work".into(),
                    ..Default::default()
                },
                Session {
                    name: "scratch".into(),
                    ..Default::default()
                },
            ],
            windows: vec![Window {
                session: "work".into(),
                index: 1,
                name: "editor".into(),
                ..Default::default()
            }],
            panes: vec![Pane {
                session: "work".into(),
                window: 1,
                index: 0,
                id: "%2".into(),
                command: "nvim".into(),
                ..Default::default()
            }],
            clients: vec![],
            error: None,
        }
    }

    #[test]
    fn targets_cover_sessions_windows_and_panes() {
        let t = build_targets(&snap());
        assert_eq!(t.len(), 4); // 2 sessions + 1 window + 1 pane
        // a pane target selects session -> window -> pane, in order.
        let pane = t.iter().find(|t| t.kind == "pane").unwrap();
        assert_eq!(pane.cmds.len(), 3);
        assert_eq!(pane.cmds[0], vec!["switch-client", "-t", "work"]);
        assert_eq!(pane.cmds[1], vec!["select-window", "-t", "work:1"]);
        assert_eq!(pane.cmds[2], vec!["select-pane", "-t", "%2"]);
    }

    #[test]
    fn fuzzy_filter_matches_subsequence_case_insensitively() {
        let t = build_targets(&snap());
        // "nvim" only matches the pane target.
        let f = filter(&t, "nvim");
        assert_eq!(f.len(), 1);
        assert_eq!(t[f[0]].kind, "pane");
        // "wrk" is a subsequence of "work" (session + window + pane labels).
        assert!(filter(&t, "wrk").len() >= 3);
        // empty query matches everything.
        assert_eq!(filter(&t, "").len(), 4);
        // no match.
        assert!(filter(&t, "zzz").is_empty());
    }

    #[test]
    fn row_at_hit_tests_the_results_list() {
        use ratatui::crossterm::event::KeyModifiers;
        let mut app = Picker::new(String::new());
        app.targets = build_targets(&snap());
        app.error = None;
        app.refilter(); // 4 filtered targets
        app.list_area = Rect::new(0, 3, 80, 10); // results block at y=3
        let at = |row| MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: 5,
            row,
            modifiers: KeyModifiers::empty(),
        };
        // First item one row below the top border (y=3) → row 4 = idx 0.
        assert_eq!(row_at(&app, at(4)), Some(0));
        assert_eq!(row_at(&app, at(7)), Some(3));
        assert_eq!(row_at(&app, at(3)), None); // border row
        assert_eq!(row_at(&app, at(8)), None); // only 4 items
    }

    #[test]
    fn renders_query_and_results() {
        let mut app = Picker::new(String::new());
        // Picker::new polled a (likely absent) server; inject deterministic data.
        app.targets = build_targets(&snap());
        app.error = None;
        app.query = "edit".into();
        app.refilter();
        let mut term = Terminal::new(TestBackend::new(80, 20)).unwrap();
        term.draw(|f| ui(f, &mut app)).unwrap();
        let s: String = term
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(s.contains("switch to"));
        assert!(s.contains("editor")); // the only match for "edit"
        assert!(!s.contains("scratch")); // filtered out
    }

    // fuzzy is a case-insensitive, in-order subsequence match; empty needle
    // always matches, out-of-order / too-long needles do not.
    #[test]
    fn fuzzy_subsequence_semantics() {
        assert!(fuzzy("nvim", "nvim"));
        assert!(fuzzy("neovim", "nvm")); // subsequence
        assert!(fuzzy("Work", "wk")); // case-insensitive
        assert!(!fuzzy("abc", "cab")); // wrong order
        assert!(fuzzy("anything", "")); // empty needle
        assert!(!fuzzy("ab", "abc")); // needle longer than remaining
    }

    // A window target switches the client to the session, then selects the
    // window; its label is "session:index name".
    #[test]
    fn window_target_switches_client_then_window() {
        let t = build_targets(&snap());
        let w = t.iter().find(|t| t.kind == "window").unwrap();
        assert_eq!(w.cmds.len(), 2);
        assert_eq!(w.cmds[0], vec!["switch-client", "-t", "work"]);
        assert_eq!(w.cmds[1], vec!["select-window", "-t", "work:1"]);
        assert_eq!(w.label, "work:1 editor");
    }
}
