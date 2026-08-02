//! Shell builtins for the [`super::repl`] console — `cd`, `pwd`, `dir`, `cat`,
//! `echo`, `export`, `printenv`, `unset`, `mkdir`, `touch`, `rm`, `cp`, `mv`,
//! `ln`.
//!
//! Ported from the zvcs console's shell verbs (`git zcd`, `zpwd`, `zls`,
//! `zenv`, …), which exist so that console never has to be left to move around
//! the filesystem. Two things change in the port:
//!
//!   * **They run in the console process.** ztmux re-invokes the binary for
//!     every line, so a spawned `cd` would move a child's directory and then
//!     exit. These run in-process instead, which is also what makes them
//!     useful: the directory and environment they mutate are inherited by
//!     every line spawned afterwards, so `cd ~/src/app` then `new-window`
//!     opens the window there, and `export FOO=bar` then `run-shell 'echo
//!     $FOO'` sees it.
//!   * **The names dodge tmux's.** The listing is `dir`, because `ls` is
//!     already the alias for `list-sessions`; the environment builtins are
//!     `export`/`printenv`/`unset`, because `setenv` and `showenv` are the
//!     aliases for `set-environment`/`show-environment`. A test holds the
//!     whole set clear of every command, alias and extension verb.
//!
//! `dir` is the plain half of zvcs's `zls`: the same `-a`/`-l`/`-r`/`-t` flags,
//! permission string, human size and relative mtime, without the per-file git
//! status column (there is no repository behind a tmux socket) and coloured by
//! entry kind rather than from `LS_COLORS`.

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::age::ago;
use super::verbs::{colored, paint};

/// What runs a shell builtin: its arguments, and the message to print when it
/// fails. A failure never leaves the console.
pub(crate) type Builtin = fn(&[String]) -> Result<(), String>;

/// The shell builtins: the name, the description [`super::verbs`] lists and the
/// console's `help` prints, and the function that runs it. Sorted, so
/// [`lookup`] can binary-search it. Carrying the function here rather than
/// matching on the name elsewhere is what keeps an advertised builtin from
/// being one the console cannot actually run.
pub(crate) const BUILTINS: &[(&str, &str, Builtin)] = &[
    ("cat", "print files to stdout (console builtin)", cat),
    (
        "cd",
        "change the console's working directory (console builtin)",
        cd,
    ),
    (
        "cp",
        "copy files, or directories with -r (console builtin)",
        cp,
    ),
    (
        "dir",
        "list a directory, -a all -l long -t by time -r reversed (console builtin)",
        dir,
    ),
    (
        "echo",
        "print the arguments, -n without a newline (console builtin)",
        echo,
    ),
    (
        "export",
        "set an environment variable for later lines (console builtin)",
        export,
    ),
    (
        "ln",
        "create a hard link, or a symlink with -s (console builtin)",
        ln,
    ),
    (
        "mkdir",
        "create directories, -p with parents (console builtin)",
        mkdir,
    ),
    ("mv", "move or rename files (console builtin)", mv),
    (
        "printenv",
        "print the environment, or one variable (console builtin)",
        printenv,
    ),
    (
        "pwd",
        "print the console's working directory (console builtin)",
        pwd,
    ),
    (
        "rm",
        "remove files, -r directories -f ignore missing (console builtin)",
        rm,
    ),
    (
        "touch",
        "create files, or bump their mtime (console builtin)",
        touch,
    ),
    (
        "unset",
        "remove environment variables (console builtin)",
        unset,
    ),
];

/// The function that runs `verb`, if it is a shell builtin — i.e. runs in the
/// console process rather than as a spawned `ztmux <line>`.
pub(crate) fn lookup(verb: &str) -> Option<Builtin> {
    BUILTINS
        .binary_search_by(|(name, _, _)| (*name).cmp(verb))
        .ok()
        .map(|i| BUILTINS[i].2)
}

/// The listable half of [`BUILTINS`]: name and description, for the `verbs`
/// listing and the console's `help`.
pub(crate) fn described() -> impl Iterator<Item = (&'static str, &'static str)> {
    BUILTINS
        .iter()
        .map(|&(name, description, _)| (name, description))
}

