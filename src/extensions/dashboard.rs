//! `ztmux dashboard` — a live ratatui dashboard for the running ztmux server.
//!
//! It runs as a normal ztmux client subcommand: it re-invokes our own binary
//! (`ztmux -S <socket> list-* -o json`) and renders the machine-readable output
//! added by the `-o` flag, so it needs no linkage against the server internals
//! and always targets the same socket the user selected.
//!
//! Keys: j/k or ↑/↓ move · g/G top/bottom · Enter focus (attach) · r refresh ·
//!       x kill (confirm) · ? help · q/Esc quit. The tree descends
//!       session→window→pane and the right column carries a live
//!       `capture-pane` preview of the selected pane.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
    MouseEvent, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Sparkline, Table, Wrap,
};

use super::tmux_query::{Client, Pane, Snapshot, poll, ztmux_cmd};

// ─── flattened session→window tree for the navigable left pane ───────────────

enum TreeRow {
    Session(usize), // index into snap.sessions
    Window(usize),  // window index into snap.windows
    Pane(usize),    // pane index into snap.panes
}

struct App {
    snap: Snapshot,
    socket: String,
    rows: Vec<TreeRow>,
    list: ListState,
    history: VecDeque<u64>, // total pane count over time (for the sparkline)
    last_refresh: Instant,
    refresh_every: Duration,
    confirm_kill: bool,
    show_help: bool,
    preview: Vec<String>, // live capture-pane of the current target pane
    preview_title: String,
    tree_area: Rect, // last rendered rect of the tree list (for mouse hit-testing)
    status: String,
    quit: bool,
}

impl App {
    fn new(socket: String) -> Self {
        let mut app = App {
            snap: Snapshot::default(),
            socket,
            rows: Vec::new(),
            list: ListState::default(),
            history: VecDeque::new(),
            last_refresh: Instant::now(),
            refresh_every: Duration::from_millis(1000),
            confirm_kill: false,
            show_help: false,
            preview: Vec::new(),
            preview_title: String::new(),
            tree_area: Rect::default(),
            status: String::from("connected"),
            quit: false,
        };
        app.refresh();
        app.list.select(Some(0));
        app.update_preview();
        app
    }

    fn refresh(&mut self) {
        self.snap = poll(&self.socket);
        self.rebuild_rows();
        let total = self.snap.panes.len() as u64;
        self.history.push_back(total);
        while self.history.len() > 120 {
            self.history.pop_front();
        }
        self.last_refresh = Instant::now();
        // Keep the selection in range after a refresh.
        if self.rows.is_empty() {
            self.list.select(None);
        } else {
            let sel = self.list.selected().unwrap_or(0).min(self.rows.len() - 1);
            self.list.select(Some(sel));
        }
        self.update_preview();
    }

