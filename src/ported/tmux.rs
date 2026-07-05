// Copyright (c) 2007 Nicholas Marriott <nicholas.marriott@gmail.com>
//
// Permission to use, copy, modify, and distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF MIND, USE, DATA OR PROFITS, WHETHER
// IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING
// OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::OnceLock;


use crate::compat::getopt::{OPTARG, OPTIND, getopt};
use crate::compat::{S_ISDIR, fdforkpty::getptmfd, getprogname::getprogname};
use crate::libc::{
    CLOCK_MONOTONIC, CLOCK_REALTIME, CODESET, EEXIST, F_GETFL, F_SETFL, LC_CTYPE, LC_TIME,
    O_NONBLOCK, S_IRWXO, S_IRWXU, X_OK, access, clock_gettime, fcntl, getpwuid, getuid, lstat,
    mkdir, nl_langinfo, setlocale, stat, strchr, strerror, strncmp, strrchr, timespec,
};
use crate::*;
use crate::options_::{options, options_create, options_default, options_set_number, options_set_string};

pub static mut GLOBAL_OPTIONS: *mut options = null_mut();
pub static mut GLOBAL_S_OPTIONS: *mut options = null_mut();
pub static mut GLOBAL_W_OPTIONS: *mut options = null_mut();
pub static mut GLOBAL_ENVIRON: *mut environ = null_mut();

pub static mut START_TIME: timeval = timeval {
    tv_sec: 0,
    tv_usec: 0,
};

pub static mut SOCKET_PATH: *const u8 = null_mut();

pub static mut PTM_FD: c_int = -1;

pub static mut SHELL_COMMAND: *mut u8 = null_mut();

/// C `vendor/tmux/tmux.c:53`: `static __dead void usage(int status)`
pub fn usage() -> ! {
    eprintln!(
        "usage: ztmux [-2CDlNuVv] [-c shell-command] [-f file] [-L socket-name]\n             [-S socket-path] [-T features] [command [flags]]\n"
    );
    std::process::exit(1)
}

/// C `vendor/tmux/tmux.c:63`: `static const char *getshell(void)`
unsafe fn getshell() -> Cow<'static, CStr> {
    unsafe {
        if let Ok(shell) = std::env::var("SHELL")
            && let shell = CString::new(shell).unwrap()
            && checkshell(Some(&shell))
        {
            return Cow::Owned(shell);
        }

        if let Some(pw) = NonNull::new(getpwuid(getuid()))
            && !(*pw.as_ptr()).pw_shell.is_null()
            && checkshell(Some(CStr::from_ptr((*pw.as_ptr()).pw_shell)))
        {
            return Cow::Owned(CString::new(cstr_to_str((*pw.as_ptr()).pw_shell.cast())).unwrap());
        }

        Cow::Borrowed(CStr::from_ptr(_PATH_BSHELL.cast()))
    }
}

/// C `vendor/tmux/tmux.c:80`: `int checkshell(const char *shell)`
pub unsafe fn checkshell(shell: Option<&CStr>) -> bool {
    unsafe {
        let Some(shell) = shell else {
            return false;
        };
        if shell.to_bytes()[0] != b'/' {
            return false;
        }
        if areshell(shell) {
            return false;
        }
        if access(shell.as_ptr().cast(), X_OK) != 0 {
            return false;
        }
    }
    true
}

pub unsafe fn checkshell_(shell: *const u8) -> bool {
    unsafe {
        if shell.is_null() {
            return false;
        }
        if *shell != b'/' {
            return false;
        }
        if areshell(CStr::from_ptr(shell.cast())) {
            return false;
        }
        if access(shell.cast(), X_OK) != 0 {
            return false;
        }
    }
    true
}

/// C `vendor/tmux/tmux.c:92`: `static int areshell(const char *shell)`
unsafe fn areshell(shell: &CStr) -> bool {
    unsafe {
        let ptr = strrchr(shell.as_ptr().cast(), b'/' as c_int);
        let ptr = if !ptr.is_null() {
            ptr.wrapping_add(1)
        } else {
            shell.as_ptr().cast()
        };
        let mut progname = getprogname();
        if *progname == b'-' {
            progname = progname.wrapping_add(1);
        }
        libc::strcmp(ptr, progname) == 0
    }
}