/// The flags `verb` accepts, for the console's Tab completion of a `-` word.
/// Empty for a builtin that takes none; [`is_builtin`] separates that from an
/// unknown verb.
pub(crate) fn flags(verb: &str) -> &'static [&'static str] {
    match verb {
        "cp" => &["-r"],
        "dir" => &["-a", "-l", "-r", "-t"],
        "echo" => &["-n"],
        "ln" => &["-s"],
        "mkdir" => &["-p"],
        "rm" => &["-f", "-r"],
        _ => &[],
    }
}

/// What a builtin's non-flag arguments name, so the console can complete them
/// against the filesystem.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Paths {
    /// Any entry — files and directories alike.
    Any,
    /// Directories only: what `cd` can be given.
    Directories,
}

/// The filesystem vocabulary of `verb`'s arguments, or `None` where they are
/// not paths (`echo`, `export`, `printenv`, `pwd`, `unset`).
pub(crate) fn path_arguments(verb: &str) -> Option<Paths> {
    match verb {
        "cd" => Some(Paths::Directories),
        "cat" | "cp" | "dir" | "ln" | "mkdir" | "mv" | "rm" | "touch" => Some(Paths::Any),
        _ => None,
    }
}

// --- process state ---

/// `cd [<dir>|-]` — change the console's working directory, which every line
/// spawned afterwards inherits. No argument goes to `$HOME`, `-` to `$OLDPWD`,
/// and a leading `~` expands. `OLDPWD`/`PWD` are updated so `cd -` round-trips,
/// exactly as a shell's `cd` does. Nothing is printed on success.
fn cd(args: &[String]) -> Result<(), String> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let oldpwd = std::env::var_os("OLDPWD").map(PathBuf::from);
    let target = cd_target(
        args.first().map(String::as_str),
        home.as_deref(),
        oldpwd.as_deref(),
    )?;

    let previous = std::env::current_dir().ok();
    std::env::set_current_dir(&target).map_err(|e| format!("{}: {e}", target.display()))?;

    // SAFETY: the console reads a line, runs it, and loops on the same thread,
    // and spawns no threads of its own — nothing can be reading the
    // environment while these two writes happen.
    unsafe {
        if let Some(previous) = previous {
            std::env::set_var("OLDPWD", previous);
        }
        if let Ok(now) = std::env::current_dir() {
            std::env::set_var("PWD", now);
        }
    }
    Ok(())
}

/// Where `cd` goes, given its argument and the two variables it reads. Split
/// out from [`cd`] so the resolution is testable without moving the process.
fn cd_target(
    arg: Option<&str>,
    home: Option<&Path>,
    oldpwd: Option<&Path>,
) -> Result<PathBuf, String> {
    match arg {
        None | Some("~") => home
            .map(Path::to_path_buf)
            .ok_or_else(|| "HOME not set".to_string()),
        Some("-") => oldpwd
            .map(Path::to_path_buf)
            .ok_or_else(|| "OLDPWD not set".to_string()),
        Some(dir) => expand_tilde(dir, home),
    }
}

/// Expand a leading `~` (`~` or `~/rest`) to `$HOME`; other paths pass through.
fn expand_tilde(dir: &str, home: Option<&Path>) -> Result<PathBuf, String> {
    let home = || {
        home.map(Path::to_path_buf)
            .ok_or_else(|| "HOME not set".to_string())
    };
    if dir == "~" {
        return home();
    }
    match dir.strip_prefix("~/") {
        Some(rest) => Ok(home()?.join(rest)),
        None => Ok(PathBuf::from(dir)),
    }
}

/// `pwd` — print the console's working directory.
fn pwd(_args: &[String]) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("cannot read cwd: {e}"))?;
    println!("{}", cwd.display());
    Ok(())
}

