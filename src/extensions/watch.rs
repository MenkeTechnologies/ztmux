//! `ztmux watch` — a top-like live monitor of every pane's process.
//!
//! A client subcommand (like [`super::dashboard`]) that reads the pane list from
//! the `list-* -o json` query layer and joins it with per-process CPU / memory
//! from a single `ps` call keyed on `pane_pid`. It shows a server-wide "what is
//! running" table, refreshed a few times a second and sortable by CPU, memory,
//! or command — something no GUI client offers because only the multiplexer
//! knows every pane's process.
//!
//! Keys: j/k or ↑/↓ move · s cycle sort (cpu/mem/name) · r refresh · q/Esc quit.
//! Mouse: wheel scrolls, left-click selects a row, right-click focuses its pane.

use std::collections::HashMap;
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
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};

use super::procstat::{ProcStat, fmt_rss, gather};
use super::tmux_query::{Snapshot, poll, ztmux_cmd};

#[derive(Clone, Copy, PartialEq)]
enum Sort {
    Cpu,
    Mem,
    Name,
}

impl Sort {
    fn label(self) -> &'static str {
        match self {
            Sort::Cpu => "cpu",
            Sort::Mem => "mem",
            Sort::Name => "name",
        }
    }
    fn next(self) -> Sort {
        match self {
            Sort::Cpu => Sort::Mem,
            Sort::Mem => Sort::Name,
            Sort::Name => Sort::Cpu,
        }
    }
}

/// One rendered row: pane identity joined with its process stats.
#[derive(Clone)]
struct WatchRow {
    id: String,
    session: String,
    window: i64,
    loc: String,
    command: String,
    pid: i64,
    stat: ProcStat,
    active: bool,
    dead: bool,
}

struct App {
    socket: String,
    rows: Vec<WatchRow>,
    table: TableState,
    table_area: Rect, // last rendered rect of the table (for mouse hit-testing)
    sort: Sort,
    last_refresh: Instant,
    refresh_every: Duration,
    error: Option<String>,
    status: Option<String>,
    quit: bool,
}

impl App {
    fn new(socket: String) -> Self {
        let mut app = App {
            socket,
            rows: Vec::new(),
            table: TableState::default(),
            table_area: Rect::default(),
            sort: Sort::Cpu,
            last_refresh: Instant::now(),
            refresh_every: Duration::from_millis(1500),
            error: None,
            status: None,
            quit: false,
        };
        app.refresh();
        app.table.select(Some(0));
        app
    }

    fn refresh(&mut self) {
        let snap = poll(&self.socket);
        self.error.clone_from(&snap.error);
        self.rows = build_rows(&snap, &gather(&pane_pids(&snap)));
        self.sort_rows();
        self.last_refresh = Instant::now();
        if self.rows.is_empty() {
            self.table.select(None);
        } else {
            let sel = self.table.selected().unwrap_or(0).min(self.rows.len() - 1);
            self.table.select(Some(sel));
        }
    }