/// C `vendor/tmux/tmux.c:109`: `static char *expand_path(const char *path, const char *home)`
unsafe fn expand_path(path: *const u8, home: Option<&CStr>) -> Option<CString> {
    unsafe {
        if strncmp(path, c!("~/"), 2) == 0 {
            return Some(
                CString::new(format!("{}{}", home?.to_str().unwrap(), _s(path.add(1)))).unwrap(),
            );
        }

        if *path == b'$' {
            let mut end: *const u8 = strchr(path, b'/' as i32).cast();
            let name = if end.is_null() {
                xstrdup(path.add(1)).cast().as_ptr()
            } else {
                xstrndup(path.add(1), end.addr() - path.addr() - 1)
                    .cast()
                    .as_ptr()
            };
            let value = environ_find(GLOBAL_ENVIRON, name);
            free_(name);
            if value.is_null() {
                return None;
            }
            if end.is_null() {
                end = c!("");
            }
            return Some(
                CString::new(format!("{}{}", _s(transmute_ptr((*value).value)), _s(end))).unwrap(),
            );
        }

        Some(CString::new(cstr_to_str(path)).unwrap())
    }
}

/// C `vendor/tmux/tmux.c:142`: `static void expand_paths(const char *s, char ***paths, u_int *n, int no_realpath)`
unsafe fn expand_paths(s: &str, paths: &mut Vec<CString>, ignore_errors: i32) {
    unsafe {
        let home = find_home();
        let mut path: CString;

        let func = "expand_paths";

        paths.clear();

        let mut next: *const u8;
        let mut tmp: *mut u8 = xstrdup__(s);
        let copy = tmp;
        while {
            next = strsep(&raw mut tmp as _, c!(":").cast());
            !next.is_null()
        } {
            let Some(expanded) = expand_path(next, home) else {
                log_debug!("{func}: invalid path: {}", _s(next));
                continue;
            };

            match PathBuf::from(expanded.to_str().unwrap()).canonicalize() {
                Ok(resolved) => {
                    path = CString::new(resolved.into_os_string().into_string().unwrap()).unwrap();
                    // free_(expanded);
                }
                Err(_) => {
                    log_debug!(
                        "{func}: realpath(\"{}\") failed: {}",
                        expanded.to_string_lossy(),
                        strerror(errno!()),
                    );
                    if ignore_errors != 0 {
                        // free_(expanded);
                        continue;
                    }
                    path = expanded;
                }
            }

            if paths.contains(&path) {
                log_debug!("{func}: duplicate path: {}", path.to_string_lossy());
                // free_(path);
                continue;
            }

            paths.push(path);
        }
        free_(copy);
    }
}

/// C `vendor/tmux/tmux.c:187`: `static char *make_label(const char *label, char **cause)`
unsafe fn make_label(mut label: *const u8, cause: *mut *mut u8) -> *const u8 {
    let mut paths: Vec<CString> = Vec::new();
    let base: *mut u8;
    let mut sb: stat = unsafe { zeroed() }; // TODO use uninit

    unsafe {
        'fail: {
            *cause = null_mut();
            if label.is_null() {
                label = c!("default");
            }
            let uid = getuid();

            expand_paths(TMUX_SOCK, &mut paths, 1);
            if paths.is_empty() {
                *cause = format_nul!("no suitable socket path");
                return null_mut();
            }

            paths.truncate(1);
            let mut path = paths.pop().unwrap(); /* can only have one socket! */

            base = format_nul!("{}/ztmux-{}", path.to_string_lossy(), uid);
            if mkdir(base.cast(), S_IRWXU) != 0 && errno!() != EEXIST {
                *cause = format_nul!(
                    "couldn't create directory {} ({})",
                    _s(base),
                    strerror(errno!())
                );
                break 'fail;
            }
            if lstat(base.cast(), &raw mut sb) != 0 {
                *cause = format_nul!(
                    "couldn't read directory {} ({})",
                    _s(base),
                    strerror(errno!()),
                );
                break 'fail;
            }
            if !S_ISDIR(sb.st_mode) {
                *cause = format_nul!("{} is not a directory", _s(base));
                break 'fail;
            }
            if sb.st_uid != uid || (sb.st_mode & S_IRWXO) != 0 {
                *cause = format_nul!("directory {} has unsafe permissions", _s(base));
                break 'fail;
            }
            path = CString::new(format!("{}/{}", _s(base), _s(label))).unwrap();
            free_(base);
            return path.into_raw().cast();
        }

        // fail:
        free_(base);
        null_mut()
    }
}