/// `export [NAME=VALUE...|NAME...]` — with no arguments, the whole environment;
/// `NAME=VALUE` sets a variable every later line inherits; a bare `NAME` prints
/// that variable's value, or nothing when it is unset.
fn export(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return printenv(&[]);
    }
    for arg in args {
        match arg.split_once('=') {
            // SAFETY: single-threaded console loop, as in `cd`.
            Some((name, value)) => unsafe { std::env::set_var(name, value) },
            None => {
                if let Ok(value) = std::env::var(arg) {
                    println!("{value}");
                }
            }
        }
    }
    Ok(())
}

/// `printenv [NAME...]` — every variable as `NAME=VALUE` sorted, or just the
/// named ones' values.
fn printenv(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        let mut vars: Vec<(String, String)> = std::env::vars().collect();
        vars.sort();
        for (name, value) in vars {
            println!("{name}={value}");
        }
        return Ok(());
    }
    for name in args {
        if let Ok(value) = std::env::var(name) {
            println!("{value}");
        }
    }
    Ok(())
}

/// `unset <NAME>...` — remove environment variables.
fn unset(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("usage: unset <NAME>...".to_string());
    }
    for name in args {
        // SAFETY: single-threaded console loop, as in `cd`.
        unsafe { std::env::remove_var(name) };
    }
    Ok(())
}

/// `echo [-n] [<arg>...]` — print the arguments joined by a single space, `-n`
/// without the trailing newline. Printed literally: no variable or glob
/// expansion happens here.
fn echo(args: &[String]) -> Result<(), String> {
    let (newline, rest) = match args.split_first() {
        Some((flag, rest)) if flag == "-n" => (false, rest),
        _ => (true, args),
    };
    let line = rest.join(" ");
    if newline {
        println!("{line}");
    } else {
        print!("{line}");
        std::io::stdout()
            .flush()
            .map_err(|e| format!("write: {e}"))?;
    }
    Ok(())
}

// --- filesystem ---

/// Split `args` into bundled single-char flags (from `-x` / `-xy` tokens) and
/// the remaining operands, honouring a `--` end-of-flags terminator.
fn split_flags(args: &[String]) -> (String, Vec<&str>) {
    let mut flags = String::new();
    let mut rest = Vec::new();
    let mut only_operands = false;
    for arg in args {
        if !only_operands && arg == "--" {
            only_operands = true;
        } else if !only_operands && arg.len() > 1 && arg.starts_with('-') {
            flags.push_str(&arg[1..]);
        } else {
            rest.push(arg.as_str());
        }
    }
    (flags, rest)
}

/// `mkdir [-p] <dir>...` — create directories; `-p` makes parents as needed and
/// does not fail on a directory that already exists.
fn mkdir(args: &[String]) -> Result<(), String> {
    let (flags, dirs) = split_flags(args);
    if dirs.is_empty() {
        return Err("usage: mkdir [-p] <dir>...".to_string());
    }
    let parents = flags.contains('p');
    for dir in dirs {
        let created = if parents {
            std::fs::create_dir_all(dir)
        } else {
            std::fs::create_dir(dir)
        };
        created.map_err(|e| format!("{dir}: {e}"))?;
    }
    Ok(())
}

/// `touch <file>...` — create each file if missing, else bump its mtime.
fn touch(args: &[String]) -> Result<(), String> {
    let (_flags, files) = split_flags(args);
    if files.is_empty() {
        return Err("usage: touch <file>...".to_string());
    }
    for file in files {
        // create + write without truncate: makes the file if absent, leaves
        // the contents intact if it is already there.
        let handle = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(file)
            .map_err(|e| format!("{file}: {e}"))?;
        handle
            .set_modified(SystemTime::now())
            .map_err(|e| format!("{file}: {e}"))?;
    }
    Ok(())
}

/// `cat <file>...` — write each file's bytes to stdout, in order.
fn cat(args: &[String]) -> Result<(), String> {
    let (_flags, files) = split_flags(args);
    if files.is_empty() {
        return Err("usage: cat <file>...".to_string());
    }
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for file in files {
        let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
        out.write_all(&bytes).map_err(|e| format!("write: {e}"))?;
    }
    out.flush().map_err(|e| format!("write: {e}"))
}

