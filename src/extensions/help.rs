//! `ztmux --help` / `-h` — the cyberpunk help screen.
//!
//! Not part of upstream tmux (tmux only has the terse `usage()` in
//! [`crate::tmux::usage`]); this is an original ztmux feature, added so
//! `ztmux --help` reads like the rest of the toolchain (`tp --help`). It lives
//! here under `src/extensions/` — and is therefore exempt from the anti-drift
//! C-name gate (`tests/ported_fn_names_match_c.rs`) — precisely because it has
//! no tmux C counterpart. Prints to stdout and exits 0.

use crate::tmux::getversion;

/// Print the cyberpunk help screen and exit 0.
pub(crate) fn help() -> ! {
    // ANSI Shadow banner (figlet -f "ANSI Shadow" ZTMUX), gradient cyan→magenta→red.
    print!(concat!(
        "\x1b[36m ███████╗████████╗███╗   ███╗██╗   ██╗██╗  ██╗\x1b[0m\n",
        "\x1b[36m ╚══███╔╝╚══██╔══╝████╗ ████║██║   ██║╚██╗██╔╝\x1b[0m\n",
        "\x1b[35m   ███╔╝    ██║   ██╔████╔██║██║   ██║ ╚███╔╝ \x1b[0m\n",
        "\x1b[35m  ███╔╝     ██║   ██║╚██╔╝██║██║   ██║ ██╔██╗ \x1b[0m\n",
        "\x1b[31m ███████╗   ██║   ██║ ╚═╝ ██║╚██████╔╝██╔╝ ██╗\x1b[0m\n",
        "\x1b[31m ╚══════╝   ╚═╝   ╚═╝     ╚═╝ ╚═════╝ ╚═╝  ╚═╝\x1b[0m\n",
    ));
    println!("\x1b[36m ┌──────────────────────────────────────────────────────┐\x1b[0m");
    println!(
        "\x1b[36m │ STATUS: ONLINE  // SIGNAL: ████████░░ // v{}\x1b[36m   │\x1b[0m",
        getversion()
    );
    println!("\x1b[36m └──────────────────────────────────────────────────────┘\x1b[0m");
    println!("\x1b[35m  >> TERMINAL MULTIPLEXER // TMUX-COMPATIBLE <<\x1b[0m");
    println!();
    println!("A terminal multiplexer: run and switch many terminals in one screen.");
    println!();
    println!("\x1b[33m  USAGE:\x1b[0m ztmux [OPTIONS] [command [flags]]");
    println!();
    println!("\x1b[36m  ── SERVER ─────────────────────────────────────────────\x1b[0m");
    println!("  -L <socket-name>   \x1b[32m//\x1b[0m Server socket name under the socket dir");
    println!("  -S <socket-path>   \x1b[32m//\x1b[0m Full path to the server socket");
    println!("  -N                 \x1b[32m//\x1b[0m Do not start the server");
    println!("  -D                 \x1b[32m//\x1b[0m Run in the foreground (do not daemonize)");
    println!("\x1b[36m  ── SESSION ────────────────────────────────────────────\x1b[0m");
    println!(
        "  -c <shell-command> \x1b[32m//\x1b[0m Execute shell-command using the default shell"
    );
    println!("  -f <file>          \x1b[32m//\x1b[0m Load an alternate configuration file");
    println!("  -l                 \x1b[32m//\x1b[0m Behave as a login shell");
    println!("\x1b[36m  ── TERMINAL ───────────────────────────────────────────\x1b[0m");
    println!("  -2                 \x1b[32m//\x1b[0m Force 256-colour support");
    println!("  -C                 \x1b[32m//\x1b[0m Control mode (-CC also disables echo)");
    println!("  -T <features>      \x1b[32m//\x1b[0m Set terminal features (comma-separated)");
    println!("  -u                 \x1b[32m//\x1b[0m Force UTF-8");
    println!("\x1b[36m  ── SYSTEM ─────────────────────────────────────────────\x1b[0m");
    println!(
        "  -v                 \x1b[32m//\x1b[0m Increase logging verbosity (repeat up to -vvvv)"
    );
    println!("  -V, --version      \x1b[32m//\x1b[0m Print version and exit");
    println!("  -h, --help         \x1b[32m//\x1b[0m Print this help and exit");
    println!("\x1b[36m  ── POSITIONAL ─────────────────────────────────────────\x1b[0m");
    println!(
        "  [command [flags]]  \x1b[32m//\x1b[0m tmux command to run (default: attach/new session)"
    );
    println!();
    println!(
        "\x1b[35m  ztmux {} \x1b[0m// \x1b[33m(c) Jacob Menke and contributors\x1b[0m",
        getversion()
    );
    println!("\x1b[33m  >>> JACK IN. SPLIT YOUR PANES. OWN YOUR SESSIONS. <<<\x1b[0m");
    println!("\x1b[36m ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░\x1b[0m");
    std::process::exit(0)
}