/// C `vendor/tmux/tmux.c:239`: `char *shell_argv0(const char *shell, int is_login)`
pub unsafe fn shell_argv0(shell: *const u8, is_login: c_int) -> *mut u8 {
    unsafe {
        let slash = strrchr(shell, b'/' as _);
        let name = if !slash.is_null() && *slash.add(1) != b'\0' {
            slash.add(1)
        } else {
            shell
        };

        if is_login != 0 {
            format_nul!("-{}", _s(name))
        } else {
            format_nul!("{}", _s(name))
        }
    }
}

/// C `vendor/tmux/tmux.c:257`: `void setblocking(int fd, int state)`
pub unsafe fn setblocking(fd: c_int, state: c_int) {
    unsafe {
        let mut mode = fcntl(fd, F_GETFL);

        if mode != -1 {
            if state == 0 {
                mode |= O_NONBLOCK;
            } else {
                mode &= !O_NONBLOCK;
            }
            fcntl(fd, F_SETFL, mode);
        }
    }
}

/// C `vendor/tmux/tmux.c:285`: `char *clean_name(const char *name, int untrusted)`
pub unsafe fn clean_name(name: *const u8, untrusted: c_int) -> *mut u8 {
    unsafe {
        if !utf8_isvalid(name) {
            return null_mut();
        }
        let copy = xstrdup(name).as_ptr();
        let mut cp = copy;
        while *cp != b'\0' {
            if untrusted != 0 && *cp == b'#' && *cp.add(1) == b'(' {
                *cp = b'_';
            }
            cp = cp.add(1);
        }
        let mut new_name: *mut u8 = null_mut();
        utf8_stravis(
            &raw mut new_name,
            copy,
            vis_flags::VIS_OCTAL | vis_flags::VIS_CSTYLE | vis_flags::VIS_TAB | vis_flags::VIS_NL,
        );
        free_(copy);
        new_name
    }
}

/// C `vendor/tmux/tmux.c:271`: `uint64_t get_timer(void)`
pub unsafe fn get_timer() -> u64 {
    unsafe {
        let mut ts: timespec = zeroed();
        // We want a timestamp in milliseconds suitable for time measurement,
        // so prefer the monotonic clock.
        if clock_gettime(CLOCK_MONOTONIC, &raw mut ts) != 0 {
            clock_gettime(CLOCK_REALTIME, &raw mut ts);
        }
        (ts.tv_sec as u64 * 1000) + (ts.tv_nsec as u64 / 1000000)
    }
}

/// C `vendor/tmux/tmux.c:323`: `const char *find_cwd(void)`
pub fn find_cwd() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;

    let pwd = match std::env::var("PWD") {
        Ok(val) if !val.is_empty() => PathBuf::from(val),
        _ => return Some(cwd),
    };

    // We want to use PWD so that symbolic links are maintained,
    // but only if it matches the actual working directory.

    let Ok(resolved1) = pwd.canonicalize() else {
        return Some(cwd);
    };

    let Ok(resolved2) = cwd.canonicalize() else {
        return Some(cwd);
    };

    if resolved1 == resolved2 {
        return Some(cwd);
    }

    Some(pwd)
}

/// C `vendor/tmux/tmux.c:348`: `const char *find_home(void)`
pub fn find_home() -> Option<&'static CStr> {
    unsafe {
        static HOME: OnceLock<Option<CString>> = OnceLock::new();
        HOME.get_or_init(|| match std::env::var("HOME") {
            Ok(home) if !home.is_empty() => Some(CString::new(home).unwrap()),
            _ => NonNull::new(getpwuid(getuid()))
                .map(|pw| CString::new(cstr_to_str((*pw.as_ptr()).pw_dir.cast())).unwrap()),
        })
        .as_deref()
    }
}