/// `rm [-r] [-f] <path>...` — remove files, or directories with `-r`; `-f`
/// ignores missing paths. Symlinks are removed, never followed.
fn rm(args: &[String]) -> Result<(), String> {
    let (flags, paths) = split_flags(args);
    let recursive = flags.contains('r') || flags.contains('R');
    let force = flags.contains('f');
    if paths.is_empty() {
        if force {
            return Ok(());
        }
        return Err("usage: rm [-r] [-f] <path>...".to_string());
    }
    for path in paths {
        match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.is_dir() => {
                if !recursive {
                    return Err(format!("{path}: is a directory (use -r)"));
                }
                std::fs::remove_dir_all(path).map_err(|e| format!("{path}: {e}"))?;
            }
            Ok(_) => std::fs::remove_file(path).map_err(|e| format!("{path}: {e}"))?,
            Err(_) if force => {}
            Err(e) => return Err(format!("{path}: {e}")),
        }
    }
    Ok(())
}

/// `cp [-r] <src>... <dst>` — copy files; `-r` copies directories. Several
/// sources require `<dst>` to be a directory.
fn cp(args: &[String]) -> Result<(), String> {
    let (flags, paths) = split_flags(args);
    let recursive = flags.contains('r') || flags.contains('R');
    if paths.len() < 2 {
        return Err("usage: cp [-r] <src>... <dst>".to_string());
    }
    let (srcs, dst) = paths.split_at(paths.len() - 1);
    apply_to_dst(srcs, Path::new(dst[0]), |src, target| {
        copy_tree(src, target, recursive)
    })
}

/// `mv <src>... <dst>` — move or rename; several sources require `<dst>` to be
/// a directory. Falls back to copy-then-remove across filesystems.
fn mv(args: &[String]) -> Result<(), String> {
    let (_flags, paths) = split_flags(args);
    if paths.len() < 2 {
        return Err("usage: mv <src>... <dst>".to_string());
    }
    let (srcs, dst) = paths.split_at(paths.len() - 1);
    apply_to_dst(srcs, Path::new(dst[0]), move_path)
}

/// `ln [-s] <target> <link>` — create a hard link, or a symlink with `-s`.
fn ln(args: &[String]) -> Result<(), String> {
    let (flags, paths) = split_flags(args);
    if paths.len() != 2 {
        return Err("usage: ln [-s] <target> <link>".to_string());
    }
    let (target, link) = (Path::new(paths[0]), Path::new(paths[1]));
    let linked = if flags.contains('s') {
        std::os::unix::fs::symlink(target, link)
    } else {
        std::fs::hard_link(target, link)
    };
    linked.map_err(|e| format!("{}: {e}", link.display()))
}

/// Shared src→dst dispatch for `cp`/`mv`: with several sources (or a directory
/// destination) each source lands *inside* `dst` under its own name; with a
/// single source and a non-directory `dst`, `dst` is the new name.
fn apply_to_dst(
    srcs: &[&str],
    dst: &Path,
    op: impl Fn(&Path, &Path) -> Result<(), String>,
) -> Result<(), String> {
    let dst_is_dir = dst.is_dir();
    if srcs.len() > 1 && !dst_is_dir {
        return Err(format!("target {} is not a directory", dst.display()));
    }
    for src in srcs {
        let src = Path::new(src);
        let target = if dst_is_dir {
            match src.file_name() {
                Some(name) => dst.join(name),
                None => return Err(format!("invalid source {}", src.display())),
            }
        } else {
            dst.to_path_buf()
        };
        op(src, &target)?;
    }
    Ok(())
}

