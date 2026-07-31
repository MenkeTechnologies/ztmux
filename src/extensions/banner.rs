//! `ztmux banner` — the console banner as a verb.
//!
//! [`super::repl`] opens with a banner: the ZTMUX logo, a boxed summary of the
//! verb totals, and the live counts for the socket it targets. That was only
//! reachable by starting the console, and it scrolled away with the first
//! screenful of output. It is a verb now, so `ztmux banner` prints it from a
//! shell and the console's `banner` builtin redraws it in place.
//!
//! Nothing here needs a server: with none running the counts line says so
//! instead, so the banner is also the shortest "is anything up on this socket"
//! check there is.

use crate::cmd_::CMD_TABLE;
use crate::tmux::getversion;

use super::tmux_query::{Snapshot, poll};
use super::verbs::{colored, paint, strip_ansi};

pub(crate) fn run(socket: &str) -> i32 {
    print_banner(socket);
    0
}

/// The banner: the ZTMUX logo, a boxed summary line (version and verb totals),
/// and the live server counts for the socket it is printed for.
pub(crate) fn print_banner(socket: &str) {
    let color = colored();
    let logo = super::help::LOGO;
    if color {
        print!("{logo}");
    } else {
        print!("{}", strip_ansi(logo));
    }

    let commands = CMD_TABLE.len();
    let extensions = super::EXTENSION_COMMANDS.len();
    let summary = format!(
        " ZTMUX // v{} // {commands} commands // {extensions} extensions ",
        getversion()
    );
    for line in box_lines(&summary, color) {
        println!("{line}");
    }

    let snap = poll(socket);
    println!("{}", server_line(socket, &snap, color));
}

/// One-line server summary under the banner: the socket and its live counts,
/// or the reason the counts are missing.
fn server_line(socket: &str, snap: &Snapshot, color: bool) -> String {
    let socket = if socket.is_empty() { "default" } else { socket };
    let head = paint(&format!(" socket {socket}"), "2", color);
    match &snap.error {
        Some(_) => format!("{head}  {}", paint("// no server running", "31", color)),
        None => format!(
            "{head}  {}  {}  {}  {}",
            count(snap.sessions.len(), "session"),
            count(snap.windows.len(), "window"),
            count(snap.panes.len(), "pane"),
            count(snap.clients.len(), "client"),
        ),
    }
}

/// `1 session` / `2 sessions` — every count in the banner is plural-correct.
fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("{n} {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// The three lines of a single-line box around `line`, sized to its *printed*
/// width (colour escapes excluded), so the borders stay aligned however long
/// the version or the verb counts get.
fn box_lines(line: &str, color: bool) -> [String; 3] {
    let rule: String = "─".repeat(strip_ansi(line).chars().count());
    let border = |s: &str| paint(s, "36", color);
    [
        border(&format!(" ┌{rule}┐")),
        format!("{}{line}{}", border(" │"), border("│")),
        border(&format!(" └{rule}┘")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_box_borders_match_the_content_width() {
        // The box is measured on printed width, so neither a longer version
        // string nor the colour escapes can push the right border out of line.
        for content in [
            " ZTMUX // v3.7.32 // 91 commands // 111 extensions ",
            " ZTMUX // v10.100.1000 // 1 command // 1 extension ",
            "",
        ] {
            for color in [false, true] {
                let widths: Vec<usize> = box_lines(content, color)
                    .iter()
                    .map(|l| strip_ansi(l).chars().count())
                    .collect();
                assert_eq!(
                    widths[0], widths[1],
                    "top border and content disagree for {content:?} (color: {color})"
                );
                assert_eq!(
                    widths[1], widths[2],
                    "content and bottom border disagree for {content:?} (color: {color})"
                );
            }
        }
        assert_eq!(strip_ansi("\x1b[36m├─┤\x1b[0m").chars().count(), 3);
    }

    #[test]
    fn server_line_reports_a_dead_server_instead_of_zero_counts() {
        let snap = Snapshot {
            error: Some("no server running".into()),
            ..Default::default()
        };
        assert!(server_line("/tmp/sock", &snap, false).contains("no server running"));
        let mut live = Snapshot::default();
        assert!(server_line("", &live, false).contains("0 sessions"));
        live.sessions.push(super::super::tmux_query::Session::default());
        assert!(server_line("", &live, false).contains("1 session  "));
    }
}