/// C `vendor/tmux/tmux.c:369`: `const char *getversion(void)`
pub fn getversion() -> &'static str {
    crate::TMUX_VERSION
}

/// Entrypoint for tmux binary
///
/// # Safety
///
/// This code is work in progress. There is no guarantee that the code is safe.
/// This function should only be called by the tmux binary crate to start tmux.
pub unsafe fn tmux_main(mut argc: i32, mut argv: *mut *mut u8, _env: *mut *mut u8) {
    std::panic::set_hook(Box::new(|_panic_info| {
        let backtrace = std::backtrace::Backtrace::capture();
        let err_str = format!("{backtrace:#?}");
        // Write to TMPDIR (honours $TMPDIR, falls back to /tmp) rather than the
        // cwd, so a panic doesn't drop the file into wherever ztmux was launched.
        _ = std::fs::write(std::env::temp_dir().join("client-panic.txt"), err_str);
    }));

    unsafe {
        // setproctitle_init(argc, argv.cast(), env.cast());
        let mut cause: *mut u8 = null_mut();
        let mut path: *const u8 = null_mut();
        let mut label: *mut u8 = null_mut();
        let mut feat: i32 = 0;
        let mut fflag: i32 = 0;
        let mut flags: client_flag = client_flag::empty();

        if setlocale(LC_CTYPE, c!("en_US.UTF-8")).is_null()
            && setlocale(LC_CTYPE, c!("C.UTF-8")).is_null()
        {
            if setlocale(LC_CTYPE, c!("")).is_null() {
                eprintln!("invalid LC_ALL, LC_CTYPE or LANG");
                std::process::exit(1);
            }
            let s: *mut u8 = nl_langinfo(CODESET).cast();
            if !strcaseeq_(s, "UTF-8") && !strcaseeq_(s, "UTF8") {
                eprintln!("need UTF-8 locale (LC_CTYPE) but have {}", _s(s));
                std::process::exit(1);
            }
        }

        setlocale(LC_TIME, c!(""));
        tzset();

        if **argv == b'-' {
            flags = client_flag::LOGIN;
        }

        GLOBAL_ENVIRON = environ_create().as_ptr();

        let mut var = environ;
        while !(*var).is_null() {
            environ_put(GLOBAL_ENVIRON, *var, environ_flags::empty());
            var = var.add(1);
        }

        if let Some(cwd) = find_cwd() {
            environ_set!(
                GLOBAL_ENVIRON,
                c!("PWD"),
                environ_flags::empty(),
                "{}",
                cwd.to_str().unwrap()
            );
        }
        expand_paths(TMUX_CONF, &mut CFG_FILES.lock().unwrap(), 1);

        // tmux itself has no `--help`; the getopt() below turns `--help` into a
        // terse one-line usage. Intercept `-h`/`--help` among the leading option
        // words (before the command) and show the full cyberpunk help instead.
        for i in 1..argc {
            let arg = cstr_to_str(*argv.add(i as usize));
            if arg == "-h" || arg == "--help" {
                crate::extensions::help::help();
            }
            // Stop at the end-of-options marker or the first non-option word
            // (the tmux command), which handles its own `--help`.
            if arg == "--" || !arg.starts_with('-') {
                break;
            }
        }

        // Option string mirrors upstream tmux (`vendor/tmux/tmux.c`):
        // "2c:CDdf:hlL:NqS:T:uUvV". `h` is included so clustered/late forms
        // like `-Nh` still reach the help handler, matching tmux's `case 'h'`.
        while let Some(opt) = getopt(argc, argv.cast(), c!("2c:CDdf:hlL:NqS:T:uUvV")) {
            match opt {
                b'2' => tty_add_features(&raw mut feat, "256", c!(":,")),
                b'c' => SHELL_COMMAND = OPTARG.cast(),
                b'D' => flags |= client_flag::NOFORK,
                b'C' => {
                    if flags.intersects(client_flag::CONTROL) {
                        flags |= client_flag::CONTROLCONTROL;
                    } else {
                        flags |= client_flag::CONTROL;
                    }
                }
                b'f' => {
                    if fflag == 0 {
                        fflag = 1;
                        CFG_FILES.lock().unwrap().clear();
                    }
                    CFG_FILES
                        .lock()
                        .unwrap()
                        .push(CString::new(cstr_to_str(OPTARG)).unwrap());
                    CFG_QUIET.store(false, atomic::Ordering::Relaxed);
                }
                b'h' => crate::extensions::help::help(),
                b'V' => {
                    println!("ztmux {}", getversion());
                    std::process::exit(0);
                }
                b'l' => flags |= client_flag::LOGIN,
                b'L' => {
                    free(label as _);
                    label = xstrdup(OPTARG.cast()).cast().as_ptr();
                }
                b'N' => flags |= client_flag::NOSTARTSERVER,
                b'q' => (),
                b'S' => {
                    free(path as _);
                    path = xstrdup(OPTARG.cast()).cast().as_ptr();
                }
                b'T' => tty_add_features(&raw mut feat, cstr_to_str(OPTARG.cast()), c!(":,")),
                b'u' => flags |= client_flag::UTF8,
                b'v' => log_add_level(),
                _ => usage(),
            }
        }
        argc -= OPTIND;
        argv = argv.add(OPTIND as usize);

        if !SHELL_COMMAND.is_null() && argc != 0 {
            usage();
        }
        if flags.intersects(client_flag::NOFORK) && argc != 0 {
            usage();
        }

        PTM_FD = getptmfd();
        if PTM_FD == -1 {
            eprintln!("getptmfd failed!");
            std::process::exit(1);
        }

        /*
        // TODO no pledge on linux
            if pledge("stdio rpath wpath cpath flock fattr unix getpw sendfd recvfd proc exec tty ps", null_mut()) != 0 {
                err(1, "pledge");
        }
        */

        // ztmux is a UTF-8 terminal, so if TMUX is set, assume UTF-8.
        // Otherwise, if the user has set LC_ALL, LC_CTYPE or LANG to contain
        // UTF-8, it is a safe assumption that either they are using a UTF-8
        // terminal, or if not they know that output from UTF-8-capable
        // programs may be wrong.
        if std::env::var("TMUX").is_ok() {
            flags |= client_flag::UTF8;
        } else {
            let s = std::env::var("LC_ALL")
                .or_else(|_| std::env::var("LC_CTYPE"))
                .or_else(|_| std::env::var("LANG"))
                .unwrap_or_default()
                .to_ascii_lowercase();

            if s.contains("utf-8") || s.contains("utf8") {
                flags |= client_flag::UTF8;
            }
        }

        GLOBAL_OPTIONS = options_create(null_mut());
        GLOBAL_S_OPTIONS = options_create(null_mut());
        GLOBAL_W_OPTIONS = options_create(null_mut());

        for oe in &OPTIONS_TABLE {
            if oe.scope & OPTIONS_TABLE_SERVER != 0 {
                options_default(GLOBAL_OPTIONS, oe);
            }
            if oe.scope & OPTIONS_TABLE_SESSION != 0 {
                options_default(GLOBAL_S_OPTIONS, oe);
            }
            if oe.scope & OPTIONS_TABLE_WINDOW != 0 {
                options_default(GLOBAL_W_OPTIONS, oe);
            }
        }

        // The default shell comes from SHELL or from the user's passwd entry if available.
        options_set_string!(
            GLOBAL_S_OPTIONS,
            "default-shell",
            false,
            "{}",
            getshell().to_string_lossy(),
        );

        // Override keys to vi if VISUAL or EDITOR are set.
        if let Ok(s) = std::env::var("VISUAL").or_else(|_| std::env::var("EDITOR")) {
            options_set_string!(GLOBAL_OPTIONS, "editor", false, "{s}");

            let s = if let Some(slash_end) = s.rfind('/') {
                &s[slash_end + 1..]
            } else {
                &s
            };

            let keys = if s.contains("vi") {
                modekey::MODEKEY_VI
            } else {
                modekey::MODEKEY_EMACS
            };
            options_set_number(GLOBAL_S_OPTIONS, "status-keys", keys as _);
            options_set_number(GLOBAL_W_OPTIONS, "mode-keys", keys as _);
        }

        // Socket resolution. If -S or -L was given it is used; otherwise ztmux
        // ALWAYS resolves its own socket via make_label ("default" under the
        // ztmux-<uid> directory). Unlike tmux, ztmux deliberately does NOT adopt
        // a socket path from $TMUX: when ztmux runs inside a real tmux pane,
        // $TMUX points at the tmux server, and ztmux must never speak its
        // protocol to a tmux socket. Ignoring $TMUX for resolution keeps ztmux
        // and tmux fully isolated — they run side by side, or nested, with no
        // socket collision — while $TMUX is still exported (pointing at ztmux's
        // own socket) for ecosystem tools that only check whether it is set.
        if path.is_null() {
            path = make_label(label.cast(), &raw mut cause);
            if path.is_null() {
                if !cause.is_null() {
                    eprintln!("{}", _s(cause));
                    free(cause as _);
                }
                std::process::exit(1);
            }
            flags |= client_flag::DEFAULTSOCKET;
        }
        SOCKET_PATH = path;
        free_(label);

        // ztmux extension: `ztmux dashboard` launches the live ratatui dashboard
        // (src/extensions) as a client subcommand instead of a server command,
        // targeting the socket resolved above.
        if argc >= 1 && !(*argv).is_null() {
            let cmd = CStr::from_ptr((*argv).cast()).to_bytes();
            if matches!(
                cmd,
                b"dashboard"
                    | b"switcher"
                    | b"tree"
                    | b"doctor"
                    | b"stats"
                    | b"graph"
                    | b"watch"
                    | b"events"
                    | b"ps"
                    | b"snapshot"
                    | b"prune"
                    | b"layout"
                    | b"finder"
                    | b"recent"
                    | b"usage"
                    | b"grep"
                    | b"peek"
                    | b"bcast"
                    | b"pstree"
                    | b"ports"
                    | b"info"
                    | b"dedup"
                    | b"size"
                    | b"groups"
                    | b"tty"
                    | b"git"
                    | b"pick"
                    | b"active"
                    | b"ssh"
                    | b"stack"
                    | b"disk"
                    | b"net"
                    | b"env"
                    | b"history"
                    | b"mode"
                    | b"zoom"
                    | b"marks"
                    | b"alerts"
                    | b"titles"
                    | b"equalize"
                    | b"revive"
                    | b"clearall"
                    | b"retitle"
                    | b"cwd"
                    | b"who"
                    | b"age"
                    | b"cmd"
                    | b"dead"
                    | b"layouts"
                    | b"detached"
                    | b"density"
                    | b"nested"
                    | b"solo"
                    | b"shells"
                    | b"fanout"
                    | b"busy"
                    | b"focus"
                    | b"named"
                    | b"project"
                    | b"remote"
                    | b"ahead"
                    | b"changes"
                    | b"stash"
                    | b"elapsed"
                    | b"mem"
                    | b"state"
                    | b"commit"
                    | b"linked"
                    | b"conflicts"
                    | b"user"
                    | b"tag"
                    | b"vcs"
                    | b"gone"
                    | b"buffers"
                    | b"worktree"
                    | b"submodules"
                    | b"term"
                    | b"startcmd"
                    | b"writable"
                    | b"sync"
                    | b"piped"
                    | b"input"
                    | b"monitor"
                    | b"remain"
                    | b"autoname"
                    | b"readonly"
                    | b"idle"
                    | b"viewers"
                    | b"connected"
                    | b"constrain"
                    | b"hooks"
                    | b"destroy"
                    | b"status"
                    | b"keys"
                    | b"limit"
                    | b"winsize"
                    | b"borders"
                    | b"autolock"
                    | b"titlebar"
                    | b"visual"
                    | b"keytable"
                    | b"control"
                    | b"utf8"
                    | b"mouse"
                    | b"triggers"
            ) {
                let sock = if SOCKET_PATH.is_null() {
                    String::new()
                } else {
                    CStr::from_ptr(SOCKET_PATH.cast()).to_string_lossy().into_owned()
                };
                let code = match cmd {
                    b"switcher" => crate::extensions::switch::run(&sock),
                    b"tree" => crate::extensions::tree::run(&sock),
                    b"triggers" => crate::extensions::triggers::run(&sock),
                    b"doctor" => crate::extensions::doctor::run(&sock),
                    b"stats" => crate::extensions::stats::run(&sock),
                    b"graph" => crate::extensions::graph::run(&sock),
                    b"watch" => crate::extensions::watch::run(&sock),
                    b"events" => crate::extensions::events::run(&sock),
                    b"ps" => crate::extensions::ps::run(&sock),
                    b"snapshot" => crate::extensions::snapshot::run(&sock),
                    b"prune" => crate::extensions::prune::run(&sock),
                    b"layout" => crate::extensions::layout::run(&sock),
                    b"finder" => crate::extensions::find::run(&sock),
                    b"recent" => crate::extensions::recent::run(&sock),
                    b"usage" => crate::extensions::usage::run(&sock),
                    b"grep" => crate::extensions::grep::run(&sock),
                    b"peek" => crate::extensions::peek::run(&sock),
                    b"bcast" => crate::extensions::bcast::run(&sock),
                    b"pstree" => crate::extensions::pstree::run(&sock),
                    b"ports" => crate::extensions::ports::run(&sock),
                    b"info" => crate::extensions::info::run(&sock),
                    b"dedup" => crate::extensions::dedup::run(&sock),
                    b"size" => crate::extensions::size::run(&sock),
                    b"groups" => crate::extensions::groups::run(&sock),
                    b"tty" => crate::extensions::tty::run(&sock),
                    b"git" => crate::extensions::git::run(&sock),
                    b"pick" => crate::extensions::pick::run(&sock),
                    b"active" => crate::extensions::active::run(&sock),
                    b"ssh" => crate::extensions::ssh::run(&sock),
                    b"stack" => crate::extensions::stack::run(&sock),
                    b"disk" => crate::extensions::disk::run(&sock),
                    b"net" => crate::extensions::net::run(&sock),
                    b"env" => crate::extensions::env::run(&sock),
                    b"history" => crate::extensions::history::run(&sock),
                    b"mode" => crate::extensions::mode::run(&sock),
                    b"zoom" => crate::extensions::zoom::run(&sock),
                    b"marks" => crate::extensions::marks::run(&sock),
                    b"alerts" => crate::extensions::alerts::run(&sock),
                    b"titles" => crate::extensions::titles::run(&sock),
                    b"equalize" => crate::extensions::equalize::run(&sock),
                    b"revive" => crate::extensions::respawn::run(&sock),
                    b"clearall" => crate::extensions::clear::run(&sock),
                    b"retitle" => crate::extensions::retitle::run(&sock),
                    b"cwd" => crate::extensions::cwd::run(&sock),
                    b"who" => crate::extensions::who::run(&sock),
                    b"age" => crate::extensions::age::run(&sock),
                    b"cmd" => crate::extensions::cmd::run(&sock),
                    b"dead" => crate::extensions::dead::run(&sock),
                    b"layouts" => crate::extensions::layouts::run(&sock),
                    b"detached" => crate::extensions::detached::run(&sock),
                    b"density" => crate::extensions::density::run(&sock),
                    b"nested" => crate::extensions::nested::run(&sock),
                    b"solo" => crate::extensions::solo::run(&sock),
                    b"shells" => crate::extensions::shells::run(&sock),
                    b"fanout" => crate::extensions::fanout::run(&sock),
                    b"busy" => crate::extensions::busy::run(&sock),
                    b"focus" => crate::extensions::focus::run(&sock),
                    b"named" => crate::extensions::named::run(&sock),
                    b"project" => crate::extensions::project::run(&sock),
                    b"remote" => crate::extensions::remote::run(&sock),
                    b"ahead" => crate::extensions::ahead::run(&sock),
                    b"changes" => crate::extensions::changes::run(&sock),
                    b"stash" => crate::extensions::stash::run(&sock),
                    b"elapsed" => crate::extensions::elapsed::run(&sock),
                    b"mem" => crate::extensions::mem::run(&sock),
                    b"state" => crate::extensions::state::run(&sock),
                    b"commit" => crate::extensions::commit::run(&sock),
                    b"linked" => crate::extensions::linked::run(&sock),
                    b"conflicts" => crate::extensions::conflicts::run(&sock),
                    b"user" => crate::extensions::user::run(&sock),
                    b"tag" => crate::extensions::tag::run(&sock),
                    b"vcs" => crate::extensions::vcs::run(&sock),
                    b"gone" => crate::extensions::gone::run(&sock),
                    b"buffers" => crate::extensions::buffers::run(&sock),
                    b"worktree" => crate::extensions::worktree::run(&sock),
                    b"submodules" => crate::extensions::submodules::run(&sock),
                    b"term" => crate::extensions::term::run(&sock),
                    b"startcmd" => crate::extensions::start::run(&sock),
                    b"writable" => crate::extensions::writable::run(&sock),
                    b"sync" => crate::extensions::sync::run(&sock),
                    b"piped" => crate::extensions::piped::run(&sock),
                    b"input" => crate::extensions::input::run(&sock),
                    b"monitor" => crate::extensions::monitor::run(&sock),
                    b"remain" => crate::extensions::remain::run(&sock),
                    b"autoname" => crate::extensions::autoname::run(&sock),
                    b"readonly" => crate::extensions::readonly::run(&sock),
                    b"idle" => crate::extensions::idle::run(&sock),
                    b"viewers" => crate::extensions::viewers::run(&sock),
                    b"connected" => crate::extensions::connected::run(&sock),
                    b"constrain" => crate::extensions::constrain::run(&sock),
                    b"hooks" => crate::extensions::hooks::run(&sock),
                    b"destroy" => crate::extensions::destroy::run(&sock),
                    b"status" => crate::extensions::status::run(&sock),
                    b"keys" => crate::extensions::keys::run(&sock),
                    b"limit" => crate::extensions::limit::run(&sock),
                    b"winsize" => crate::extensions::winsize::run(&sock),
                    b"borders" => crate::extensions::borders::run(&sock),
                    b"autolock" => crate::extensions::lock::run(&sock),
                    b"titlebar" => crate::extensions::titlebar::run(&sock),
                    b"visual" => crate::extensions::visual::run(&sock),
                    b"keytable" => crate::extensions::keytable::run(&sock),
                    b"control" => crate::extensions::control::run(&sock),
                    b"utf8" => crate::extensions::utf8::run(&sock),
                    b"mouse" => crate::extensions::mouse::run(&sock),
                    _ => crate::extensions::dashboard::run(&sock),
                };
                std::process::exit(code);
            }
        }

        // Pass control to the client.
        std::process::exit(client_main(osdep_event_init(), argc, argv, flags, feat))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Regression (bug 3): `ztmux -V` once reported the crate's placeholder
    // "0.1.0", so version-gated user config (e.g. `tmux -V | awk '{print
    // ($2>=3.1)}'`) evaluated the gate to 0 and sourced the wrong files. The
    // reported version must be a real tmux-like version that clears the common
    // `>= 3.1` gate.
    #[test]
    fn test_getversion_passes_config_version_gate() {
        let v = getversion();

        // Not the old placeholder.
        assert_ne!(v, "0.1.0");

        // Parse "MAJOR.MINOR..." the way an awk gate reads $2 as a float: take
        // everything up to the second '.'.
        let mut it = v.split('.');
        let major: u32 = it.next().unwrap().parse().expect("major version");
        let minor: u32 = it.next().unwrap().parse().expect("minor version");
        let as_float: f64 = format!("{major}.{minor}").parse().unwrap();

        assert!(major >= 3, "major version {major} too low: {v}");
        assert!(
            as_float >= 3.1,
            "version {v} ({as_float}) fails the `>= 3.1` config gate"
        );
    }
}