    fn sort_rows(&mut self) {
        match self.sort {
            Sort::Cpu => self.rows.sort_by(|a, b| {
                b.stat
                    .cpu
                    .partial_cmp(&a.stat.cpu)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            Sort::Mem => self.rows.sort_by(|a, b| {
                b.stat
                    .mem
                    .partial_cmp(&a.stat.mem)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            Sort::Name => self.rows.sort_by(|a, b| a.command.cmp(&b.command)),
        }
    }

    fn move_sel(&mut self, delta: isize) {
        if self.rows.is_empty() {
            return;
        }
        let n = self.rows.len() as isize;
        let cur = self.table.selected().unwrap_or(0) as isize;
        self.table
            .select(Some((cur + delta).rem_euclid(n) as usize));
    }

    /// Focus the selected pane: attach the client to its session, select its
    /// window, then select the pane itself (mirroring `ztmux switch`).
    fn focus_selected(&mut self) {
        let Some(row) = self.table.selected().and_then(|i| self.rows.get(i)) else {
            return;
        };
        // Clone out before issuing commands so we can borrow self mutably again.
        let (session, id) = (row.session.clone(), row.id.clone());
        let win = format!("{}:{}", row.session, row.window);
        let cmds: [Vec<&str>; 3] = [
            vec!["switch-client", "-t", &session],
            vec!["select-window", "-t", &win],
            vec!["select-pane", "-t", &id],
        ];
        for c in &cmds {
            if let Ok(o) = ztmux_cmd(&self.socket, c).output()
                && !o.status.success()
            {
                self.status = Some(format!(
                    "focus failed: {}",
                    String::from_utf8_lossy(&o.stderr).trim()
                ));
                return;
            }
        }
        self.status = Some(format!("focused {id}"));
    }
}

fn pane_pids(snap: &Snapshot) -> Vec<i64> {
    snap.panes
        .iter()
        .map(|p| p.pid)
        .filter(|&pid| pid > 0)
        .collect()
}

fn build_rows(snap: &Snapshot, stats: &HashMap<i64, ProcStat>) -> Vec<WatchRow> {
    snap.panes
        .iter()
        .map(|p| WatchRow {
            id: p.id.clone(),
            session: p.session.clone(),
            window: p.window,
            loc: format!("{}:{}.{}", p.session, p.window, p.index),
            command: p.command.clone(),
            pid: p.pid,
            stat: stats.get(&p.pid).cloned().unwrap_or_default(),
            active: p.active,
            dead: p.dead,
        })
        .collect()
}

pub(crate) fn run(socket: &str) -> i32 {
    let mut terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableMouseCapture);
    let mut app = App::new(socket.to_string());
    let result = (|| -> std::io::Result<()> {
        while !app.quit {
            terminal.draw(|f| ui(f, &mut app))?;
            let timeout = app.refresh_every.saturating_sub(app.last_refresh.elapsed());
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
            eprintln!("watch error: {e}");
            1
        }
    }
}

/// Map a mouse position to a table row index, if it lands on a data row. The
/// table body starts two rows below the block top (border + header row).
fn row_at(app: &App, m: MouseEvent) -> Option<usize> {
    let a = app.table_area;
    let inside = m.column >= a.x
        && m.column < a.x + a.width
        && m.row >= a.y + 2
        && m.row + 1 < a.y + a.height;
    if !inside {
        return None;
    }
    let idx = (m.row - a.y - 2) as usize + app.table.offset();
    (idx < app.rows.len()).then_some(idx)
}

/// Mouse: wheel scrolls, left-click selects the row, right-click focuses it.
fn handle_mouse(app: &mut App, m: MouseEvent) {
    match m.kind {
        MouseEventKind::ScrollDown => app.move_sel(1),
        MouseEventKind::ScrollUp => app.move_sel(-1),
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some(idx) = row_at(app, m) {
                app.table.select(Some(idx));
            }
        }
        MouseEventKind::Down(MouseButton::Right) => {
            if let Some(idx) = row_at(app, m) {
                app.table.select(Some(idx));
                app.focus_selected();
            }
        }
        _ => {}
    }
}

fn handle_key(code: KeyCode, app: &mut App) {
    match code {
        KeyCode::Char('q') | KeyCode::Esc => app.quit = true,
        KeyCode::Char('j') | KeyCode::Down => app.move_sel(1),
        KeyCode::Char('k') | KeyCode::Up => app.move_sel(-1),
        KeyCode::Char('s') => {
            app.sort = app.sort.next();
            app.sort_rows();
        }
        KeyCode::Char('r') => app.refresh(),
        _ => {}
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(f.area());
    render_header(f, app, root[0]);
    if let Some(err) = app.error.clone() {
        let p = Paragraph::new(format!("  no ztmux server reachable\n  {err}")).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" ztmux watch "),
        );
        f.render_widget(p, root[1]);
    } else {
        render_table(f, app, root[1]);
    }
    let help = " j/k move · s sort · r refresh · right-click focus · q quit";
    let footer = match &app.status {
        Some(s) => Line::from(vec![
            Span::styled(
                format!(" {s} "),
                Style::default().fg(Color::Black).bg(Color::DarkGray),
            ),
            Span::styled(help, Style::default().fg(Color::DarkGray)),
        ]),
        None => Line::from(Span::styled(help, Style::default().fg(Color::DarkGray))),
    };
    f.render_widget(Paragraph::new(footer), root[2]);
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let total_cpu: f32 = app.rows.iter().map(|r| r.stat.cpu).sum();
    let total_mem: f32 = app.rows.iter().map(|r| r.stat.mem).sum();
    let line = Line::from(vec![
        Span::styled(
            " ztmux watch ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("{} panes", app.rows.len()),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw(" · "),
        Span::styled(
            format!("Σcpu {total_cpu:.1}%"),
            Style::default().fg(Color::Green),
        ),
        Span::raw(" · "),
        Span::styled(
            format!("Σmem {total_mem:.1}%"),
            Style::default().fg(Color::Magenta),
        ),
        Span::raw(" · "),
        Span::styled(
            format!("sort: {}", app.sort.label()),
            Style::default().fg(Color::Cyan),
        ),
    ]);
    f.render_widget(
        Paragraph::new(line).block(Block::default().borders(Borders::ALL)),
        area,
    );
}

fn render_table(f: &mut Frame, app: &mut App, area: Rect) {
    app.table_area = area; // remember for mouse hit-testing
    let header = Row::new(vec![
        "pane", "location", "command", "pid", "%cpu", "%mem", "rss", "st",
    ])
    .style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(Color::Cyan),
    );
    let rows: Vec<Row> = app
        .rows
        .iter()
        .map(|r| {
            let flag = if r.active {
                "●"
            } else if r.dead {
                "✝"
            } else {
                ""
            };
            let cpu_style = if r.stat.cpu >= 50.0 {
                Style::default().fg(Color::Red)
            } else if r.stat.cpu >= 10.0 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
            Row::new(vec![
                Cell::from(format!("{} {}", r.id, flag)),
                Cell::from(r.loc.clone()),
                Cell::from(r.command.clone()).style(Style::default().fg(Color::Green)),
                Cell::from(r.pid.to_string()),
                Cell::from(format!("{:.1}", r.stat.cpu)).style(cpu_style),
                Cell::from(format!("{:.1}", r.stat.mem)),
                Cell::from(fmt_rss(r.stat.rss_kb)),
                Cell::from(r.stat.state.clone()),
            ])
        })
        .collect();
    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Length(8),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Length(4),
        ],
    )
    .header(header)
    .row_highlight_style(
        Style::default()
            .bg(Color::Indexed(237))
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("❯ ")
    .block(Block::default().borders(Borders::ALL).title(" processes "));
    f.render_stateful_widget(table, area, &mut app.table);
}

#[cfg(test)]
mod tests {
    use super::super::tmux_query::Pane;
    use super::*;