/// Copy `src` to `dst`: a whole tree when `recursive`, a single file otherwise
/// (a directory without `-r` is an error).
fn copy_tree(src: &Path, dst: &Path, recursive: bool) -> Result<(), String> {
    let meta = std::fs::symlink_metadata(src).map_err(|e| format!("{}: {e}", src.display()))?;
    if !meta.is_dir() {
        std::fs::copy(src, dst).map_err(|e| format!("{}: {e}", src.display()))?;
        return Ok(());
    }
    if !recursive {
        return Err(format!("{}: is a directory (use -r)", src.display()));
    }
    std::fs::create_dir_all(dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    for entry in std::fs::read_dir(src).map_err(|e| format!("{}: {e}", src.display()))? {
        let entry = entry.map_err(|e| format!("{}: {e}", src.display()))?;
        copy_tree(&entry.path(), &dst.join(entry.file_name()), true)?;
    }
    Ok(())
}

/// Move `src` to `dst`: a rename when possible, else copy the tree and remove
/// the original (crossing filesystems, where `rename` fails with `EXDEV`).
fn move_path(src: &Path, dst: &Path) -> Result<(), String> {
    if std::fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    copy_tree(src, dst, true)?;
    let meta = std::fs::symlink_metadata(src).map_err(|e| format!("{}: {e}", src.display()))?;
    if meta.is_dir() {
        std::fs::remove_dir_all(src).map_err(|e| format!("{}: {e}", src.display()))
    } else {
        std::fs::remove_file(src).map_err(|e| format!("{}: {e}", src.display()))
    }
}

// --- dir ---

/// Parsed `dir` options.
struct DirOpts {
    all: bool,
    long: bool,
    reverse: bool,
    by_mtime: bool,
    path: PathBuf,
}

impl DirOpts {
    /// Bundled flags (`-la`) and at most one path; unknown flags are ignored
    /// rather than fatal — this is a console convenience, not a full `ls`.
    fn parse(args: &[String]) -> DirOpts {
        let (flags, operands) = split_flags(args);
        DirOpts {
            all: flags.contains('a'),
            long: flags.contains('l'),
            reverse: flags.contains('r'),
            by_mtime: flags.contains('t'),
            path: operands
                .first()
                .map_or_else(|| PathBuf::from("."), PathBuf::from),
        }
    }
}

/// What an entry is, which is what colours its name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Dir,
    Symlink,
    Exec,
    File,
}

/// A listing entry, with its size and date pre-rendered so the caller can size
/// those columns to the widest value instead of running ragged.
struct Row {
    name: String,
    sort_key: String,
    kind: Kind,
    mode: u32,
    size_str: String,
    date_str: String,
    mtime: i64,
}

/// `dir [-alrt] [<path>]` — list a directory, or a single named file.
fn dir(args: &[String]) -> Result<(), String> {
    let opts = DirOpts::parse(args);
    let rows = dir_rows(&opts)?;
    let color = colored();

    // Widest value per column: the relative dates ("3d" vs "just now") and the
    // sizes vary in width, so both are measured before anything is printed.
    let (size_w, date_w) = if opts.long {
        (
            rows.iter().map(|r| r.size_str.len()).max().unwrap_or(0),
            rows.iter().map(|r| r.date_str.len()).max().unwrap_or(0),
        )
    } else {
        (0, 0)
    };

    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    for row in &rows {
        if opts.long {
            write!(
                out,
                "{} {:>size_w$} {:>date_w$}  ",
                perm_string(row.mode),
                row.size_str,
                row.date_str
            )
            .map_err(|e| format!("write: {e}"))?;
        }
        writeln!(out, "{}", colored_name(row, color)).map_err(|e| format!("write: {e}"))?;
    }
    out.flush().map_err(|e| format!("write: {e}"))
}

/// The rows `dir` prints, sorted: by name (case-insensitively) or by mtime with
/// `-t`, newest first, reversed by `-r`.
fn dir_rows(opts: &DirOpts) -> Result<Vec<Row>, String> {
    let meta = std::fs::symlink_metadata(&opts.path)
        .map_err(|e| format!("{}: {e}", opts.path.display()))?;
    let (base, names): (PathBuf, Vec<PathBuf>) = if meta.is_dir() {
        let mut names = Vec::new();
        for entry in
            std::fs::read_dir(&opts.path).map_err(|e| format!("{}: {e}", opts.path.display()))?
        {
            let entry = entry.map_err(|e| format!("{}: {e}", opts.path.display()))?;
            let name = entry.file_name();
            if !opts.all && name.as_encoded_bytes().first() == Some(&b'.') {
                continue;
            }
            names.push(PathBuf::from(name));
        }
        (opts.path.clone(), names)
    } else {
        let base = opts.path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let name = opts
            .path
            .file_name()
            .map_or_else(|| opts.path.clone(), PathBuf::from);
        (base, vec![name])
    };

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let mut rows: Vec<Row> = names
        .iter()
        .map(|name| Row::build(&base, name, now))
        .collect();
    rows.sort_by(|a, b| {
        if opts.by_mtime {
            b.mtime
                .cmp(&a.mtime)
                .then_with(|| a.sort_key.cmp(&b.sort_key))
        } else {
            a.sort_key.cmp(&b.sort_key)
        }
    });
    if opts.reverse {
        rows.reverse();
    }
    Ok(rows)
}

