//! `ztmux shadow [<dir>] [-n|--print] [--all]` — install the `~/.ztmux` shadow
//! and print the shell lines that activate it.
//!
//! ztmux is a from-source port of the whole multiplexer, so it can stand in for
//! `tmux` itself. Shadowing is opt-in, and this verb is the whole opt-in: it
//! installs a `tmux` shim beside a `ztmux` one in `<dir>` (default
//! `~/.ztmux/bin`), the man pages under `~/.ztmux/man` (`ztmux.1`, `ztmuxall.1`
//! and a `tmux.1` copy, so `man tmux` reads this port's page), and the zsh
//! completion under `~/.ztmux/completions` (`_ztmux`, plus a `_tmux` wrapper
//! that shadows the system one by file name). Everything installed is compiled
//! into the binary, so the install needs nothing from the source tree and works
//! from a `cargo install`ed copy.
//!
//! stdout carries only shell code, so `eval "$(ztmux shadow)"` sets the current
//! shell up and the same lines paste into an rc file; the install summary goes
//! to stderr. A `PATH`/`MANPATH` line the environment already satisfies is
//! emitted commented out (so re-evaluating never duplicates an entry, and the
//! line is still there to uncomment when pasting into `.zshrc`); `--all` prints
//! every line uncommented. `fpath` is a zsh variable, not an exported one, so
//! this process cannot see it — that line is always emitted live.
//!
//! Nothing is ever clobbered: a real (non-symlink) file holding a shim's name
//! is left alone, so pointing `<dir>` at a directory that already carries the
//! real `tmux` never replaces it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The man pages shipped in the tree, embedded so the installed shadow is
/// self-contained.
const MAN_ZTMUX: &str = include_str!("../../man/man1/ztmux.1");
const MAN_ZTMUXALL: &str = include_str!("../../man/man1/ztmuxall.1");

/// The zsh completion shipped in the tree (`completions/_ztmux`): full tmux
/// depth plus the ztmux extensions. Embedded for the same reason.
const COMPLETION: &str = include_str!("../../completions/_ztmux");

/// The `_tmux` wrapper installed beside `_ztmux`. It shadows the system `_tmux`
/// by file name (compinit takes the first file of a given name on `fpath`) and
/// delegates, so the shimmed `tmux` completes every tmux command *and* every
/// ztmux extension. `_ztmux` is autoloadable because the file beside it carries
/// `#compdef ztmux`.
const COMPLETION_TMUX: &str = "\
#compdef tmux

# Installed by `ztmux shadow`: `tmux` on PATH is the ztmux shim, so complete it
# with ztmux's own completion (full tmux depth plus the ztmux extensions).
_ztmux \"$@\"
";