    #[test]
    fn rows_join_stats_and_sort_by_cpu() {
        let snap = Snapshot {
            panes: vec![
                Pane {
                    id: "%0".into(),
                    pid: 10,
                    command: "zsh".into(),
                    ..Default::default()
                },
                Pane {
                    id: "%1".into(),
                    pid: 20,
                    command: "nvim".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut stats = HashMap::new();
        stats.insert(
            10,
            ProcStat {
                cpu: 2.0,
                mem: 1.0,
                rss_kb: 1000,
                state: "S".into(),
            },
        );
        stats.insert(
            20,
            ProcStat {
                cpu: 90.0,
                mem: 5.0,
                rss_kb: 2000,
                state: "R".into(),
            },
        );
        let mut app = App {
            socket: String::new(),
            rows: build_rows(&snap, &stats),
            table: TableState::default(),
            table_area: Rect::default(),
            sort: Sort::Cpu,
            last_refresh: Instant::now(),
            refresh_every: Duration::from_millis(1500),
            error: None,
            status: None,
            quit: false,
        };
        app.sort_rows();
        assert_eq!(app.rows[0].id, "%1"); // nvim, 90% cpu on top
        app.sort = Sort::Name;
        app.sort_rows();
        assert_eq!(app.rows[0].command, "nvim"); // alphabetical
    }

    #[test]
    fn row_at_hit_tests_the_table_body() {
        use ratatui::crossterm::event::KeyModifiers;
        let snap = Snapshot {
            panes: vec![
                Pane {
                    id: "%0".into(),
                    pid: 10,
                    ..Default::default()
                },
                Pane {
                    id: "%1".into(),
                    pid: 20,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let app = App {
            socket: String::new(),
            rows: build_rows(&snap, &HashMap::new()),
            table: TableState::default(),
            table_area: Rect::new(0, 0, 40, 10),
            sort: Sort::Cpu,
            last_refresh: Instant::now(),
            refresh_every: Duration::from_millis(1500),
            error: None,
            status: None,
            quit: false,
        };
        let at = |row| MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: 3,
            row,
            modifiers: KeyModifiers::empty(),
        };
        // Body starts at y+2 (border + header): row 2 → idx 0, row 3 → idx 1.
        assert_eq!(row_at(&app, at(2)), Some(0));
        assert_eq!(row_at(&app, at(3)), Some(1));
        // Header/border rows and past-the-end → no hit.
        assert_eq!(row_at(&app, at(1)), None);
        assert_eq!(row_at(&app, at(4)), None); // only 2 rows
        // Outside the horizontal bounds → no hit.
        assert_eq!(
            row_at(
                &app,
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Right),
                    column: 99,
                    row: 2,
                    modifiers: KeyModifiers::empty(),
                }
            ),
            None
        );
    }
}