    fn rebuild_rows(&mut self) {
        self.rows.clear();
        for (si, s) in self.snap.sessions.iter().enumerate() {
            self.rows.push(TreeRow::Session(si));
            let mut wins: Vec<usize> = self
                .snap
                .windows
                .iter()
                .enumerate()
                .filter(|(_, w)| w.session == s.name)
                .map(|(i, _)| i)
                .collect();
            wins.sort_by_key(|&i| self.snap.windows[i].index);
            for wi in wins {
                self.rows.push(TreeRow::Window(wi));
                let w = &self.snap.windows[wi];
                let mut panes: Vec<usize> = self
                    .snap
                    .panes
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p.session == w.session && p.window == w.index)
                    .map(|(i, _)| i)
                    .collect();
                panes.sort_by_key(|&i| self.snap.panes[i].index);
                for pi in panes {
                    self.rows.push(TreeRow::Pane(pi));
                }
            }
        }
    }

    fn move_sel(&mut self, delta: isize) {
        if self.rows.is_empty() {
            return;
        }
        let n = self.rows.len() as isize;
        let cur = self.list.selected().unwrap_or(0) as isize;
        let next = (cur + delta).rem_euclid(n);
        self.list.select(Some(next as usize));
    }

    fn select_edge(&mut self, top: bool) {
        if self.rows.is_empty() {
            return;
        }
        self.list
            .select(Some(if top { 0 } else { self.rows.len() - 1 }));
    }

    /// Kill the currently selected session or window via the ztmux CLI.
    fn kill_selected(&mut self) {
        let Some(sel) = self.list.selected() else {
            return;
        };
        let (cmd, target, label) = match self.rows.get(sel) {
            Some(TreeRow::Session(si)) => {
                let s = &self.snap.sessions[*si];
                (
                    "kill-session",
                    s.name.clone(),
                    format!("session {}", s.name),
                )
            }
            Some(TreeRow::Window(wi)) => {
                let w = &self.snap.windows[*wi];
                (
                    "kill-window",
                    format!("{}:{}", w.session, w.index),
                    format!("window {}:{}", w.session, w.index),
                )
            }
            Some(TreeRow::Pane(pi)) => {
                let p = &self.snap.panes[*pi];
                ("kill-pane", p.id.clone(), format!("pane {}", p.id))
            }
            None => return,
        };
        let out = ztmux_cmd(&self.socket, &[cmd, "-t", &target]).output();
        self.status = match out {
            Ok(o) if o.status.success() => format!("killed {label}"),
            Ok(o) => format!("kill failed: {}", String::from_utf8_lossy(&o.stderr).trim()),
            Err(e) => format!("kill failed: {e}"),
        };
        self.refresh();
    }

    /// The pane id whose contents the preview should show for the current
    /// selection: the pane itself, or the active pane of the selected window /
    /// session (falling back to the first matching pane).
    fn current_pane_target(&self) -> Option<String> {
        let sel = self.list.selected()?;
        match self.rows.get(sel)? {
            TreeRow::Pane(pi) => Some(self.snap.panes[*pi].id.clone()),
            TreeRow::Window(wi) => {
                let w = &self.snap.windows[*wi];
                self.active_pane_of(&w.session, Some(w.index))
            }
            TreeRow::Session(si) => {
                let name = self.snap.sessions[*si].name.clone();
                self.active_pane_of(&name, None)
            }
        }
    }

    /// Pick the active pane (or first) in a session, optionally restricted to a
    /// window index.
    fn active_pane_of(&self, session: &str, window: Option<i64>) -> Option<String> {
        let matches = |p: &&Pane| p.session == session && window.is_none_or(|wi| p.window == wi);
        self.snap
            .panes
            .iter()
            .filter(matches)
            .find(|p| p.active)
            .or_else(|| self.snap.panes.iter().find(matches))
            .map(|p| p.id.clone())
    }

    /// Refresh the live preview for the current selection (a `capture-pane` of
    /// the resolved target). Cheap enough to run each poll tick.
    fn update_preview(&mut self) {
        match self.current_pane_target() {
            Some(target) => {
                self.preview_title = format!(" preview: {target} ");
                let out = ztmux_cmd(&self.socket, &["capture-pane", "-p", "-t", &target]).output();
                self.preview = match out {
                    Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .map(str::to_string)
                        .collect(),
                    Ok(o) => vec![String::from_utf8_lossy(&o.stderr).trim().to_string()],
                    Err(e) => vec![format!("capture failed: {e}")],
                };
            }
            None => {
                self.preview_title = " preview ".into();
                self.preview.clear();
            }
        }
    }

    /// Attach/focus the current selection: switch the client to its session and
    /// select the matching window / pane, mirroring `ztmux switch`.
    fn focus_selected(&mut self) {
        let Some(sel) = self.list.selected() else {
            return;
        };
        let mut cmds: Vec<Vec<String>> = Vec::new();
        let label = match self.rows.get(sel) {
            Some(TreeRow::Session(si)) => {
                let s = &self.snap.sessions[si.to_owned()];
                cmds.push(vec!["switch-client".into(), "-t".into(), s.name.clone()]);
                format!("session {}", s.name)
            }
            Some(TreeRow::Window(wi)) => {
                let w = &self.snap.windows[wi.to_owned()];
                let target = format!("{}:{}", w.session, w.index);
                cmds.push(vec!["switch-client".into(), "-t".into(), w.session.clone()]);
                cmds.push(vec!["select-window".into(), "-t".into(), target]);
                format!("window {}:{}", w.session, w.index)
            }
            Some(TreeRow::Pane(pi)) => {
                let p = &self.snap.panes[pi.to_owned()];
                let win = format!("{}:{}", p.session, p.window);
                cmds.push(vec!["switch-client".into(), "-t".into(), p.session.clone()]);
                cmds.push(vec!["select-window".into(), "-t".into(), win]);
                cmds.push(vec!["select-pane".into(), "-t".into(), p.id.clone()]);
                format!("pane {}", p.id)
            }
            None => return,
        };
        for c in &cmds {
            let args: Vec<&str> = c.iter().map(String::as_str).collect();
            if let Ok(o) = ztmux_cmd(&self.socket, &args).output()
                && !o.status.success()
            {
                self.status = format!(
                    "focus failed: {}",
                    String::from_utf8_lossy(&o.stderr).trim()
                );
                return;
            }
        }
        self.status = format!("focused {label}");
    }
}