impl Row {
    fn build(base: &Path, name: &Path, now: i64) -> Row {
        let meta = std::fs::symlink_metadata(base.join(name)).ok();
        let file_type = meta.as_ref().map(std::fs::Metadata::file_type);
        let mode = meta.as_ref().map_or(0, |m| m.permissions().mode());
        let kind = match file_type {
            Some(t) if t.is_symlink() => Kind::Symlink,
            Some(t) if t.is_dir() => Kind::Dir,
            _ if mode & 0o111 != 0 => Kind::Exec,
            _ => Kind::File,
        };
        let mtime = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs() as i64);

        let name = name.to_string_lossy().into_owned();
        Row {
            sort_key: name.to_lowercase(),
            name,
            kind,
            mode,
            size_str: human_size(meta.as_ref().map_or(0, std::fs::Metadata::len)),
            date_str: ago(now - mtime),
            mtime,
        }
    }
}

/// The `ls -l`-style permission string of a raw `st_mode`.
fn perm_string(mode: u32) -> String {
    let type_ch = match mode & 0o170000 {
        0o040000 => 'd',
        0o120000 => 'l',
        0o140000 => 's',
        0o060000 => 'b',
        0o020000 => 'c',
        0o010000 => 'p',
        _ => '-',
    };
    let mut out = String::with_capacity(10);
    out.push(type_ch);
    for shift in [6, 3, 0] {
        let bits = (mode >> shift) & 0o7;
        for (set, ch) in [(bits & 0o4, 'r'), (bits & 0o2, 'w'), (bits & 0o1, 'x')] {
            out.push(if set == 0 { '-' } else { ch });
        }
    }
    out
}

/// The entry name coloured by kind, with a trailing `/` on directories.
fn colored_name(row: &Row, color: bool) -> String {
    let (suffix, code) = match row.kind {
        Kind::Dir => ("/", "36"),
        Kind::Symlink => ("", "35"),
        Kind::Exec => ("", "32"),
        Kind::File => ("", "0"),
    };
    paint(&format!("{}{suffix}", row.name), code, color)
}

