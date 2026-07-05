//! `ztmux sessions` — a zellij-style session manager (Ctrl-o w in zellij).
//!
//! A full-screen ratatui list of the server's sessions with CRUD: Enter switches
//! to one, Ctrl-r renames, Ctrl-x kills (with a confirm), Ctrl-n makes a new one.
//! Type to filter. Like the [`super::switch`] picker it is a client subcommand
//! that reads the server through `list-* -o json` ([`super::tmux_query`]) and
//! acts through `ztmux` commands, so it needs no server linkage.

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

use super::tmux_query::{Session, ztmux_cmd};

/// What the input line is doing: just filtering the list, or editing a name for
/// a rename / new-session, or confirming a kill.
enum Mode {
    List,
    Rename,
    New,
    ConfirmKill,
}

struct App {
    socket: String,
    sessions: Vec<Session>,
    filtered: Vec<usize>,
    query: String,
    input: String, // the name being typed in Rename / New mode
    mode: Mode,
    list: ListState,
    list_area: Rect,
    attached: Vec<String>, // names of currently-attached sessions
    error: Option<String>,
    status: String,
    quit: bool,
}

impl App {
    fn new(socket: String) -> Self {
        let mut app = App {
            socket,
            sessions: Vec::new(),
            filtered: Vec::new(),
            query: String::new(),
            input: String::new(),
            mode: Mode::List,
            list: ListState::default(),
            list_area: Rect::default(),
            attached: Vec::new(),
            error: None,
            status: String::new(),
            quit: false,
        };
        app.reload();
        app
    }

    /// Re-read the session list from the server, preserving the selection where
    /// possible. Sessions are shown most-recently-active first.
    fn reload(&mut self) {
        match super::tmux_query::run_json::<Session>(&self.socket, &["list-sessions", "-o", "json"])
        {
            Ok(mut v) => {
                v.sort_by(|a, b| b.activity.cmp(&a.activity).then(a.name.cmp(&b.name)));
                self.sessions = v;
                self.error = None;
            }
            Err(e) => {
                self.sessions.clear();
                self.error = Some(e);
            }
        }
        self.attached = self
            .sessions
            .iter()
            .filter(|s| s.attached)
            .map(|s| s.name.clone())
            .collect();
        self.refilter();
    }

    fn refilter(&mut self) {
        let q = self.query.to_lowercase();
        self.filtered = self
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, s)| q.is_empty() || s.name.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect();
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

    /// The session currently highlighted, if any.
    fn selected(&self) -> Option<&Session> {
        self.list
            .selected()
            .and_then(|i| self.filtered.get(i))
            .and_then(|&si| self.sessions.get(si))
    }

    /// Run a `ztmux` command against the server; record stderr on failure.
    /// Returns whether it succeeded.
    fn run_cmd(&mut self, args: &[&str]) -> bool {
        match ztmux_cmd(&self.socket, args).output() {
            Ok(o) if o.status.success() => true,
            Ok(o) => {
                self.status = String::from_utf8_lossy(&o.stderr).trim().to_string();
                false
            }
            Err(e) => {
                self.status = format!("spawn: {e}");
                false
            }
        }
    }

    fn switch_to(&mut self) {
        let Some(name) = self.selected().map(|s| s.name.clone()) else {
            return;
        };
        if self.run_cmd(&["switch-client", "-t", &name]) {
            self.quit = true;
        }
    }

    fn commit_rename(&mut self) {
        let new = self.input.trim().to_string();
        let Some(old) = self.selected().map(|s| s.name.clone()) else {
            return;
        };
        if !new.is_empty() && new != old {
            self.run_cmd(&["rename-session", "-t", &old, &new]);
        }
        self.mode = Mode::List;
        self.input.clear();
        self.reload();
    }

    fn commit_new(&mut self) {
        let name = self.input.trim().to_string();
        // -d so it's created detached; a blank name lets the server auto-number.
        if name.is_empty() {
            self.run_cmd(&["new-session", "-d"]);
        } else {
            self.run_cmd(&["new-session", "-d", "-s", &name]);
        }
        self.mode = Mode::List;
        self.input.clear();
        self.reload();
    }

    fn commit_kill(&mut self) {
        if let Some(name) = self.selected().map(|s| s.name.clone()) {
            self.run_cmd(&["kill-session", "-t", &name]);
        }
        self.mode = Mode::List;
        self.reload();
    }
}

pub(crate) fn run(socket: &str) -> i32 {
    let mut terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableMouseCapture);
    let mut app = App::new(socket.to_string());
    let result = (|| -> std::io::Result<()> {
        while !app.quit {
            terminal.draw(|f| ui(f, &mut app))?;
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => on_key(&mut app, k),
                Event::Mouse(m) => on_mouse(&mut app, m),
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
            eprintln!("sessions error: {e}");
            1
        }
    }
}

