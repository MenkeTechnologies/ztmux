//! `ztmux open` — pick a URL or file path out of the current pane and open it.
//!
//! Captures the active pane (visible screen + recent scrollback), scans it for
//! URLs and file paths, and shows a ratatui picker. Enter opens the selection —
//! a URL in the OS opener (`open`/`xdg-open`), a file in `$EDITOR` (at its line
//! if the token is `file:line`), a directory revealed in the file manager. `y`
//! copies it instead. Like tmux-open / tmux-urlview, but built in.
//!
//! A standalone client subcommand (reads via `capture-pane`, acts via the OS and
//! `ztmux set-buffer`); best bound to a key or run as `:open` in a popup.

use std::collections::HashSet;

use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use super::tmux_query::ztmux_cmd;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Url,
    Path,
}

#[derive(Clone)]
struct Item {
    text: String,
    kind: Kind,
}

pub(crate) fn run(socket: &str) -> i32 {
    // Capture the active pane: visible screen plus a few screens of scrollback,
    // wrapped lines joined so a long URL split across rows is seen whole.
    let out = ztmux_cmd(socket, &["capture-pane", "-p", "-J", "-S", "-400"]).output();
    let text = out
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let mut items = extract(&text);
    // Newest first: pane output reads top-to-bottom, so the most recent links are
    // at the bottom — reverse so they land at the top of the list.
    items.reverse();
    if items.is_empty() {
        eprintln!("open: no URLs or paths found in the current pane");
        return 1;
    }
    App::new(socket.to_string(), items).main()
}

// ---- extraction ----------------------------------------------------------

/// Pull URLs and paths out of captured pane text (whitespace-delimited tokens,
/// with surrounding brackets/quotes and trailing punctuation trimmed).
fn extract(text: &str) -> Vec<Item> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for raw in text.split_whitespace() {
        let t = trim_token(raw);
        if t.len() < 4 {
            continue;
        }
        if let Some(kind) = classify(t)
            && seen.insert(t.to_string())
        {
            out.push(Item {
                text: t.to_string(),
                kind,
            });
        }
    }
    out
}

/// Strip wrapping delimiters and trailing sentence punctuation, but keep a
/// `:line[:col]` suffix (editor jump targets).
fn trim_token(s: &str) -> &str {
    let mut t = s.trim_matches(|c| {
        matches!(
            c,
            '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | '"' | '\'' | '`' | ','
        )
    });
    // Trailing sentence punctuation, but not a digit-terminated `:line`.
    while let Some(last) = t.chars().last() {
        if matches!(last, '.' | ';' | '!' | '?' | ':') {
            t = &t[..t.len() - last.len_utf8()];
        } else {
            break;
        }
    }
    t
}

fn classify(t: &str) -> Option<Kind> {
    const SCHEMES: &[&str] = &[
        "http://", "https://", "ftp://", "file://", "git://", "ssh://",
    ];
    if SCHEMES.iter().any(|s| t.starts_with(s)) || t.starts_with("www.") || t.starts_with("git@") {
        return Some(Kind::Url);
    }
    if looks_like_path(t) {
        return Some(Kind::Path);
    }
    None
}

/// A path is `/…`, `~/…`, `./…`, `../…`, or a `word/word…` with a `/` inside and
/// only path-ish characters (so prose with a slash, like "and/or", is excluded
/// by requiring a dot or a deeper path).
fn looks_like_path(t: &str) -> bool {
    let core = t.split(':').next().unwrap_or(t); // drop :line for the shape test
    if !core.contains('/') {
        return false;
    }
    if !core
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '~' | '+' | '@'))
    {
        return false;
    }
    if core.starts_with('/')
        || core.starts_with("~/")
        || core.starts_with("./")
        || core.starts_with("../")
    {
        return true;
    }
    // Relative like `src/main.rs`: require a dotted last segment or two slashes.
    core.matches('/').count() >= 2 || core.rsplit('/').next().is_some_and(|f| f.contains('.'))
}

// ---- open / copy ---------------------------------------------------------

/// The OS "open this" command: macOS `open`, else `xdg-open`.
fn os_opener() -> &'static str {
    if std::env::consts::OS == "macos" {
        "open"
    } else {
        "xdg-open"
    }
}

/// Split a `file:line[:col]` token into (path, Some(line)).
fn split_line(t: &str) -> (&str, Option<&str>) {
    let mut parts = t.splitn(3, ':');
    let path = parts.next().unwrap_or(t);
    let line = parts
        .next()
        .filter(|l| l.chars().all(|c| c.is_ascii_digit()));
    (path, line)
}

impl Item {
    /// Open the item: URL in the browser/opener, a file in $EDITOR (at its line),
    /// a directory revealed. Returns an error string on failure.
    fn open(&self) -> Result<(), String> {
        use std::process::Command;
        match self.kind {
            Kind::Url => Command::new(os_opener())
                .arg(&self.text)
                .spawn()
                .map(|_| ())
                .map_err(|e| format!("{}: {e}", os_opener())),
            Kind::Path => {
                let (path, line) = split_line(&self.text);
                let editor = std::env::var("EDITOR").or_else(|_| std::env::var("VISUAL"));
                match editor {
                    Ok(ed) if !ed.trim().is_empty() => {
                        // Foreground in the popup's tty; `+N` jumps to the line in
                        // vi/nano/emacs-style editors.
                        let mut cmd = Command::new(ed.trim());
                        if let Some(l) = line {
                            cmd.arg(format!("+{l}"));
                        }
                        cmd.arg(path)
                            .status()
                            .map(|_| ())
                            .map_err(|e| format!("editor: {e}"))
                    }
                    _ => Command::new(os_opener())
                        .arg(path)
                        .spawn()
                        .map(|_| ())
                        .map_err(|e| format!("{}: {e}", os_opener())),
                }
            }
        }
    }
}