/// Human-readable byte size (1024-based, one decimal above K).
fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];
    if bytes < 1024 {
        return format!("{bytes}B");
    }
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{size:.1}{}", UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A private directory for one test, named after it so the tests never
    /// collide with each other (they run in threads of one process) and are
    /// safe to run in parallel. Absolute paths throughout: nothing here may
    /// depend on, or change, the process's working directory.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(test: &str) -> TempDir {
            let path =
                std::env::temp_dir().join(format!("ztmux-shell-{}-{test}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("create temp dir");
            TempDir(path)
        }

        /// `self.0/name` as the string the builtins take as an argument.
        fn arg(&self, name: &str) -> String {
            self.0.join(name).to_string_lossy().into_owned()
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn builtin_names_stay_sorted_so_lookup_finds_every_one() {
        let names: Vec<&str> = described().map(|(name, _)| name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(
            names, sorted,
            "BUILTINS must stay sorted for lookup's binary search"
        );

        for name in &names {
            assert!(
                lookup(name).is_some(),
                "{name} is listed but lookup misses it"
            );
        }
        // A verb the console must spawn instead of running in-process.
        assert!(lookup("kill-pane").is_none());
    }

    #[test]
    fn cd_resolves_home_oldpwd_and_tilde_without_moving_the_process() {
        let home = Path::new("/home/user");
        let old = Path::new("/var/tmp");
        assert_eq!(cd_target(None, Some(home), None).unwrap(), home);
        assert_eq!(cd_target(Some("~"), Some(home), None).unwrap(), home);
        assert_eq!(
            cd_target(Some("~/src/app"), Some(home), None).unwrap(),
            home.join("src/app")
        );
        assert_eq!(cd_target(Some("-"), Some(home), Some(old)).unwrap(), old);
        assert_eq!(
            cd_target(Some("/etc"), Some(home), None).unwrap(),
            Path::new("/etc")
        );
        // A `~` inside the path is not a home reference, only a leading one.
        assert_eq!(
            cd_target(Some("a/~/b"), Some(home), None).unwrap(),
            Path::new("a/~/b")
        );
        // The two variables the resolution needs report their own absence.
        assert_eq!(cd_target(None, None, None).unwrap_err(), "HOME not set");
        assert_eq!(
            cd_target(Some("-"), Some(home), None).unwrap_err(),
            "OLDPWD not set"
        );
    }

    #[test]
    fn flags_bundle_and_stop_at_the_double_dash() {
        let separate = args(&["-r", "-f", "a", "b"]);
        let (flags, rest) = split_flags(&separate);
        assert_eq!(flags, "rf");
        assert_eq!(rest, ["a", "b"]);
        // Bundled in one token, as `-rf` is typed in practice.
        assert_eq!(split_flags(&args(&["-rf", "a"])).0, "rf");
        // `--` ends the flags: a file literally named `-r` is still removable.
        let terminated = args(&["--", "-r"]);
        let (flags, rest) = split_flags(&terminated);
        assert!(flags.is_empty());
        assert_eq!(rest, ["-r"]);
        // A lone `-` is an operand (it is `cd`'s previous-directory argument),
        // never a flag.
        assert_eq!(split_flags(&args(&["-"])).1, ["-"]);
    }

    #[test]
    fn filesystem_builtins_create_copy_move_and_remove() {
        let tmp = TempDir::new("fs");
        let (tree, file) = (tmp.arg("tree/nested"), tmp.arg("tree/nested/f"));

        mkdir(&args(&["-p", &tree])).expect("mkdir -p");
        assert!(Path::new(&tree).is_dir());
        // Without -p, an existing directory is an error rather than a no-op.
        assert!(mkdir(&args(&[&tree])).is_err());

        touch(&args(&[&file])).expect("touch");
        std::fs::write(&file, b"payload").expect("write");
        touch(&args(&[&file])).expect("touch again");
        assert_eq!(
            std::fs::read(&file).expect("read"),
            b"payload",
            "touch truncated the file"
        );

        // cp -r copies the tree; without -r a directory is refused.
        let copy = tmp.arg("copy");
        assert!(cp(&args(&[&tmp.arg("tree"), &copy])).is_err());
        cp(&args(&["-r", &tmp.arg("tree"), &copy])).expect("cp -r");
        assert_eq!(
            std::fs::read(tmp.arg("copy/nested/f")).expect("read"),
            b"payload"
        );

        // mv renames; the source is gone afterwards.
        let moved = tmp.arg("moved");
        mv(&args(&[&copy, &moved])).expect("mv");
        assert!(!Path::new(&copy).exists());
        assert!(Path::new(&moved).is_dir());

        // ln -s links; rm removes the link, not its target.
        let link = tmp.arg("link");
        ln(&args(&["-s", &file, &link])).expect("ln -s");
        assert!(
            std::fs::symlink_metadata(&link)
                .expect("lstat")
                .is_symlink()
        );
        rm(&args(&[&link])).expect("rm link");
        assert!(Path::new(&file).exists(), "rm followed the symlink");

        // A directory needs -r; -f swallows a missing path.
        assert!(rm(&args(&[&moved])).is_err());
        rm(&args(&["-r", &moved])).expect("rm -r");
        assert!(!Path::new(&moved).exists());
        assert!(rm(&args(&[&tmp.arg("nothing")])).is_err());
        rm(&args(&["-f", &tmp.arg("nothing")])).expect("rm -f on a missing path");
    }

    #[test]
    fn several_sources_need_a_directory_destination() {
        let tmp = TempDir::new("multi");
        let (a, b) = (tmp.arg("a"), tmp.arg("b"));
        std::fs::write(&a, b"a").expect("write a");
        std::fs::write(&b, b"b").expect("write b");

        // Two sources onto a non-directory would silently overwrite one with
        // the other; it is refused instead.
        let plain = tmp.arg("plain");
        std::fs::write(&plain, b"").expect("write plain");
        assert!(cp(&args(&[&a, &b, &plain])).is_err());

        let into = tmp.arg("into");
        std::fs::create_dir(&into).expect("mkdir into");
        cp(&args(&[&a, &b, &into])).expect("cp into a directory");
        assert_eq!(std::fs::read(tmp.arg("into/a")).expect("read"), b"a");
        assert_eq!(std::fs::read(tmp.arg("into/b")).expect("read"), b"b");
    }

    #[test]
    fn dir_lists_sorted_hides_dotfiles_and_orders_by_time() {
        let tmp = TempDir::new("dir");
        for name in ["b.txt", "A.txt", ".hidden"] {
            std::fs::write(tmp.arg(name), b"x").expect("write");
        }
        std::fs::create_dir(tmp.arg("sub")).expect("mkdir sub");

        let names = |flags: &[&str]| -> Vec<String> {
            let mut argv = args(flags);
            argv.push(tmp.0.to_string_lossy().into_owned());
            dir_rows(&DirOpts::parse(&argv))
                .expect("rows")
                .into_iter()
                .map(|r| r.name)
                .collect()
        };

        // Case-insensitive by name, dotfiles hidden until -a.
        assert_eq!(names(&[]), ["A.txt", "b.txt", "sub"]);
        assert_eq!(names(&["-a"]), [".hidden", "A.txt", "b.txt", "sub"]);
        assert_eq!(names(&["-r"]), ["sub", "b.txt", "A.txt"]);

        // -t is newest first: bumping A.txt's mtime moves it to the front.
        touch(&args(&[&tmp.arg("A.txt")])).expect("touch");
        assert_eq!(names(&["-t"])[0], "A.txt");

        // A single named file lists just itself, not its directory.
        let mut one = args(&[]);
        one.push(tmp.arg("b.txt"));
        let rows = dir_rows(&DirOpts::parse(&one)).expect("rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "b.txt");
        assert_eq!(rows[0].kind, Kind::File);
        // A missing path is an error, not an empty listing.
        let mut missing = args(&[]);
        missing.push(tmp.arg("no-such-entry"));
        assert!(dir_rows(&DirOpts::parse(&missing)).is_err());
    }

    #[test]
    fn long_columns_render_mode_and_size() {
        assert_eq!(perm_string(0o040755), "drwxr-xr-x");
        assert_eq!(perm_string(0o100644), "-rw-r--r--");
        assert_eq!(perm_string(0o120777), "lrwxrwxrwx");
        assert_eq!(human_size(0), "0B");
        assert_eq!(human_size(1023), "1023B");
        assert_eq!(human_size(1024), "1.0K");
        assert_eq!(human_size(1536), "1.5K");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0M");
    }

    #[test]
    fn completion_vocabulary_covers_every_builtin() {
        // `cd` completes directories only; the other path-taking builtins take
        // any entry; the process-state ones have no filesystem vocabulary.
        assert_eq!(path_arguments("cd"), Some(Paths::Directories));
        assert_eq!(path_arguments("cat"), Some(Paths::Any));
        assert_eq!(path_arguments("export"), None);
        assert_eq!(path_arguments("pwd"), None);
        // Flags are advertised only for the builtins that parse them, and
        // every advertised flag is one `split_flags` would pick up.
        assert_eq!(flags("rm"), ["-f", "-r"]);
        assert!(flags("pwd").is_empty());
        for (name, _) in described() {
            for flag in flags(name) {
                assert_eq!(
                    split_flags(&args(&[flag])).0.len(),
                    1,
                    "{name}: {flag} is not a flag"
                );
            }
        }
    }
}