fn on_key(app: &mut App, k: ratatui::crossterm::event::KeyEvent) {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl && matches!(k.code, KeyCode::Char('c')) {
        app.quit = true;
        return;
    }
    match app.mode {
        // Editing a name (rename / new): typing edits it, Enter commits, Esc aborts.
        Mode::Rename | Mode::New => match k.code {
            KeyCode::Esc => {
                app.mode = Mode::List;
                app.input.clear();
            }
            KeyCode::Enter => {
                if matches!(app.mode, Mode::Rename) {
                    app.commit_rename();
                } else {
                    app.commit_new();
                }
            }
            KeyCode::Backspace => {
                app.input.pop();
            }
            KeyCode::Char(c) => app.input.push(c),
            _ => {}
        },
        // Confirming a kill: y/Enter kills, anything else cancels.
        Mode::ConfirmKill => match k.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => app.commit_kill(),
            _ => app.mode = Mode::List,
        },
        // The list: type to filter, single/Ctrl keys act on the selection.
        Mode::List => match k.code {
            KeyCode::Esc => app.quit = true,
            KeyCode::Enter => app.switch_to(),
            KeyCode::Down => app.move_sel(1),
            KeyCode::Up => app.move_sel(-1),
            KeyCode::Char('n') if ctrl => {
                app.mode = Mode::New;
                app.input.clear();
                app.status.clear();
            }
            KeyCode::Char('r') if ctrl => {
                if let Some(s) = app.selected() {
                    app.input = s.name.clone();
                    app.mode = Mode::Rename;
                    app.status.clear();
                }
            }
            KeyCode::Char('x') if ctrl => {
                if app.selected().is_some() {
                    app.mode = Mode::ConfirmKill;
                }
            }
            KeyCode::Backspace => {
                app.query.pop();
                app.refilter();
            }
            KeyCode::Char(c) => {
                app.query.push(c);
                app.refilter();
            }
            _ => {}
        },
    }
}

fn on_mouse(app: &mut App, m: MouseEvent) {
    if !matches!(app.mode, Mode::List) {
        return;
    }
    match m.kind {
        MouseEventKind::ScrollDown => app.move_sel(1),
        MouseEventKind::ScrollUp => app.move_sel(-1),
        MouseEventKind::Down(MouseButton::Left) => {
            let a = app.list_area;
            let inside = m.column >= a.x
                && m.column < a.x + a.width
                && m.row > a.y
                && m.row + 1 < a.y + a.height;
            if inside {
                let idx = (m.row - a.y - 1) as usize + app.list.offset();
                if idx < app.filtered.len() {
                    app.list.select(Some(idx));
                }
            }
        }
        MouseEventKind::Down(MouseButton::Right) => app.switch_to(),
        _ => {}
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // input / search box
            Constraint::Min(3),    // session list
            Constraint::Length(1), // help
        ])
        .split(f.area());

    // Top box: the filter query, or the name being typed in an edit mode.
    let (label, value, box_title) = match app.mode {
        Mode::Rename => ("rename to: ", app.input.as_str(), " rename session "),
        Mode::New => ("new name: ", app.input.as_str(), " new session "),
        _ => ("❯ ", app.query.as_str(), " sessions "),
    };
    let title = if let Some(e) = &app.error {
        format!(" no server: {e} ")
    } else {
        format!("{box_title}({} )", app.filtered.len())
    };
    let input = Paragraph::new(Line::from(vec![
        Span::styled(label, Style::default().fg(Color::Cyan)),
        Span::raw(value.to_string()),
        Span::styled("▏", Style::default().fg(Color::DarkGray)),
    ]))
    .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(input, rows[0]);

    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .map(|&i| {
            let s = &app.sessions[i];
            let live = s.attached;
            let (dot, dot_c) = if live {
                ("● ", Color::Green)
            } else {
                ("○ ", Color::DarkGray)
            };
            let win = format!(
                "{} window{}",
                s.windows,
                if s.windows == 1 { "" } else { "s" }
            );
            ListItem::new(Line::from(vec![
                Span::styled(dot, Style::default().fg(dot_c)),
                Span::styled(
                    format!("{:<20}", s.name),
                    Style::default()
                        .fg(if live { Color::White } else { Color::Gray })
                        .add_modifier(if live {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::styled(win, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" switch · Ctrl-r rename · Ctrl-x kill · Ctrl-n new "),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Indexed(237))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    app.list_area = rows[1];
    f.render_stateful_widget(list, rows[1], &mut app.list);

    let help = match app.mode {
        Mode::ConfirmKill => {
            let name = app.selected().map_or("", |s| s.name.as_str());
            format!(" kill session '{name}'?  y = yes · any other key = cancel")
        }
        Mode::Rename | Mode::New => " enter confirm · esc cancel".to_string(),
        Mode::List if !app.status.is_empty() => format!(" {}", app.status),
        Mode::List => {
            " type filter · ↑/↓ move · enter switch · ^r rename · ^x kill · ^n new · esc quit"
                .to_string()
        }
    };
    let help_style = if matches!(app.mode, Mode::ConfirmKill) {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    f.render_widget(Paragraph::new(Span::styled(help, help_style)), rows[2]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with(names: &[(&str, bool, i64)]) -> App {
        let mut app = App {
            socket: String::new(),
            sessions: names
                .iter()
                .map(|(n, at, act)| Session {
                    name: (*n).into(),
                    attached: *at,
                    activity: *act,
                    windows: 1,
                    ..Default::default()
                })
                .collect(),
            filtered: Vec::new(),
            query: String::new(),
            input: String::new(),
            mode: Mode::List,
            list: ListState::default(),
            list_area: Rect::default(),
            attached: Vec::new(),
            error: None,
            status: String::new(),
            quit: false,
        };
        app.refilter();
        if !app.filtered.is_empty() {
            app.list.select(Some(0));
        }
        app
    }

    #[test]
    fn filter_matches_substring_case_insensitively() {
        let mut app = app_with(&[("work", true, 3), ("scratch", false, 1)]);
        app.query = "WOR".into();
        app.refilter();
        assert_eq!(app.filtered.len(), 1);
        assert_eq!(app.sessions[app.filtered[0]].name, "work");
    }

    #[test]
    fn selected_tracks_the_highlighted_row() {
        let mut app = app_with(&[("a", true, 3), ("b", false, 2), ("c", false, 1)]);
        app.list.select(Some(2));
        assert_eq!(app.selected().unwrap().name, "c");
        app.move_sel(1); // wraps to 0
        assert_eq!(app.selected().unwrap().name, "a");
    }
}