pub(crate) fn run(_socket: &str) -> i32 {
    let args = verb_args();
    let print_only = args.iter().any(|a| a == "-n" || a == "--print");
    let all = args.iter().any(|a| a == "--all");

    let Some(home) = super::diagnostics::path() else {
        eprintln!("ztmux: shadow: $HOME is unset — nowhere to install");
        return 1;
    };
    let bin = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .map_or_else(|| home.join("bin"), PathBuf::from);
    let man = home.join("man");
    let comp = home.join("completions");

    if !print_only && let Err(err) = install(&bin, &man, &comp) {
        eprintln!("ztmux: shadow: {err}");
        return 1;
    }

    // stdout is shell code only — the install summary went to stderr.
    emit(
        "PATH",
        &format!(r#"export PATH="{}:$PATH""#, shell_path(&bin)),
        &bin,
        all,
    );
    emit(
        "MANPATH",
        &format!(r#"export MANPATH="{}:$MANPATH""#, shell_path(&man)),
        &man,
        all,
    );
    println!(
        r#"fpath=("{}" $fpath)  # zsh: before compinit"#,
        shell_path(&comp)
    );
    0
}

/// The words after the verb: `ztmux -L work shadow -n` → `["-n"]`, so a socket
/// flag before the verb is never read as the install directory.
fn verb_args() -> Vec<String> {
    std::env::args()
        .skip_while(|a| a != "shadow")
        .skip(1)
        .collect()
}

/// Install the shims, the man pages and the completion, reporting one summary
/// line on stderr.
fn install(bin: &Path, man: &Path, comp: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(bin)?;

    // The `ztmux` shim first, then `tmux` linked to it *by name*: a rebuild at
    // a new path only needs that one link repointed.
    let me = std::env::current_exe()?;
    let mut links = LinkStats::default();
    link_to(&bin.join("ztmux"), &me, &mut links)?;
    link_to(&bin.join("tmux"), Path::new("ztmux"), &mut links)?;

    let pages = install_man(man)?;
    let completions = install_completions(comp)?;

    eprintln!(
        "shadow: {} ({} link(s) new / {} current / {} left alone: real file); \
         {} man page(s) in {} ({} written); {} completion(s) in {} ({} written)",
        bin.join("tmux").display(),
        links.created,
        links.current,
        links.skipped,
        pages.total,
        man.join("man1").display(),
        pages.written,
        completions.total,
        comp.display(),
        completions.written,
    );
    Ok(())
}

/// Write the embedded man pages into `<man>/man1`. `tmux.1` is the same page as
/// `ztmux.1`: with `<man>` first on `MANPATH`, `man tmux` documents what `tmux`
/// now runs, which is the manual half of the shadow.
fn install_man(man: &Path) -> std::io::Result<Written> {
    let dir = man.join("man1");
    std::fs::create_dir_all(&dir)?;
    let mut done = Written::default();
    for (name, text) in [
        ("ztmux.1", MAN_ZTMUX),
        ("ztmuxall.1", MAN_ZTMUXALL),
        ("tmux.1", MAN_ZTMUX),
    ] {
        done.record(write_if_changed(&dir.join(name), text)?);
    }
    Ok(done)
}

/// Write `_ztmux` and its `_tmux` wrapper into `comp`.
fn install_completions(comp: &Path) -> std::io::Result<Written> {
    std::fs::create_dir_all(comp)?;
    let mut done = Written::default();
    for (name, text) in [("_ztmux", COMPLETION), ("_tmux", COMPLETION_TMUX)] {
        done.record(write_if_changed(&comp.join(name), text)?);
    }
    Ok(done)
}

/// How many files an install step covered, and how many it had to write.
#[derive(Default)]
struct Written {
    total: usize,
    written: usize,
}

impl Written {
    fn record(&mut self, wrote: bool) {
        self.total += 1;
        self.written += usize::from(wrote);
    }
}

/// Write `text` to `path` unless the file already holds exactly that; returns
/// whether it wrote. The man page and the completion are ~100 KiB each, so a
/// no-op re-run should not rewrite them.
fn write_if_changed(path: &Path, text: &str) -> std::io::Result<bool> {
    if std::fs::read_to_string(path).is_ok_and(|on_disk| on_disk == text) {
        return Ok(false);
    }
    std::fs::write(path, text)?;
    Ok(true)
}

/// What the shim install did: links created, links already pointing at the
/// target, and names left untouched because a real file holds them.
#[derive(Default)]
struct LinkStats {
    created: usize,
    current: usize,
    skipped: usize,
}

/// Point `link` at `target`, idempotently: a correct symlink is left alone, a
/// stale one is repointed, and a real (non-symlink) file is never clobbered —
/// so `ztmux shadow /usr/local/bin` cannot replace an installed `tmux`.
fn link_to(link: &Path, target: &Path, stats: &mut LinkStats) -> std::io::Result<()> {
    match std::fs::symlink_metadata(link) {
        Ok(m) if m.file_type().is_symlink() => {
            if std::fs::read_link(link).ok().as_deref() == Some(target) {
                stats.current += 1;
                return Ok(());
            }
            std::fs::remove_file(link)?; // stale target — repointed below
        }
        Ok(_) => {
            stats.skipped += 1; // a real file or directory: leave it alone
            return Ok(());
        }
        Err(_) => {} // absent — created below
    }
    std::os::unix::fs::symlink(target, link)?;
    stats.created += 1;
    Ok(())
}

/// Print one shell line, commented out when `var` already lists `dir` (and
/// `--all` was not given), so an `eval` cannot duplicate the entry.
fn emit(var: &str, code: &str, dir: &Path, all: bool) {
    if !all && path_list_contains(var, dir) {
        println!("# {code}  # already on {var}");
    } else {
        println!("{code}");
    }
}

/// Whether the `var` search path (`PATH`, `MANPATH`) already lists `dir`,
/// compared on resolved paths so one entry written two ways counts once. Also
/// used by [`super::doctor`] to report whether the installed shadow is active.
pub(crate) fn path_list_contains(var: &str, dir: &Path) -> bool {
    list_contains(std::env::var_os(var), dir)
}

/// [`path_list_contains`] over an explicit value, so the matching is testable
/// without touching the process environment.
fn list_contains(value: Option<OsString>, dir: &Path) -> bool {
    let Some(value) = value else {
        return false;
    };
    let target = canon(dir);
    std::env::split_paths(&value).any(|entry| canon(&entry) == target)
}

/// `path` resolved through symlinks when it exists, left as written when it
/// does not (a `PATH` entry naming a missing directory still compares equal to
/// itself).
pub(crate) fn canon(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// A path written for a shell rc file: `$HOME`-relative when it is under
/// `$HOME` (so the line is portable across machines), absolute otherwise.
/// Always emitted inside double quotes, where `$HOME` expands.
fn shell_path(p: &Path) -> String {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match home.as_ref().and_then(|h| p.strip_prefix(h).ok()) {
        Some(rest) => format!("$HOME/{}", rest.display()),
        None => p.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch directory of this test's own, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("ztmux-shadow-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("scratch dir");
            Scratch(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_second_install_rewrites_nothing() {
        let scratch = Scratch::new("idempotent");
        let comp = scratch.0.join("completions");

        let first = install_completions(&comp).expect("install");
        assert_eq!((first.total, first.written), (2, 2));

        // Re-running the install must not rewrite ~100 KiB of completion that
        // is already byte-identical on disk.
        let second = install_completions(&comp).expect("reinstall");
        assert_eq!((second.total, second.written), (2, 0));

        // A hand-edited file is repaired, not left diverged.
        std::fs::write(comp.join("_tmux"), "stale\n").expect("clobber");
        let third = install_completions(&comp).expect("repair");
        assert_eq!((third.total, third.written), (2, 1));
        assert_eq!(
            std::fs::read_to_string(comp.join("_tmux")).expect("read"),
            COMPLETION_TMUX
        );
    }

    #[test]
    fn a_real_file_keeps_its_name_and_a_stale_link_is_repointed() {
        let scratch = Scratch::new("links");
        let bin = &scratch.0;

        // A real `tmux` in the target directory is never replaced — the whole
        // reason `ztmux shadow /usr/local/bin` is safe to run.
        std::fs::write(bin.join("tmux"), "#!/bin/sh\n").expect("real tmux");
        let mut stats = LinkStats::default();
        link_to(&bin.join("tmux"), Path::new("ztmux"), &mut stats).expect("link");
        assert_eq!((stats.created, stats.current, stats.skipped), (0, 0, 1));
        assert_eq!(
            std::fs::read_to_string(bin.join("tmux")).expect("read"),
            "#!/bin/sh\n"
        );

        // A link this run made is left alone on the next one, and a link left
        // pointing at an old build is repointed.
        let mut stats = LinkStats::default();
        link_to(&bin.join("ztmux"), Path::new("/new/ztmux"), &mut stats).expect("create");
        link_to(&bin.join("ztmux"), Path::new("/new/ztmux"), &mut stats).expect("current");
        assert_eq!((stats.created, stats.current, stats.skipped), (1, 1, 0));

        let mut stats = LinkStats::default();
        link_to(&bin.join("ztmux"), Path::new("/newer/ztmux"), &mut stats).expect("repoint");
        assert_eq!((stats.created, stats.current, stats.skipped), (1, 0, 0));
        assert_eq!(
            std::fs::read_link(bin.join("ztmux")).expect("read_link"),
            Path::new("/newer/ztmux")
        );
    }

    #[test]
    fn a_path_entry_written_another_way_still_counts_as_present() {
        let scratch = Scratch::new("pathlist");
        let bin = scratch.0.join("bin");
        std::fs::create_dir_all(&bin).expect("bin");

        let list = |s: &str| Some(OsString::from(s));
        assert!(list_contains(list(&bin.display().to_string()), &bin));
        // Trailing slash and a `.` hop name the same directory once resolved.
        assert!(list_contains(
            list(&format!("/usr/bin:{}/", bin.display())),
            &bin
        ));
        assert!(list_contains(list(&format!("{}/./", bin.display())), &bin));
        assert!(!list_contains(list("/usr/bin:/bin"), &bin));
        assert!(!list_contains(None, &bin));
    }

    #[test]
    fn rc_lines_are_home_relative_so_they_paste_into_zshrc() {
        let home = PathBuf::from(std::env::var_os("HOME").expect("HOME"));
        assert_eq!(shell_path(&home.join(".ztmux/bin")), "$HOME/.ztmux/bin");
        // Outside $HOME the absolute path is the only portable form.
        assert_eq!(shell_path(Path::new("/opt/bin")), "/opt/bin");
    }

    #[test]
    fn the_install_directory_is_read_from_the_words_after_the_verb() {
        // `verb_args` reads the real argv, so exercise its rule directly: the
        // first non-flag word is the directory, flags anywhere are flags.
        let dir_of = |args: &[&str]| -> Option<String> {
            args.iter()
                .find(|a| !a.starts_with('-'))
                .map(|a| (*a).to_string())
        };
        assert_eq!(dir_of(&["-n"]), None);
        assert_eq!(dir_of(&["/usr/local/bin"]), Some("/usr/local/bin".into()));
        assert_eq!(dir_of(&["--all", "/opt/bin"]), Some("/opt/bin".into()));
    }

    // An apostrophe inside a single-quoted `_arguments` spec ends the quote and
    // leaves the whole file unparseable — which kills zsh completion for every
    // verb, not just the one with the typo, and the shadow installs that file
    // verbatim. (`--run[re-run each pane's saved command]` did exactly that.)
    #[test]
    fn every_extension_spec_the_shadow_installs_is_quoted_shut() {
        const MARKER: &str = "# ── ztmux client extensions";
        let extensions = COMPLETION.split_once(MARKER).expect("extension section").1;
        let unbalanced: Vec<&str> = extensions
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with('\'') && line.matches('\'').count() % 2 == 1)
            .collect();
        assert!(
            unbalanced.is_empty(),
            "unterminated quote in: {unbalanced:?}"
        );
    }

    #[test]
    fn the_completion_wrapper_shadows_tmux_and_delegates_to_ztmux() {
        assert!(COMPLETION.starts_with("#compdef ztmux"));
        assert!(COMPLETION_TMUX.starts_with("#compdef tmux\n"));
        assert!(COMPLETION_TMUX.contains("_ztmux \"$@\""));
        // The embedded pages are the real ones, not an empty include.
        assert!(MAN_ZTMUX.starts_with(".TH ZTMUX 1"));
        assert!(MAN_ZTMUXALL.starts_with(".\\\""));
    }
}