// ─── rendering ───────────────────────────────────────────────────────────────

/// Entry point for the `ztmux dashboard` subcommand. `socket` is the resolved
/// server socket path (from `-S`/`-L`/`$TMUX`). Returns a process exit code.
pub(crate) fn run(socket: &str) -> i32 {
    let mut terminal = ratatui::init();
    // ratatui::init() sets up raw mode + the alternate screen but NOT mouse
    // reporting, so scroll/click never reached us — enable it explicitly.
    let _ = execute!(std::io::stdout(), EnableMouseCapture);
    let mut app = App::new(socket.to_string());

    let result = (|| -> std::io::Result<()> {
        while !app.quit {
            terminal.draw(|f| ui(f, &mut app))?;

            let timeout = app.refresh_every.saturating_sub(app.last_refresh.elapsed());
            // Dispatch every ready event explicitly: a fragile `&&` let-chain
            // silently dropped anything that was not a key press (including
            // mouse and resize), which is why the mouse did nothing.
            if event::poll(timeout)? {
                match event::read()? {
                    Event::Key(k) if k.kind == KeyEventKind::Press => handle_key(k.code, &mut app),
                    Event::Mouse(m) => handle_mouse(&mut app, m),
                    _ => {}
                }
            }
            if app.last_refresh.elapsed() >= app.refresh_every {
                app.refresh();
            }
        }
        Ok(())
    })();

    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("dashboard error: {e}");
            1
        }
    }
}

/// Mouse: wheel scrolls the tree, left-click selects the row under the cursor,
/// right-click selects and focuses it.
fn handle_mouse(app: &mut App, m: MouseEvent) {
    match m.kind {
        MouseEventKind::ScrollDown => {
            app.move_sel(1);
            app.update_preview();
        }
        MouseEventKind::ScrollUp => {
            app.move_sel(-1);
            app.update_preview();
        }
        // Left click selects the row under the cursor.
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some(idx) = row_at(app, m) {
                app.list.select(Some(idx));
                app.update_preview();
            }
        }
        // Right click selects the row under the cursor and focuses it (attaches
        // the client to that session/window/pane) — a one-gesture "go there".
        MouseEventKind::Down(MouseButton::Right) => {
            if let Some(idx) = row_at(app, m) {
                app.list.select(Some(idx));
                app.focus_selected();
                app.refresh();
            }
        }
        _ => {}
    }
}