/// Copy text to the tmux buffer and, if available, the OS clipboard.
fn copy(socket: &str, text: &str) {
    let _ = ztmux_cmd(socket, &["set-buffer", text]).status();
    let clip = if std::env::consts::OS == "macos" {
        Some(("pbcopy", vec![]))
    } else if which("wl-copy") {
        Some(("wl-copy", vec![]))
    } else if which("xclip") {
        Some(("xclip", vec!["-selection", "clipboard"]))
    } else {
        None
    };
    if let Some((bin, args)) = clip
        && let Ok(mut child) = std::process::Command::new(bin)
            .args(&args)
            .stdin(std::process::Stdio::piped())
            .spawn()
    {
        use std::io::Write;
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
    }
}

fn which(bin: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|d| d.join(bin).is_file()))
}

// ---- picker --------------------------------------------------------------

struct App {
    socket: String,
    items: Vec<Item>,
    filtered: Vec<usize>,
    query: String,
    list: ListState,
    status: String,
    quit: bool,
}

impl App {
    fn new(socket: String, items: Vec<Item>) -> Self {
        let filtered = (0..items.len()).collect();
        let mut list = ListState::default();
        if !items.is_empty() {
            list.select(Some(0));
        }
        App {
            socket,
            items,
            filtered,
            query: String::new(),
            list,
            status: String::new(),
            quit: false,
        }
    }

    fn refilter(&mut self) {
        let q = self.query.to_lowercase();
        self.filtered = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, it)| q.is_empty() || it.text.to_lowercase().contains(&q))
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

    fn selected(&self) -> Option<&Item> {
        self.list
            .selected()
            .and_then(|i| self.filtered.get(i))
            .and_then(|&idx| self.items.get(idx))
    }

    fn main(mut self) -> i32 {
        let mut terminal = ratatui::init();
        let res = (|| -> std::io::Result<()> {
            while !self.quit {
                terminal.draw(|f| ui(f, &mut self))?;
                if let Event::Key(k) = event::read()?
                    && k.kind == KeyEventKind::Press
                {
                    if k.modifiers.contains(KeyModifiers::CONTROL)
                        && matches!(k.code, KeyCode::Char('c'))
                    {
                        self.quit = true;
                        continue;
                    }
                    match k.code {
                        KeyCode::Esc => self.quit = true,
                        KeyCode::Enter => {
                            if let Some(it) = self.selected().cloned() {
                                // Restore the terminal before handing the tty to
                                // an editor; re-init if the editor returns.
                                ratatui::restore();
                                match it.open() {
                                    Ok(()) => return Ok(()),
                                    Err(e) => {
                                        terminal = ratatui::init();
                                        self.status = e;
                                    }
                                }
                            }
                        }
                        KeyCode::Char('y') if !k.modifiers.contains(KeyModifiers::CONTROL) => {
                            if let Some(it) = self.selected().map(|i| i.text.clone()) {
                                copy(&self.socket, &it);
                                self.status = format!("copied: {it}");
                            }
                        }
                        KeyCode::Down => self.move_sel(1),
                        KeyCode::Up => self.move_sel(-1),
                        KeyCode::Backspace => {
                            self.query.pop();
                            self.refilter();
                        }
                        KeyCode::Char(c) => {
                            self.query.push(c);
                            self.refilter();
                        }
                        _ => {}
                    }
                }
            }
            Ok(())
        })();
        ratatui::restore();
        match res {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("open error: {e}");
                1
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(f.area());

    let query = Paragraph::new(Line::from(vec![
        Span::styled("❯ ", Style::default().fg(Color::Cyan)),
        Span::raw(app.query.clone()),
        Span::styled("▏", Style::default().fg(Color::DarkGray)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" open ({}) ", app.filtered.len())),
    );
    f.render_widget(query, rows[0]);

    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .map(|&i| {
            let it = &app.items[i];
            let (tag, col) = match it.kind {
                Kind::Url => ("url ", Color::Cyan),
                Kind::Path => ("path", Color::Green),
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{tag} "), Style::default().fg(col)),
                Span::raw(it.text.clone()),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" enter open · y copy "),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Indexed(237))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, rows[1], &mut app.list);

    let help = if app.status.is_empty() {
        " type filter · ↑/↓ move · enter open · y copy · esc quit".to_string()
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
    use super::*;

    fn kinds(text: &str) -> Vec<(String, Kind)> {
        extract(text)
            .into_iter()
            .map(|i| (i.text, i.kind))
            .collect()
    }

    #[test]
    fn extracts_urls_and_strips_punctuation() {
        let got = kinds("see https://github.com/foo/bar, and http://x.io.");
        assert_eq!(got[0].0, "https://github.com/foo/bar");
        assert_eq!(got[0].1, Kind::Url);
        assert_eq!(got[1].0, "http://x.io");
    }

    #[test]
    fn extracts_paths_with_line_numbers() {
        let got = kinds("edit src/main.rs:42 or /etc/hosts or ~/.zshrc");
        let texts: Vec<&str> = got.iter().map(|(t, _)| t.as_str()).collect();
        assert!(texts.contains(&"src/main.rs:42"));
        assert!(texts.contains(&"/etc/hosts"));
        assert!(texts.contains(&"~/.zshrc"));
    }

    #[test]
    fn ignores_prose_and_bare_words() {
        let got = kinds("this and/or that, a normal sentence with no links");
        assert!(got.is_empty(), "got {got:?}");
    }

    #[test]
    fn split_line_parses_editor_target() {
        assert_eq!(split_line("src/main.rs:42"), ("src/main.rs", Some("42")));
        assert_eq!(split_line("/etc/hosts"), ("/etc/hosts", None));
    }
}