/// Map a mouse position to a tree row index, if it lands inside the list body.
fn row_at(app: &App, m: MouseEvent) -> Option<usize> {
    let a = app.tree_area;
    // Inside the bordered list body (skip the top border row)?
    let inside =
        m.column >= a.x && m.column < a.x + a.width && m.row > a.y && m.row + 1 < a.y + a.height;
    if !inside {
        return None;
    }
    let idx = (m.row - a.y - 1) as usize + app.list.offset();
    (idx < app.rows.len()).then_some(idx)
}

fn handle_key(code: KeyCode, app: &mut App) {
    if app.confirm_kill {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                app.confirm_kill = false;
                app.kill_selected();
            }
            _ => {
                app.confirm_kill = false;
                app.status = "kill cancelled".into();
            }
        }
        return;
    }
    if app.show_help {
        app.show_help = false;
        return;
    }
    match code {
        KeyCode::Char('q') | KeyCode::Esc => app.quit = true,
        KeyCode::Char('j') | KeyCode::Down => {
            app.move_sel(1);
            app.update_preview();
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.move_sel(-1);
            app.update_preview();
        }
        KeyCode::Char('g') | KeyCode::Home => {
            app.select_edge(true);
            app.update_preview();
        }
        KeyCode::Char('G') | KeyCode::End => {
            app.select_edge(false);
            app.update_preview();
        }
        KeyCode::Enter => {
            app.focus_selected();
            app.refresh();
        }
        KeyCode::Char('r') => {
            app.refresh();
            app.status = "refreshed".into();
        }
        KeyCode::Char('?') => app.show_help = true,
        KeyCode::Char('x') if app.list.selected().is_some() && !app.rows.is_empty() => {
            app.confirm_kill = true;
        }
        _ => {}
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // header
            Constraint::Min(3),    // body
            Constraint::Length(1), // footer
        ])
        .split(f.area());

    render_header(f, app, root[0]);

    if let Some(err) = app.snap.error.clone() {
        let msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  no ztmux server reachable",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )),
            Line::from(format!("  {err}")),
            Line::from(""),
            Line::from("  start one with `ztmux new-session`, then press r"),
        ])
        .block(Block::default().borders(Borders::ALL).title(" ztmux "));
        f.render_widget(msg, root[1]);
    } else {
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
            .split(root[1]);
        render_tree(f, app, body[0]);
        // Right column: structured detail on top, a live pane preview below.
        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(body[1]);
        render_detail(f, app, right[0]);
        render_preview(f, app, right[1]);
    }

    render_footer(f, app, root[2]);

    if app.confirm_kill {
        render_confirm(f, app);
    }
    if app.show_help {
        render_help(f);
    }
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let s = &app.snap;
    let server = s.sessions.first();
    let pid = "?"; // pid is per-session in our schema; keep the header compact
    let _ = pid;
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    let attached = s.sessions.iter().filter(|x| x.attached).count();
    let stats = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                " ztmux dashboard ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{} sessions", s.sessions.len()),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw(" · "),
            Span::styled(
                format!("{} windows", s.windows.len()),
                Style::default().fg(Color::Green),
            ),
            Span::raw(" · "),
            Span::styled(
                format!("{} panes", s.panes.len()),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(" · "),
            Span::styled(
                format!("{} clients", s.clients.len()),
                Style::default().fg(Color::Magenta),
            ),
        ]),
        Line::from(vec![
            Span::raw(" "),
            Span::styled(
                format!("{attached} attached"),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw("   "),
            Span::styled(
                match server {
                    Some(_) => "● live".to_string(),
                    None => "○ idle".to_string(),
                },
                Style::default().fg(Color::Green),
            ),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(stats, cols[0]);

    let spark_data: Vec<u64> = app.history.iter().copied().collect();
    let max = spark_data.iter().copied().max().unwrap_or(1).max(1);
    let spark = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" panes (max {max}) ")),
        )
        .data(&spark_data)
        .style(Style::default().fg(Color::Yellow));
    f.render_widget(spark, cols[1]);
}

fn render_tree(f: &mut Frame, app: &mut App, area: Rect) {
    app.tree_area = area; // remember for mouse hit-testing
    let items: Vec<ListItem> = app
        .rows
        .iter()
        .map(|r| match r {
            TreeRow::Session(si) => {
                let s = &app.snap.sessions[*si];
                let marker = if s.attached { "*" } else { " " };
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("▸ {} ", s.name),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("({} win){}", s.windows, marker),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]))
            }
            TreeRow::Window(wi) => {
                let w = &app.snap.windows[*wi];
                let active = if w.active {
                    Span::styled(" ●", Style::default().fg(Color::Green))
                } else {
                    Span::raw("")
                };
                ListItem::new(Line::from(vec![
                    Span::raw(format!("    {}: ", w.index)),
                    Span::styled(w.name.clone(), Style::default().fg(Color::White)),
                    Span::styled(
                        format!(" [{} panes]", w.panes),
                        Style::default().fg(Color::DarkGray),
                    ),
                    active,
                ]))
            }
            TreeRow::Pane(pi) => {
                let p = &app.snap.panes[*pi];
                let flag = if p.active {
                    Span::styled(" ●", Style::default().fg(Color::Green))
                } else if p.dead {
                    Span::styled(" ✝", Style::default().fg(Color::Red))
                } else {
                    Span::raw("")
                };
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("        {} ", p.id),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(p.command.clone(), Style::default().fg(Color::Green)),
                    flag,
                ]))
            }
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" sessions "))
        .highlight_style(
            Style::default()
                .bg(Color::Indexed(237))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("❯ ");
    f.render_stateful_widget(list, area, &mut app.list);
}

fn render_detail(f: &mut Frame, app: &App, area: Rect) {
    let Some(sel) = app.list.selected() else {
        let p = Paragraph::new("no selection")
            .block(Block::default().borders(Borders::ALL).title(" detail "));
        f.render_widget(p, area);
        return;
    };

    match app.rows.get(sel) {
        Some(TreeRow::Session(si)) => render_session_detail(f, app, *si, area),
        Some(TreeRow::Window(wi)) => render_window_panes(f, app, *wi, area),
        Some(TreeRow::Pane(pi)) => render_pane_detail(f, app, *pi, area),
        None => {}
    }
}

fn render_pane_detail(f: &mut Frame, app: &App, pi: usize, area: Rect) {
    let p = &app.snap.panes[pi];
    let flags = {
        let mut v = Vec::new();
        if p.active {
            v.push("active");
        }
        if p.dead {
            v.push("dead");
        }
        if v.is_empty() {
            "-".to_string()
        } else {
            v.join(", ")
        }
    };
    let lines = vec![
        kv("pane", &p.id),
        kv("window", &format!("{}:{}", p.session, p.window)),
        kv("command", &p.command),
        kv("pid", &p.pid.to_string()),
        kv("tty", if p.tty.is_empty() { "-" } else { &p.tty }),
        kv("size", &format!("{}x{}", p.width, p.height)),
        kv("flags", &flags),
        kv("title", if p.title.is_empty() { "-" } else { &p.title }),
        kv("path", &p.path),
    ];
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" pane {} ", p.id));
    let para = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
    f.render_widget(para, area);
}

/// Render the live `capture-pane` preview of the selected target.
fn render_preview(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(app.preview_title.clone())
        .border_style(Style::default().fg(Color::DarkGray));
    // Show the tail so the most recent output stays visible in a short pane.
    let inner_h = area.height.saturating_sub(2) as usize;
    let start = app.preview.len().saturating_sub(inner_h.max(1));
    let text: Vec<Line> = if app.preview.is_empty() {
        vec![Line::from(Span::styled(
            "  (no output / no server)",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        app.preview[start..]
            .iter()
            .map(|l| Line::from(l.clone()))
            .collect()
    };
    f.render_widget(Paragraph::new(text).block(block), area);
}

fn render_help(f: &mut Frame) {
    let area = centered_rect(52, 55, f.area());
    f.render_widget(Clear, area);
    let key = |k: &str, d: &str| -> Line<'static> {
        Line::from(vec![
            Span::styled(
                format!("  {k:<12}"),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(d.to_string()),
        ])
    };
    let lines = vec![
        Line::from(""),
        key("j / k, ↑↓", "move selection"),
        key("g / G", "jump to top / bottom"),
        key("Enter", "focus (attach client to the selection)"),
        key("r", "refresh now"),
        key("x", "kill selection (confirm)"),
        key("?", "toggle this help"),
        key("q / Esc", "quit"),
        Line::from(""),
        Line::from(Span::styled(
            "  mouse: wheel scrolls · left-click selects · right-click focuses",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "  the right column shows a live capture of the selected pane",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from("  press any key to close"),
    ];
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" help ")
        .border_style(Style::default().fg(Color::Cyan));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_session_detail(f: &mut Frame, app: &App, si: usize, area: Rect) {
    let s = &app.snap.sessions[si];
    let clients: Vec<&Client> = app
        .snap
        .clients
        .iter()
        .filter(|c| c.session == s.name)
        .collect();
    let mut lines = vec![
        kv("session", &s.name),
        kv("id", &s.id),
        kv("windows", &s.windows.to_string()),
        kv("attached", if s.attached { "yes" } else { "no" }),
        kv("group", if s.group.is_empty() { "-" } else { &s.group }),
        Line::from(""),
        Line::from(Span::styled(
            format!("  clients ({})", clients.len()),
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];
    for c in clients {
        lines.push(Line::from(format!(
            "    {} {} {}x{} {}",
            c.name, c.tty, c.width, c.height, c.termname
        )));
    }
    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" session: {} ", s.name)),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

fn render_window_panes(f: &mut Frame, app: &App, wi: usize, area: Rect) {
    let w = &app.snap.windows[wi];
    let panes: Vec<&Pane> = app
        .snap
        .panes
        .iter()
        .filter(|p| p.session == w.session && p.window == w.index)
        .collect();

    let header = Row::new(vec!["#", "id", "command", "pid", "size", "path"]).style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(Color::Cyan),
    );
    let rows: Vec<Row> = panes
        .iter()
        .map(|p| {
            let flag = if p.active {
                " ●"
            } else if p.dead {
                " ✝"
            } else {
                ""
            };
            Row::new(vec![
                Cell::from(format!("{}{}", p.index, flag)),
                Cell::from(p.id.clone()),
                Cell::from(p.command.clone()).style(Style::default().fg(Color::Green)),
                Cell::from(p.pid.to_string()),
                Cell::from(format!("{}x{}", p.width, p.height)),
                Cell::from(shorten(&p.path, 30)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(12),
            Constraint::Length(7),
            Constraint::Length(8),
            Constraint::Min(10),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(format!(
        " {}:{} {} — {} panes ",
        w.session, w.index, w.name, w.panes
    )));
    f.render_widget(table, area);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let help = " j/k move · Enter focus · r refresh · x kill · ? help · q quit";
    let line = Line::from(vec![
        Span::styled(
            format!(" {} ", app.status),
            Style::default().fg(Color::Black).bg(Color::DarkGray),
        ),
        Span::styled(help, Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn render_confirm(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 20, f.area());
    let sel = app.list.selected().unwrap_or(0);
    let what = match app.rows.get(sel) {
        Some(TreeRow::Session(si)) => format!("session '{}'", app.snap.sessions[*si].name),
        Some(TreeRow::Window(wi)) => {
            let w = &app.snap.windows[*wi];
            format!("window {}:{} '{}'", w.session, w.index, w.name)
        }
        Some(TreeRow::Pane(pi)) => {
            let p = &app.snap.panes[*pi];
            format!("pane {} ({})", p.id, p.command)
        }
        None => "nothing".to_string(),
    };
    f.render_widget(Clear, area);
    let p = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  kill {what}?"),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  y = yes    any other key = no"),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" confirm ")
            .border_style(Style::default().fg(Color::Red)),
    );
    f.render_widget(p, area);
}

// ─── small helpers ───────────────────────────────────────────────────────────

fn kv(k: &str, v: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {k:<10}"), Style::default().fg(Color::DarkGray)),
        Span::raw(v.to_string()),
    ])
}

fn shorten(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_string();
    }
    let tail: String = chars[chars.len() - (max - 1)..].iter().collect();
    format!("…{tail}")
}

fn centered_rect(pct_x: u16, pct_y: u16, area: Rect) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - pct_y) / 2),
            Constraint::Percentage(pct_y),
            Constraint::Percentage((100 - pct_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - pct_x) / 2),
            Constraint::Percentage(pct_x),
            Constraint::Percentage((100 - pct_x) / 2),
        ])
        .split(v[1])[1]
}

#[cfg(test)]
mod tests {
    use super::super::tmux_query::{Session, Window};
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn sample() -> App {
        let snap = Snapshot {
            sessions: vec![
                Session {
                    name: "work".into(),
                    id: "$0".into(),
                    windows: 2,
                    attached: true,
                    ..Default::default()
                },
                Session {
                    name: "scratch".into(),
                    id: "$1".into(),
                    windows: 1,
                    ..Default::default()
                },
            ],
            windows: vec![
                Window {
                    session: "work".into(),
                    index: 0,
                    name: "zsh".into(),
                    id: "@0".into(),
                    active: false,
                    panes: 2,
                    width: 100,
                    height: 30,
                    ..Default::default()
                },
                Window {
                    session: "work".into(),
                    index: 1,
                    name: "editor".into(),
                    id: "@1".into(),
                    active: true,
                    panes: 1,
                    width: 100,
                    height: 30,
                    ..Default::default()
                },
                Window {
                    session: "scratch".into(),
                    index: 0,
                    name: "zsh".into(),
                    id: "@2".into(),
                    active: true,
                    panes: 1,
                    ..Default::default()
                },
            ],
            panes: vec![Pane {
                session: "work".into(),
                window: 1,
                index: 0,
                id: "%2".into(),
                active: true,
                pid: 113,
                command: "nvim".into(),
                path: "/home/x/proj".into(),
                width: 100,
                height: 30,
                ..Default::default()
            }],
            clients: vec![Client {
                name: "c0".into(),
                tty: "/dev/ttys9".into(),
                session: "work".into(),
                width: 100,
                height: 30,
                termname: "xterm".into(),
                pid: 99,
            }],
            error: None,
        };
        let mut app = App {
            snap,
            socket: String::new(),
            rows: Vec::new(),
            list: ListState::default(),
            history: VecDeque::from([1u64]),
            last_refresh: Instant::now(),
            refresh_every: Duration::from_millis(1000),
            confirm_kill: false,
            show_help: false,
            preview: Vec::new(),
            preview_title: String::new(),
            tree_area: Rect::default(),
            status: "test".into(),
            quit: false,
        };
        app.rebuild_rows();
        app.list.select(Some(0));
        app
    }

    fn render(app: &mut App, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| ui(f, app)).unwrap();
        term.backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect()
    }

    #[test]
    fn flattened_tree_has_a_row_per_session_window_and_pane() {
        let app = sample();
        // 2 sessions + 3 windows + 1 pane (only work:1 has a pane in the snapshot)
        assert_eq!(app.rows.len(), 6);
    }

    #[test]
    fn selecting_a_pane_shows_its_detail() {
        let mut app = sample();
        app.list.select(Some(3)); // the pane row under the "editor" window
        let s = render(&mut app, 120, 40);
        assert!(s.contains("%2"), "pane id in detail");
        assert!(s.contains("nvim"), "pane command in detail");
    }

    #[test]
    fn renders_header_stats_and_tree() {
        let mut app = sample();
        let s = render(&mut app, 120, 40);
        assert!(s.contains("ztmux dashboard"), "header title");
        assert!(s.contains("2 sessions"), "session count");
        assert!(s.contains("work") && s.contains("editor") && s.contains("scratch"));
    }

    #[test]
    fn selecting_a_window_shows_its_panes() {
        let mut app = sample();
        app.list.select(Some(2)); // the "editor" window row
        let s = render(&mut app, 120, 40);
        assert!(s.contains("nvim"), "pane command in detail table");
        assert!(s.contains("113"), "pane pid in detail table");
    }

    #[test]
    fn selecting_a_session_shows_its_clients() {
        let mut app = sample();
        app.list.select(Some(0)); // the "work" session row
        let s = render(&mut app, 120, 40);
        assert!(s.contains("clients"), "clients heading");
        assert!(s.contains("xterm"), "client termname");
    }

    #[test]
    fn mouse_click_selects_row_under_cursor() {
        use ratatui::crossterm::event::KeyModifiers;
        let mut app = sample();
        // Tree list body starts one row below the top border at y=1 (so y=2 is
        // the first item). A click at row 3 maps to item index 1.
        app.tree_area = Rect::new(0, 1, 40, 10);
        app.list.select(Some(0));
        handle_mouse(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 5,
                row: 3,
                modifiers: KeyModifiers::empty(),
            },
        );
        assert_eq!(app.list.selected(), Some(1));
    }

    #[test]
    fn row_at_hit_tests_the_list_body() {
        use ratatui::crossterm::event::KeyModifiers;
        let mut app = sample();
        app.tree_area = Rect::new(0, 1, 40, 10);
        let at = |row, col| MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: col,
            row,
            modifiers: KeyModifiers::empty(),
        };
        // Row 2 = first item (idx 0); row 4 = idx 2.
        assert_eq!(row_at(&app, at(2, 5)), Some(0));
        assert_eq!(row_at(&app, at(4, 5)), Some(2));
        // On the top border (row == y) → no hit.
        assert_eq!(row_at(&app, at(1, 5)), None);
        // Past the last row (only 6 rows) → no hit.
        assert_eq!(row_at(&app, at(9, 5)), None);
        // Outside the horizontal bounds → no hit.
        assert_eq!(row_at(&app, at(2, 99)), None);
    }

    #[test]
    fn mouse_scroll_moves_selection() {
        use ratatui::crossterm::event::KeyModifiers;
        let mut app = sample();
        app.list.select(Some(0));
        let ev = |kind| MouseEvent {
            kind,
            column: 1,
            row: 1,
            modifiers: KeyModifiers::empty(),
        };
        handle_mouse(&mut app, ev(MouseEventKind::ScrollDown));
        assert_eq!(app.list.selected(), Some(1));
        handle_mouse(&mut app, ev(MouseEventKind::ScrollUp));
        assert_eq!(app.list.selected(), Some(0));
    }

    #[test]
    fn navigation_wraps_around() {
        let mut app = sample();
        app.list.select(Some(0));
        app.move_sel(-1);
        assert_eq!(app.list.selected(), Some(5)); // wraps to last row (6 rows)
        app.move_sel(1);
        assert_eq!(app.list.selected(), Some(0)); // wraps back to first
    }

    #[test]
    fn error_snapshot_renders_no_server_screen() {
        let mut app = sample();
        app.snap = Snapshot {
            error: Some("no server running".into()),
            ..Default::default()
        };
        app.rebuild_rows();
        let s = render(&mut app, 100, 30);
        assert!(s.contains("no ztmux server reachable"));
    }
}
