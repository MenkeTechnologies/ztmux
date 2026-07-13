// Copyright (c) 2011 Nicholas Marriott <nicholas.marriott@gmail.com>
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

use crate::compat::HOST_NAME_MAX;
use crate::libc::{
    FNM_CASEFOLD, REG_NOSUB, ctime_r, getpwuid, getuid, ispunct, localtime_r, memcpy, regcomp,
    regex_t, regexec, regfree, strchr, strcmp, strcspn, strftime, strstr, strtod, tm,
};
use crate::*;
use crate::options_::*;

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Copy, Clone)]
    pub struct format_flags: i32 {
        const FORMAT_STATUS  = 1;
        const FORMAT_FORCE   = 2;
        const FORMAT_NOJOBS  = 4;
        const FORMAT_VERBOSE = 8;
    }
}

pub const FORMAT_NONE: i32 = 0;
pub const FORMAT_PANE: u32 = 0x80000000u32;
pub const FORMAT_WINDOW: u32 = 0x40000000u32;

pub type format_cb = unsafe fn(_: *mut format_tree) -> format_table_type;

// Entry in format job tree.
#[repr(C)]
pub struct format_job {
    pub client: *mut client,
    pub tag: u32,
    pub cmd: *mut u8,
    pub expanded: *mut u8,

    pub last: time_t,
    pub out: *mut u8,
    pub updated: i32,

    pub job: *mut job,
    pub status: i32,

    pub entry: rb_entry<format_job>,
}

pub type format_job_tree = rb_head<format_job>;

pub static mut FORMAT_JOBS: format_job_tree = rb_initializer();
RB_GENERATE!(
    format_job_tree,
    format_job,
    entry,
    discr_entry,
    format_job_cmp
);

// Format job tree comparison function.
/// C `vendor/tmux/format.c:78`: `static int format_job_cmp(struct format_job *fj1, struct format_job *fj2)`
pub fn format_job_cmp(fj1: &format_job, fj2: &format_job) -> cmp::Ordering {
    unsafe {
        fj1.tag
            .cmp(&fj2.tag)
            .then_with(|| i32_to_ordering(strcmp(fj1.cmd, fj2.cmd)))
    }
}

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Copy, Clone)]
    pub struct format_modifiers : i32 {
        const FORMAT_TIMESTRING = 0x1;
        const FORMAT_BASENAME   = 0x2;
        const FORMAT_DIRNAME    = 0x4;
        const FORMAT_QUOTE_SHELL  = 0x8;
        const FORMAT_LITERAL = 0x10;
        const FORMAT_EXPAND = 0x20;
        const FORMAT_EXPANDTIME = 0x40;
        const FORMAT_SESSIONS = 0x80;
        const FORMAT_WINDOWS = 0x100;
        const FORMAT_PANES = 0x200;
        const FORMAT_PRETTY = 0x400;
        const FORMAT_LENGTH = 0x800;
        const FORMAT_WIDTH = 0x1000;
        const FORMAT_QUOTE_STYLE = 0x2000;
        const FORMAT_WINDOW_NAME = 0x4000;
        const FORMAT_SESSION_NAME = 0x8000;
        const FORMAT_CHARACTER = 0x10000;
        const FORMAT_COLOUR = 0x20000;
        const FORMAT_CLIENTS = 0x40000;
        const FORMAT_NOT = 0x80000;
        const FORMAT_NOT_NOT = 0x100000;
        const FORMAT_REPEAT = 0x200000;
        const FORMAT_COLOUR_ESC_FG = 0x8000000;
        const FORMAT_COLOUR_ESC_BG = 0x10000000;
    }
}

/// C `vendor/tmux/format.c:91`: `#define FORMAT_MAX_REPEAT 10000`
const FORMAT_MAX_REPEAT: i32 = 10000;

/// Limit on recursion.
const FORMAT_LOOP_LIMIT: i32 = 100;

/// C `vendor/tmux/format.c:131`: `#define FORMAT_TIME_LIMIT 100` — a single
/// expansion may run for at most this many milliseconds before it is aborted.
const FORMAT_TIME_LIMIT: u64 = 100;

/// C `vendor/tmux/format.c:134`: `#define FORMAT_TIME_LOOP_CHECK 10000` — only
/// consult the clock once per this many loop iterations, so the guard is cheap.
const FORMAT_TIME_LOOP_CHECK: u32 = 10000;

bitflags::bitflags! {
    /// Format expand flags.
    #[repr(transparent)]
    #[derive(Copy, Clone)]
    pub struct format_expand_flags: i32 {
        const FORMAT_EXPAND_TIME = 0x1;
        const FORMAT_EXPAND_NOJOBS = 0x2;
    }
}

#[repr(i32)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum format_type {
    FORMAT_TYPE_UNKNOWN,
    FORMAT_TYPE_SESSION,
    FORMAT_TYPE_WINDOW,
    FORMAT_TYPE_PANE,
}

// Entry in format tree.
#[repr(C)]
pub struct format_entry {
    pub key: *mut u8,
    pub value: *mut u8,
    pub time: time_t,
    pub cb: Option<format_cb>,
    pub entry: rb_entry<format_entry>,
}

#[repr(C)]
pub struct format_tree {
    pub type_: format_type,

    pub c: *mut client,
    pub s: *mut session,
    pub wl: *mut winlink,
    pub w: *mut window,
    pub wp: *mut window_pane,
    pub pb: *mut paste_buffer,

    pub item: *mut cmdq_item,
    pub client: *mut client,
    pub flags: format_flags,
    pub tag: u32,

    pub m: mouse_event,

    pub tree: format_entry_tree,
}
pub type format_entry_tree = rb_head<format_entry>;
RB_GENERATE!(
    format_entry_tree,
    format_entry,
    entry,
    discr_entry,
    format_entry_cmp
);

/// Format expand state.
#[repr(C)]
pub struct format_expand_state {
    pub ft: *mut format_tree,
    pub loop_: u32,
    pub start_time: u64,
    pub time: time_t,
    pub tm: tm,
    pub flags: format_expand_flags,
}

/// Format modifier.
#[repr(C)]
pub struct format_modifier {
    pub modifier: [u8; 3],
    pub size: u32,

    pub argv: *mut *mut u8,
    pub argc: i32,
}

/// Format entry tree comparison function.
/// C `vendor/tmux/format.c:203`: `static int format_entry_cmp(struct format_entry *fe1, struct format_entry *fe2)`
fn format_entry_cmp(fe1: &format_entry, fe2: &format_entry) -> cmp::Ordering {
    unsafe { i32_to_ordering(strcmp(fe1.key, fe2.key)) }
}

/// Single-character uppercase aliases.
static FORMAT_UPPER: [SyncCharPtr; 26] = const {
    const fn idx(c: char) -> usize {
        (c as u8 - b'A') as usize
    }
    let mut tmp = [SyncCharPtr::null(); 26];

    tmp[idx('D')] = SyncCharPtr::new(c"pane_id");
    tmp[idx('F')] = SyncCharPtr::new(c"window_flags");
    tmp[idx('H')] = SyncCharPtr::new(c"host");
    tmp[idx('I')] = SyncCharPtr::new(c"window_index");
    tmp[idx('P')] = SyncCharPtr::new(c"pane_index");
    tmp[idx('S')] = SyncCharPtr::new(c"session_name");
    tmp[idx('T')] = SyncCharPtr::new(c"pane_title");
    tmp[idx('W')] = SyncCharPtr::new(c"window_name");

    tmp
};

/// Single-character lowercase aliases.
static FORMAT_LOWER: [SyncCharPtr; 26] = const {
    const fn idx(c: char) -> usize {
        (c as u8 - b'a') as usize
    }
    let mut tmp = [SyncCharPtr::null(); 26];
    tmp[idx('h')] = SyncCharPtr::new(c"host_short");
    tmp
};

/// Is logging enabled?
/// C `vendor/tmux/format.c:270`: `static inline int format_logging(struct format_tree *ft)`
pub fn format_logging(ft: &format_tree) -> bool {
    log_get_level() != 0 || ft.flags.intersects(format_flags::FORMAT_VERBOSE)
}

macro_rules! format_log1 {
   ($es:expr, $from:expr, $fmt:literal $(, $args:expr)* $(,)?) => {
        format_log1_($es, $from, format_args!($fmt $(, $args)*))
    };
}

/// Log a message if verbose.
pub unsafe fn format_log1_(
    es: *mut format_expand_state,
    from: *const u8,
    args: std::fmt::Arguments,
) {
    unsafe {
        let ft: *mut format_tree = (*es).ft;
        let spaces = c"          ";

        if !format_logging(&*ft) {
            return;
        }

        let s = args.to_string();

        log_debug!("{}: {}", _s(from), s);
        if !(*ft).item.is_null() && (*ft).flags.intersects(format_flags::FORMAT_VERBOSE) {
            cmdq_print!(
                (*ft).item,
                "#{1:0$}{2}",
                (*es).loop_ as usize,
                _s(spaces.as_ptr()),
                s
            );
        }
    }
}

/// Copy expand state.
/// C `vendor/tmux/format.c:302`: `static void format_copy_state(struct format_expand_state *to, struct format_expand_state *from, int flags)`
pub unsafe fn format_copy_state(
    to: *mut format_expand_state,
    from: *mut format_expand_state,
    flags: format_expand_flags,
) {
    unsafe {
        (*to).ft = (*from).ft;
        (*to).loop_ = (*from).loop_;
        (*to).start_time = (*from).start_time;
        (*to).time = (*from).time;
        memcpy__(&raw mut (*to).tm, &raw const (*from).tm);
        (*to).flags = (*from).flags | flags;
    }
}

/// Format job update callback.
/// C `vendor/tmux/format.c:315`: `static void format_job_update(struct job *job)`
pub unsafe fn format_job_update(job: *mut job) {
    unsafe {
        let fj = job_get_data(job) as *mut format_job;
        let evb: *mut evbuffer = (*job_get_event(job)).input;
        let mut line: *mut u8 = null_mut();

        while let Some(next) = NonNull::new(evbuffer_readline(evb)) {
            free(line.cast());
            line = next.as_ptr();
        }
        if line.is_null() {
            return;
        }
        (*fj).updated = 1;

        free((*fj).out.cast());
        (*fj).out = line;

        log_debug!(
            "{}: {:p} {}: {}",
            function_name!(),
            fj,
            _s((*fj).cmd),
            _s((*fj).out)
        );

        let t = libc::time(null_mut());
        if (*fj).status != 0 && (*fj).last != t {
            if !(*fj).client.is_null() {
                server_status_client((*fj).client);
            }
            (*fj).last = t;
        }
    }
}

/// Format job complete callback.
/// C `vendor/tmux/format.c:345`: `static void format_job_complete(struct job *job)`
pub unsafe fn format_job_complete(job: *mut job) {
    unsafe {
        let fj = job_get_data(job) as *mut format_job;
        let evb: *mut evbuffer = (*job_get_event(job)).input;

        (*fj).job = null_mut();

        let buf: *mut u8;

        let line = evbuffer_readline(evb);
        if line.is_null() {
            let len = EVBUFFER_LENGTH(evb);
            buf = xmalloc(len + 1).as_ptr().cast();
            if len != 0 {
                memcpy(buf.cast(), EVBUFFER_DATA(evb).cast(), len);
            }
            *buf.add(len) = b'\0';
        } else {
            buf = line;
        }

        log_debug!(
            "{}: {:p} {}: {}",
            function_name!(),
            fj,
            _s((*fj).cmd),
            _s(buf)
        );

        if *buf != b'\0' || (*fj).updated == 0 {
            free((*fj).out.cast());
            (*fj).out = buf;
        } else {
            free(buf.cast());
        }

        if (*fj).status != 0 {
            if !(*fj).client.is_null() {
                server_status_client((*fj).client);
            }
            (*fj).status = 0;
        }
    }
}

/// C `vendor/tmux/format.c:381`: `static char *format_job_get(struct format_expand_state *es, const char *cmd)`
pub unsafe fn format_job_get(es: *mut format_expand_state, cmd: *mut u8) -> *mut u8 {
    unsafe {
        let ft: *mut format_tree = (*es).ft;
        let mut fj0 = MaybeUninit::<format_job>::uninit();
        let fj0 = fj0.as_mut_ptr();

        let jobs = if (*ft).client.is_null() {
            &raw mut FORMAT_JOBS
        } else if !(*(*ft).client).jobs.is_null() {
            (*(*ft).client).jobs
        } else {
            (*(*ft).client).jobs = Box::leak(Box::new(zeroed())) as *mut format_job_tree;
            rb_init((*(*ft).client).jobs);
            (*(*ft).client).jobs
        };

        (*fj0).tag = (*ft).tag;
        (*fj0).cmd = cmd;
        let mut fj = rb_find(jobs, fj0);
        if fj.is_null() {
            fj = xcalloc1() as *mut format_job;
            (*fj).client = (*ft).client;
            (*fj).tag = (*ft).tag;
            (*fj).cmd = xstrdup(cmd).as_ptr();

            rb_insert(jobs, fj);
        }

        let mut next = MaybeUninit::<format_expand_state>::uninit();
        let next = next.as_mut_ptr();
        format_copy_state(next, es, format_expand_flags::FORMAT_EXPAND_NOJOBS);
        (*next).flags &= !format_expand_flags::FORMAT_EXPAND_TIME;

        let expanded = format_expand1(next, cmd);

        let force = if (*fj).expanded.is_null() || strcmp(expanded, (*fj).expanded) != 0 {
            free((*fj).expanded.cast());
            (*fj).expanded = xstrdup(expanded).as_ptr();
            true
        } else {
            (*ft).flags.intersects(format_flags::FORMAT_FORCE)
        };

        let t = libc::time(null_mut());
        if force && !(*fj).job.is_null() {
            job_free((*fj).job);
        }
        if force || ((*fj).job.is_null() && (*fj).last != t) {
            (*fj).job = job_run(
                expanded,
                0,
                null_mut(),
                null_mut(),
                null_mut(),
                server_client_get_cwd((*ft).client, null_mut()),
                Some(format_job_update),
                Some(format_job_complete),
                None,
                fj.cast(),
                job_flag::JOB_NOWAIT,
                -1,
                -1,
            );
            if (*fj).job.is_null() {
                free((*fj).out.cast());
                (*fj).out = format_nul!("<'{}' didn't start>", _s((*fj).cmd),);
            }
            (*fj).last = t;
            (*fj).updated = 0;
        } else if !(*fj).job.is_null() && (t - (*fj).last) > 1 && (*fj).out.is_null() {
            (*fj).out = format_nul!("<'{}' not ready>", _s((*fj).cmd));
        }
        free(expanded.cast());

        if (*ft).flags.intersects(format_flags::FORMAT_STATUS) {
            (*fj).status = 1;
        }
        if (*fj).out.is_null() {
            return xstrdup_(c"").as_ptr();
        }

        format_expand1(next, (*fj).out)
    }
}

/// C `vendor/tmux/format.c:448`: `static void format_job_tidy(struct format_job_tree *jobs, int force)`
pub unsafe fn format_job_tidy(jobs: *mut format_job_tree, force: i32) {
    unsafe {
        let now = libc::time(null_mut());
        for fj in rb_foreach(jobs) {
            let fj = fj.as_ptr();
            if force == 0 && ((*fj).last > now || now - (*fj).last < 3600) {
                continue;
            }
            rb_remove(jobs, fj);

            log_debug!("{}: {}", "format_job_tidy", _s((*fj).cmd));

            if !(*fj).job.is_null() {
                job_free((*fj).job);
            }

            free_((*fj).expanded);
            free_((*fj).cmd);
            free_((*fj).out);

            free_(fj);
        }
    }
}

/// C `vendor/tmux/format.c:488`: `void format_tidy_jobs(void)`
pub unsafe fn format_tidy_jobs() {
    unsafe {
        format_job_tidy(&raw mut FORMAT_JOBS, 0);
        for c in tailq_foreach(&raw mut CLIENTS).map(NonNull::as_ptr) {
            if !(*c).jobs.is_null() {
                format_job_tidy((*c).jobs, 0);
            }
        }
    }
}

/// C `vendor/tmux/format.c:501`: `void format_lost_client(struct client *c)`
pub unsafe fn format_lost_client(c: *mut client) {
    unsafe {
        if !(*c).jobs.is_null() {
            format_job_tidy((*c).jobs, 1);
        }
        free_((*c).jobs);
    }
}

/// C `vendor/tmux/format.c:523`: `static void *format_cb_host(__unused struct format_tree *ft)`
pub unsafe fn format_cb_host(_ft: *mut format_tree) -> format_table_type {
    unsafe {
        let mut host = MaybeUninit::<[u8; HOST_NAME_MAX + 1]>::uninit();

        if libc::gethostname(host.as_mut_ptr().cast(), HOST_NAME_MAX + 1) != 0 {
            "".into()
        } else {
            format!("{}", _s(host.as_ptr().cast::<u8>())).into()
        }
    }
}

/// Callback for `host_short`.
/// C `vendor/tmux/format.c:534`: `static void *format_cb_host_short(__unused struct format_tree *ft)`
pub unsafe fn format_cb_host_short(_ft: *mut format_tree) -> format_table_type {
    unsafe {
        let mut host = MaybeUninit::<[u8; HOST_NAME_MAX + 1]>::uninit();

        if libc::gethostname(host.as_mut_ptr().cast(), HOST_NAME_MAX + 1) != 0 {
            return "".into();
        }

        let cp = strchr(host.as_mut_ptr().cast(), b'.' as i32);
        if !cp.is_null() {
            *cp = b'\0';
        }
        format!("{}", _s(&raw const host as *const u8)).into()
    }
}

/// Callback for pid.
/// C `vendor/tmux/format.c:547`: `static void *format_cb_pid(__unused struct format_tree *ft)`
pub unsafe fn format_cb_pid(_ft: *mut format_tree) -> format_table_type {
    unsafe { format!("{}", libc::getpid()).into() }
}

/// Callback for `session_attached_list`.
/// C `vendor/tmux/format.c:557`: `static void *format_cb_session_attached_list(struct format_tree *ft)`
pub unsafe fn format_cb_session_attached_list(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let s = (*ft).s;

        if s.is_null() {
            return format_table_type::None;
        }

        let buffer = evbuffer_new();
        if buffer.is_null() {
            fatalx("out of memory");
        }

        for loop_ in tailq_foreach(&raw mut CLIENTS).map(NonNull::as_ptr) {
            if (*loop_).session == s {
                if EVBUFFER_LENGTH(buffer) > 0 {
                    evbuffer_add(buffer, c!(",").cast(), 1);
                }
                evbuffer_add_printf!(buffer, "{}", _s((*loop_).name));
            }
        }

        let size = EVBUFFER_LENGTH(buffer);
        let result = if size != 0 {
            format!("{1:0$}", size, _s(EVBUFFER_DATA(buffer).cast::<u8>())).into()
        } else {
            format_table_type::None
        };
        evbuffer_free(buffer);
        result
    }
}

/// Callback for `session_alerts`.
/// Callback for `session_alert`.
/// C `vendor/tmux/format.c:588`: `static void *format_cb_session_alert(struct format_tree *ft)`
pub unsafe fn format_cb_session_alert(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let s: *mut session = (*ft).s;
        const SIZEOF_ALERTS: usize = 1024;
        let mut alerts = MaybeUninit::<[u8; 1024]>::uninit();
        let alerts: *mut u8 = alerts.as_mut_ptr().cast();
        let mut alerted = winlink_flags::empty();

        if s.is_null() {
            return format_table_type::None;
        }

        *alerts = b'\0';
        for wl in rb_foreach(&raw mut (*s).windows).map(NonNull::as_ptr) {
            if !(*wl).flags.intersects(WINLINK_ALERTFLAGS) {
                continue;
            }
            if !alerted.intersects(winlink_flags::WINLINK_ACTIVITY)
                && (*wl).flags.intersects(winlink_flags::WINLINK_ACTIVITY)
            {
                strlcat(alerts, c!("#"), SIZEOF_ALERTS);
                alerted |= winlink_flags::WINLINK_ACTIVITY;
            }
            if !alerted.intersects(winlink_flags::WINLINK_BELL)
                && (*wl).flags.intersects(winlink_flags::WINLINK_BELL)
            {
                strlcat(alerts, c!("!"), SIZEOF_ALERTS);
                alerted |= winlink_flags::WINLINK_BELL;
            }
            if !alerted.intersects(winlink_flags::WINLINK_SILENCE)
                && (*wl).flags.intersects(winlink_flags::WINLINK_SILENCE)
            {
                strlcat(alerts, c!("~"), SIZEOF_ALERTS);
                alerted |= winlink_flags::WINLINK_SILENCE;
            }
        }
        format!("{}", _s(alerts)).into()
    }
}

/// C `vendor/tmux/format.c:620`: `static void *format_cb_session_alerts(struct format_tree *ft)`
pub unsafe fn format_cb_session_alerts(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let s: *mut session = (*ft).s;
        const SIZEOF_ALERTS: usize = 1024;
        const SIZEOF_TMP: usize = 16;
        let mut alerts = MaybeUninit::<[u8; 1024]>::uninit();
        let alerts: *mut u8 = alerts.as_mut_ptr().cast();
        let mut tmp = MaybeUninit::<[u8; 16]>::uninit();
        let tmp: *mut u8 = tmp.as_mut_ptr().cast();

        if s.is_null() {
            return format_table_type::None;
        }

        *alerts = b'\0';
        for wl in rb_foreach(&raw mut (*s).windows).map(NonNull::as_ptr) {
            if !(*wl).flags.intersects(WINLINK_ALERTFLAGS) {
                continue;
            }
            _ = xsnprintf_!(tmp, SIZEOF_TMP, "{}", (*wl).idx);

            if *alerts != b'\0' {
                strlcat(alerts, c!(","), SIZEOF_ALERTS);
            }
            strlcat(alerts, tmp, SIZEOF_ALERTS);
            if (*wl).flags.intersects(winlink_flags::WINLINK_ACTIVITY) {
                strlcat(alerts, c!("#"), SIZEOF_ALERTS);
            }
            if (*wl).flags.intersects(winlink_flags::WINLINK_BELL) {
                strlcat(alerts, c!("!"), SIZEOF_ALERTS);
            }
            if (*wl).flags.intersects(winlink_flags::WINLINK_SILENCE) {
                strlcat(alerts, c!("~"), SIZEOF_ALERTS);
            }
        }
        format!("{}", _s(alerts)).into()
    }
}

/// Callback for `session_activity_flag`.
/// C `vendor/tmux/format.c:2526`: `static void *format_cb_session_activity_flag(struct format_tree *ft)`
pub unsafe fn format_cb_session_activity_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).s.is_null() {
            // Mirrors vendor/tmux/format.c: RB_FOREACH here returns on its first
            // iteration (checking ft->wl, not the loop element), so only the
            // first winlink is inspected. `.next()` preserves that exactly.
            if rb_foreach(&raw mut (*(*ft).s).windows).next().is_some() {
                if (*(*ft).wl).flags.intersects(winlink_flags::WINLINK_ACTIVITY) {
                    return "1".into();
                }
                return "0".into();
            }
        }
        format_table_type::None
    }
}

/// Callback for `session_bell_flag`.
/// C `vendor/tmux/format.c:2543`: `static void *format_cb_session_bell_flag(struct format_tree *ft)`
pub unsafe fn format_cb_session_bell_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).s.is_null() {
            // Mirrors vendor/tmux/format.c: only the first winlink is inspected
            // (the C loop body returns unconditionally). `.next()` preserves it.
            if let Some(wl) = rb_foreach(&raw mut (*(*ft).s).windows)
                .map(NonNull::as_ptr)
                .next()
            {
                if (*wl).flags.intersects(winlink_flags::WINLINK_BELL) {
                    return "1".into();
                }
                return "0".into();
            }
        }
        format_table_type::None
    }
}

/// Callback for `session_silence_flag`.
/// C `vendor/tmux/format.c:2559`: `static void *format_cb_session_silence_flag(struct format_tree *ft)`
pub unsafe fn format_cb_session_silence_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).s.is_null() {
            // Mirrors vendor/tmux/format.c: RB_FOREACH here returns on its first
            // iteration (checking ft->wl, not the loop element). `.next()` keeps
            // that behavior.
            if rb_foreach(&raw mut (*(*ft).s).windows).next().is_some() {
                if (*(*ft).wl).flags.intersects(winlink_flags::WINLINK_SILENCE) {
                    return "1".into();
                }
                return "0".into();
            }
        }
        format_table_type::None
    }
}

/// Callback for `bracket_paste_flag`.
/// C `vendor/tmux/format.c:1712`: `static void *format_cb_bracket_paste_flag(struct format_tree *ft)`
pub unsafe fn format_cb_bracket_paste_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() && !(*(*ft).wp).screen.is_null() {
            if (*(*(*ft).wp).screen)
                .mode
                .intersects(mode_flag::MODE_BRACKETPASTE)
            {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `sixel_support`.
/// C `vendor/tmux/format.c:1681`: `static void *format_cb_sixel_support(__unused struct format_tree *ft)`.
/// ztmux is built without sixel support (no `ENABLE_SIXEL`), so this is always
/// `"0"` — matching the `#else` arm of the C.
pub unsafe fn format_cb_sixel_support(_ft: *mut format_tree) -> format_table_type {
    "0".into()
}

/// Callback for `synchronized_output_flag`.
/// C `vendor/tmux/format.c`: `static void *format_cb_synchronized_output_flag(struct format_tree *ft)`
pub unsafe fn format_cb_synchronized_output_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp).base.mode.intersects(mode_flag::MODE_SYNC) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_zoomed_flag`.
/// C `vendor/tmux/format.c:2472`: `static void *format_cb_pane_zoomed_flag(struct format_tree *ft)`
pub unsafe fn format_cb_pane_zoomed_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let wp = (*ft).wp;
        if !wp.is_null() {
            if (*wp).flags.intersects(window_pane_flags::PANE_ZOOMED) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `session_stack`.
/// C `vendor/tmux/format.c:650`: `static void *format_cb_session_stack(struct format_tree *ft)`
pub unsafe fn format_cb_session_stack(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let s = (*ft).s;
        const SIZEOF_RESULT: usize = 1024;
        const SIZEOF_TMP: usize = 16;

        let mut result = MaybeUninit::<[u8; 1024]>::uninit();
        let result: *mut u8 = result.as_mut_ptr().cast();
        let mut tmp = MaybeUninit::<[u8; 16]>::uninit();
        let tmp: *mut u8 = tmp.as_mut_ptr().cast();

        if s.is_null() {
            return format_table_type::None;
        }

        _ = xsnprintf_!(result, SIZEOF_RESULT, "{}", (*(*s).curw).idx);
        for wl in tailq_foreach::<_, discr_sentry>(&raw mut (*s).lastw).map(NonNull::as_ptr) {
            _ = xsnprintf_!(tmp, SIZEOF_TMP, "{}", (*wl).idx);

            if *result != b'\0' {
                strlcat(result, c!(","), SIZEOF_RESULT);
            }
            strlcat(result, tmp, SIZEOF_RESULT);
        }
        format!("{}", _s(result)).into()
    }
}

/// Callback for `window_stack_index`.
/// C `vendor/tmux/format.c:672`: `static void *format_cb_window_stack_index(struct format_tree *ft)`
pub unsafe fn format_cb_window_stack_index(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if (*ft).wl.is_null() {
            return format_table_type::None;
        }
        let s = (*(*ft).wl).session;

        let mut idx: u32 = 0;
        let mut wl = null_mut();
        for wl_ in tailq_foreach::<_, discr_sentry>(&raw mut (*s).lastw).map(NonNull::as_ptr) {
            wl = wl_;
            idx += 1;
            if wl == (*ft).wl {
                break;
            }
        }
        if wl.is_null() {
            return "0".into();
        }
        format!("{idx}").into()
    }
}

/// Callback for `window_linked_sessions_list`.
/// C `vendor/tmux/format.c:697`: `static void *format_cb_window_linked_sessions_list(struct format_tree *ft)`
pub unsafe fn format_cb_window_linked_sessions_list(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if (*ft).wl.is_null() {
            return format_table_type::None;
        }
        let w = (*(*ft).wl).window;

        let buffer = evbuffer_new();
        if buffer.is_null() {
            fatalx("out of memory");
        }

        for wl in tailq_foreach::<_, discr_wentry>(&raw mut (*w).winlinks).map(NonNull::as_ptr) {
            if EVBUFFER_LENGTH(buffer) > 0 {
                evbuffer_add(buffer, c!(",").cast(), 1);
            }
            evbuffer_add_printf!(buffer, "{}", (*(*wl).session).name);
        }

        let size = EVBUFFER_LENGTH(buffer);
        let mut value = format_table_type::None;
        if size != 0 {
            value = format_table_type::String(
                format!("{1:0$}", size, _s(EVBUFFER_DATA(buffer).cast::<u8>())).into(),
            );
        }
        evbuffer_free(buffer);
        value
    }
}

/// Callback for `window_active_sessions`.
/// C `vendor/tmux/format.c:727`: `static void *format_cb_window_active_sessions(struct format_tree *ft)`
pub unsafe fn format_cb_window_active_sessions(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if (*ft).wl.is_null() {
            return format_table_type::None;
        }
        let w = (*(*ft).wl).window;

        let n = tailq_foreach::<_, discr_wentry>(&raw mut (*w).winlinks)
            .filter(|wl| (*(*wl.as_ptr()).session).curw == wl.as_ptr())
            .count() as u32;

        format!("{n}").into()
    }
}

/// Callback for `window_active_sessions_list`.
/// C `vendor/tmux/format.c:749`: `static void *format_cb_window_active_sessions_list(struct format_tree *ft)`
pub unsafe fn format_cb_window_active_sessions_list(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if (*ft).wl.is_null() {
            return format_table_type::None;
        }
        let w = (*(*ft).wl).window;

        let buffer = evbuffer_new();
        if buffer.is_null() {
            fatalx("out of memory");
        }

        for wl in tailq_foreach::<_, discr_wentry>(&raw mut (*w).winlinks).map(NonNull::as_ptr) {
            if (*(*wl).session).curw == wl {
                if EVBUFFER_LENGTH(buffer) > 0 {
                    evbuffer_add(buffer, c!(",").cast(), 1);
                }
                evbuffer_add_printf!(buffer, "{}", (*(*wl).session).name);
            }
        }

        let size = EVBUFFER_LENGTH(buffer);
        let mut value = format_table_type::None;
        if size != 0 {
            value = format_table_type::String(
                format!("{1:0$}", size, _s(EVBUFFER_DATA(buffer).cast::<u8>())).into(),
            );
        }
        evbuffer_free(buffer);
        value
    }
}

/// Callback for `window_active_clients`.
/// C `vendor/tmux/format.c:781`: `static void *format_cb_window_active_clients(struct format_tree *ft)`
pub unsafe fn format_cb_window_active_clients(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if (*ft).wl.is_null() {
            return format_table_type::None;
        }
        let w = (*(*ft).wl).window;

        let mut n = 0u32;
        for loop_ in tailq_foreach(&raw mut CLIENTS).map(NonNull::as_ptr) {
            let client_session = (*loop_).session;
            if client_session.is_null() {
                continue;
            }

            if w == (*(*client_session).curw).window {
                n += 1;
            }
        }

        format!("{n}").into()
    }
}

/// Callback for `window_active_clients_list`.
/// C `vendor/tmux/format.c:808`: `static void *format_cb_window_active_clients_list(struct format_tree *ft)`
pub unsafe fn format_cb_window_active_clients_list(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if (*ft).wl.is_null() {
            return format_table_type::None;
        }
        let w = (*(*ft).wl).window;

        let buffer = evbuffer_new();
        if buffer.is_null() {
            fatalx("out of memory");
        }

        for loop_ in tailq_foreach(&raw mut CLIENTS).map(NonNull::as_ptr) {
            let client_session = (*loop_).session;
            if client_session.is_null() {
                continue;
            }

            if w == (*(*client_session).curw).window {
                if EVBUFFER_LENGTH(buffer) > 0 {
                    evbuffer_add(buffer, c!(",").cast(), 1);
                }
                evbuffer_add_printf!(buffer, "{}", _s((*loop_).name));
            }
        }

        let size = EVBUFFER_LENGTH(buffer);
        let mut value = format_table_type::None;
        if size != 0 {
            value = format_table_type::String(
                format!("{1:0$}", size, _s(EVBUFFER_DATA(buffer).cast::<u8>())).into(),
            );
        }
        evbuffer_free(buffer);
        value
    }
}

/// Callback for `window_layout`.
/// C `vendor/tmux/format.c:845`: `static void *format_cb_window_layout(struct format_tree *ft)`
pub unsafe fn format_cb_window_layout(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let w = (*ft).w;

        if w.is_null() {
            return format_table_type::None;
        }

        if !(*w).saved_layout_root.is_null() {
            return layout_dump(w, (*w).saved_layout_root)
                .map(Into::into)
                .unwrap_or_default();
        }
        layout_dump(w, (*w).layout_root)
            .map(Into::into)
            .unwrap_or_default()
    }
}

/// Callback for `window_visible_layout`.
/// C `vendor/tmux/format.c:859`: `static void *format_cb_window_visible_layout(struct format_tree *ft)`
pub unsafe fn format_cb_window_visible_layout(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let w = (*ft).w;

        if w.is_null() {
            return format_table_type::None;
        }

        layout_dump(w, (*w).layout_root)
            .map(Into::into)
            .unwrap_or_default()
    }
}

/// Callback for `pane_start_command`.
/// C `vendor/tmux/format.c:871`: `static void *format_cb_start_command(struct format_tree *ft)`
pub unsafe fn format_cb_start_command(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let wp = (*ft).wp;

        if wp.is_null() {
            return format_table_type::None;
        }

        cmd_stringify_argv((*wp).argc, (*wp).argv).into()
    }
}

/// Callback for `pane_start_path`.
/// C `vendor/tmux/format.c:883`: `static void *format_cb_start_path(struct format_tree *ft)`
pub unsafe fn format_cb_start_path(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let wp = (*ft).wp;

        if wp.is_null() {
            return format_table_type::None;
        }

        if (*wp).cwd.is_none() {
            return "".into();
        }
        format!("{}", _s((*wp).cwd_ptr())).into()
    }
}

/// Callback for `pane_current_command`.
/// C `vendor/tmux/format.c:897`: `static void *format_cb_current_command(struct format_tree *ft)`
pub unsafe fn format_cb_current_command(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let wp = (*ft).wp;

        if wp.is_null() || (*wp).shell.is_none() {
            return format_table_type::None;
        }

        let mut cmd = osdep_get_name((*wp).fd, (*wp).tty.as_ptr());
        if cmd.is_null() || *cmd == b'\0' {
            free_(cmd);
            cmd = CString::new(cmd_stringify_argv((*wp).argc, (*wp).argv))
                .unwrap()
                .into_raw()
                .cast();
            if cmd.is_null() || *cmd == b'\0' {
                free_(cmd);
                cmd = xstrdup((*wp).shell_ptr()).as_ptr().cast();
            }
        }
        let value = parse_window_name(cmd);
        free_(cmd);
        value.into()
    }
}

/// Callback for `pane_current_path`.
/// C `vendor/tmux/format.c:921`: `static void *format_cb_current_path(struct format_tree *ft)`
pub unsafe fn format_cb_current_path(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let wp = (*ft).wp;

        if wp.is_null() {
            return format_table_type::None;
        }

        let cwd = osdep_get_cwd((*wp).fd);
        if cwd.is_null() {
            return format_table_type::None;
        }
        format!("{}", _s(cwd)).into()
    }
}

/// Callback for `history_bytes`.
/// C `vendor/tmux/format.c:937`: `static void *format_cb_history_bytes(struct format_tree *ft)`
pub unsafe fn format_cb_history_bytes(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let wp = (*ft).wp;

        if wp.is_null() {
            return format_table_type::None;
        }

        let gd = (*wp).base.grid;
        let mut size: usize = 0;

        for i in 0..((*gd).hsize + (*gd).sy) {
            let gl = grid_get_line(gd, i);
            size += (*gl).cellsize as usize * std::mem::size_of::<grid_cell>();
            size += (*gl).extdsize as usize * std::mem::size_of::<grid_cell>();
        }
        size += ((*gd).hsize + (*gd).sy) as usize * std::mem::size_of::<grid_line>();

        format!("{size}").into()
    }
}

/// Callback for `history_all_bytes`.
/// C `vendor/tmux/format.c:963`: `static void *format_cb_history_all_bytes(struct format_tree *ft)`
pub unsafe fn format_cb_history_all_bytes(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let wp = (*ft).wp;

        if wp.is_null() {
            return format_table_type::None;
        }

        let gd = (*wp).base.grid;
        let lines = (*gd).hsize + (*gd).sy;
        let mut cells = 0;
        let mut extended_cells = 0;

        for i in 0..lines {
            let gl = grid_get_line(gd, i);
            cells += (*gl).cellsize;
            extended_cells += (*gl).extdsize;
        }

        format!(
            "{},{},{},{},{},{}",
            lines,
            lines as usize * std::mem::size_of::<grid_line>(),
            cells,
            cells as usize * std::mem::size_of::<grid_cell>(),
            extended_cells,
            extended_cells as usize * std::mem::size_of::<grid_cell>(),
        )
        .into()
    }
}

/// Callback for `pane_tabs`.
/// C `vendor/tmux/format.c:990`: `static void *format_cb_pane_tabs(struct format_tree *ft)`
pub unsafe fn format_cb_pane_tabs(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let wp = (*ft).wp;

        if wp.is_null() {
            return format_table_type::None;
        }

        let buffer = evbuffer_new();
        if buffer.is_null() {
            fatalx("out of memory");
        }

        let mut first = true;
        for i in 0..(*(*wp).base.grid).sx {
            if !(*wp).base.tabs.as_ref().unwrap().borrow().bit_test(i) {
                continue;
            }

            if !first {
                evbuffer_add(buffer, c!(",").cast(), 1);
            }
            evbuffer_add_printf!(buffer, "{i}");
            first = false;
        }

        let size = EVBUFFER_LENGTH(buffer);
        let result = if size != 0 {
            format!("{}", _s(EVBUFFER_DATA(buffer).cast::<u8>())).into()
        } else {
            format_table_type::None
        };
        evbuffer_free(buffer);
        result
    }
}

/// Callback for `pane_fg`.
/// C `vendor/tmux/format.c:1020`: `static void *format_cb_pane_fg(struct format_tree *ft)`
pub unsafe fn format_cb_pane_fg(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let wp = (*ft).wp;
        let mut gc = MaybeUninit::<grid_cell>::uninit();

        if wp.is_null() {
            return format_table_type::None;
        }

        tty_default_colours(gc.as_mut_ptr(), wp);

        colour_tostring((*gc.as_ptr()).fg).into()
    }
}

/// Callback for `pane_bg`.
/// C `vendor/tmux/format.c:1057`: `static void *format_cb_pane_bg(struct format_tree *ft)`
pub unsafe fn format_cb_pane_bg(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let wp = (*ft).wp;
        let mut gc = MaybeUninit::<grid_cell>::uninit();

        if wp.is_null() {
            return format_table_type::None;
        }

        tty_default_colours(gc.as_mut_ptr(), wp);

        colour_tostring((*gc.as_ptr()).bg).into()
    }
}

/// Callback for `session_group_list`.
/// C `vendor/tmux/format.c:1071`: `static void *format_cb_session_group_list(struct format_tree *ft)`
pub unsafe fn format_cb_session_group_list(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let s = (*ft).s;
        if s.is_null() {
            return format_table_type::None;
        }

        let sg = session_group_contains(s);
        if sg.is_null() {
            return format_table_type::None;
        }

        let buffer = evbuffer_new();
        if buffer.is_null() {
            fatalx("out of memory");
        }

        for loop_ in tailq_foreach(&raw mut (*sg).sessions).map(NonNull::as_ptr) {
            if EVBUFFER_LENGTH(buffer) > 0 {
                evbuffer_add(buffer, c!(",").cast(), 1);
            }
            evbuffer_add_printf!(buffer, "{}", (*loop_).name);
        }

        let size = EVBUFFER_LENGTH(buffer);
        let result = if size != 0 {
            format!("{1:0$}", size, _s(EVBUFFER_DATA(buffer).cast::<u8>())).into()
        } else {
            format_table_type::None
        };
        evbuffer_free(buffer);
        result
    }
}

/// Callback for `session_group_attached_list`.
/// C `vendor/tmux/format.c:1104`: `static void *format_cb_session_group_attached_list(struct format_tree *ft)`
pub unsafe fn format_cb_session_group_attached_list(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let s = (*ft).s;
        if s.is_null() {
            return format_table_type::None;
        }

        let sg = session_group_contains(s);
        if sg.is_null() {
            return format_table_type::None;
        }

        let buffer = evbuffer_new();
        if buffer.is_null() {
            fatalx("out of memory");
        }

        for loop_ in tailq_foreach(&raw mut CLIENTS).map(NonNull::as_ptr) {
            let client_session = (*loop_).session;
            if client_session.is_null() {
                continue;
            }

            for session_loop in tailq_foreach(&raw mut (*sg).sessions).map(NonNull::as_ptr) {
                if session_loop == client_session {
                    if EVBUFFER_LENGTH(buffer) > 0 {
                        evbuffer_add(buffer, c!(",").cast(), 1);
                    }
                    evbuffer_add_printf!(buffer, "{}", _s((*loop_).name));
                }
            }
        }

        let size = EVBUFFER_LENGTH(buffer);
        let result = if size != 0 {
            format!("{1:0$}", size, _s(EVBUFFER_DATA(buffer).cast::<u8>())).into()
        } else {
            format_table_type::None
        };
        evbuffer_free(buffer);
        result
    }
}

/// Callback for `pane_in_mode`.
/// C `vendor/tmux/format.c:1144`: `static void *format_cb_pane_in_mode(struct format_tree *ft)`
pub unsafe fn format_cb_pane_in_mode(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let wp = (*ft).wp;
        if wp.is_null() {
            return format_table_type::None;
        }

        let n = tailq_foreach(&raw mut (*wp).modes).count() as u32;

        format!("{n}").into()
    }
}

/// Callback for `pane_at_top`.
/// C `vendor/tmux/format.c:1162`: `static void *format_cb_pane_at_top(struct format_tree *ft)`
pub unsafe fn format_cb_pane_at_top(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let wp = (*ft).wp;
        if wp.is_null() {
            return format_table_type::None;
        }

        let w = (*wp).window;
        let status: i64 = options_get_number___(&*(*w).options, "pane-border-status");
        let flag = if status == pane_status::PANE_STATUS_TOP as i64 {
            (*wp).yoff == 1
        } else {
            (*wp).yoff == 0
        };

        // C: xasprintf(&value, "%d", flag) — 1/0, not Rust's true/false.
        format!("{}", i32::from(flag)).into()
    }
}

/// Callback for `pane_at_bottom`.
/// C `vendor/tmux/format.c:1182`: `static void *format_cb_pane_at_bottom(struct format_tree *ft)`
pub unsafe fn format_cb_pane_at_bottom(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let wp = (*ft).wp;
        if wp.is_null() {
            return format_table_type::None;
        }

        let w = (*wp).window;
        let status: i64 = options_get_number___(&*(*w).options, "pane-border-status");
        let flag = if status == pane_status::PANE_STATUS_BOTTOM as i64 {
            (*wp).yoff + (*wp).sy == (*w).sy - 1
        } else {
            (*wp).yoff + (*wp).sy == (*w).sy
        };

        // C: xasprintf(&value, "%d", flag) — 1/0, not Rust's true/false.
        format!("{}", i32::from(flag)).into()
    }
}

/// Callback for `cursor_character`.
/// C `vendor/tmux/format.c:1204`: `static void *format_cb_cursor_character(struct format_tree *ft)`
pub unsafe fn format_cb_cursor_character(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let wp = (*ft).wp;
        if wp.is_null() {
            return format_table_type::None;
        }
        let mut gc = MaybeUninit::<grid_cell>::uninit();
        grid_view_get_cell(
            (*wp).base.grid,
            (*wp).base.cx,
            (*wp).base.cy,
            gc.as_mut_ptr(),
        );
        if !(*gc.as_ptr()).flags.intersects(grid_flag::PADDING) {
            format!(
                "{1:0$}",
                (*gc.as_ptr()).data.size as usize,
                _s((&raw const (*gc.as_ptr()).data.data).cast::<u8>())
            )
            .into()
        } else {
            format_table_type::None
        }
    }
}

/// Callback for `mouse_word`.
/// C `vendor/tmux/format.c:1235`: `static void *format_cb_mouse_word(struct format_tree *ft)`
pub unsafe fn format_cb_mouse_word(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).m.valid {
            return format_table_type::None;
        }
        let Some(wp) = cmd_mouse_pane(&raw mut (*ft).m, null_mut(), null_mut()) else {
            return format_table_type::None;
        };
        let mut x = 0;
        let mut y = 0;
        if cmd_mouse_at(wp.as_ptr(), &raw mut (*ft).m, &mut x, &mut y, 0) != 0 {
            return format_table_type::None;
        }

        if !tailq_empty(&raw mut (*wp.as_ptr()).modes) {
            if window_pane_mode(wp.as_ptr()) != WINDOW_PANE_NO_MODE {
                return window_copy_get_word(wp.as_ptr(), x, y).into();
            }
            return format_table_type::None;
        }
        let gd = (*wp.as_ptr()).base.grid;
        format_grid_word(gd, x, (*gd).hsize + y).into()
    }
}

/// Callback for `mouse_hyperlink`.
/// C `vendor/tmux/format.c:1260`: `static void *format_cb_mouse_hyperlink(struct format_tree *ft)`
pub unsafe fn format_cb_mouse_hyperlink(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).m.valid {
            return format_table_type::None;
        }
        let Some(wp) = cmd_mouse_pane(&raw mut (*ft).m, null_mut(), null_mut()) else {
            return format_table_type::None;
        };
        let mut x = 0;
        let mut y = 0;
        if cmd_mouse_at(wp.as_ptr(), &raw mut (*ft).m, &mut x, &mut y, 0) != 0 {
            return format_table_type::None;
        }
        let gd = (*wp.as_ptr()).base.grid;
        format_grid_hyperlink(gd, x, (*gd).hsize + y, (*wp.as_ptr()).screen)
            .map(Into::into)
            .unwrap_or_default()
    }
}

/// Callback for `mouse_line`.
/// C `vendor/tmux/format.c:1285`: `static void *format_cb_mouse_line(struct format_tree *ft)`
pub unsafe fn format_cb_mouse_line(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).m.valid {
            return format_table_type::None;
        }
        let Some(wp) = cmd_mouse_pane(&raw mut (*ft).m, null_mut(), null_mut()) else {
            return format_table_type::None;
        };
        let mut x = 0;
        let mut y = 0;
        if cmd_mouse_at(wp.as_ptr(), &raw mut (*ft).m, &mut x, &mut y, 0) != 0 {
            return format_table_type::None;
        }

        if !tailq_empty(&raw mut (*wp.as_ptr()).modes) {
            if window_pane_mode(wp.as_ptr()) != WINDOW_PANE_NO_MODE {
                return window_copy_get_line(wp.as_ptr(), y).into();
            }
            return format_table_type::None;
        }
        let gd = (*wp.as_ptr()).base.grid;
        format_grid_line(gd, (*gd).hsize + y).into()
    }
}

/// Callback for `mouse_status_line`.
/// C `vendor/tmux/format.c:1310`: `static void *format_cb_mouse_status_line(struct format_tree *ft)`
pub unsafe fn format_cb_mouse_status_line(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).m.valid {
            return format_table_type::None;
        }
        if (*ft).c.is_null() || !(*(*ft).c).tty.flags.intersects(tty_flags::TTY_STARTED) {
            return format_table_type::None;
        }

        let y = if (*ft).m.statusat == 0 && (*ft).m.y < (*ft).m.statuslines {
            (*ft).m.y
        } else if (*ft).m.statusat > 0 && (*ft).m.y >= (*ft).m.statusat as u32 {
            (*ft).m.y - (*ft).m.statusat as u32
        } else {
            return format_table_type::None;
        };

        format!("{y}").into()
    }
}

/// Callback for `mouse_status_range`.
/// C `vendor/tmux/format.c:1333`: `static void *format_cb_mouse_status_range(struct format_tree *ft)`
pub unsafe fn format_cb_mouse_status_range(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).m.valid {
            return format_table_type::None;
        }
        if (*ft).c.is_null() || !(*(*ft).c).tty.flags.intersects(tty_flags::TTY_STARTED) {
            return format_table_type::None;
        }

        let x;
        let y;
        if (*ft).m.statusat == 0 && (*ft).m.y < (*ft).m.statuslines {
            x = (*ft).m.x;
            y = (*ft).m.y;
        } else if (*ft).m.statusat > 0 && (*ft).m.y >= (*ft).m.statusat as u32 {
            x = (*ft).m.x;
            y = (*ft).m.y - (*ft).m.statusat as u32;
        } else {
            return format_table_type::None;
        }

        let sr = status_get_range((*ft).c, x, y);
        if sr.is_null() {
            return format_table_type::None;
        }

        match (*sr).type_ {
            style_range_type::STYLE_RANGE_NONE => format_table_type::None,
            style_range_type::STYLE_RANGE_LEFT => "left".into(),
            style_range_type::STYLE_RANGE_RIGHT => "right".into(),
            style_range_type::STYLE_RANGE_PANE => "pane".into(),
            style_range_type::STYLE_RANGE_WINDOW => "window".into(),
            style_range_type::STYLE_RANGE_SESSION => "session".into(),
            style_range_type::STYLE_RANGE_USER => format!("{}", _s((*sr).string.as_ptr())).into(),
        }
    }
}

/// C `vendor/tmux/format.c:1378`: `static void *format_cb_alternate_on(struct format_tree *ft)`
pub unsafe fn format_cb_alternate_on(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if !(*(*ft).wp).base.saved_grid.is_null() {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1390`: `static void *format_cb_alternate_saved_x(struct format_tree *ft)`
pub unsafe fn format_cb_alternate_saved_x(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            return format!("{}", (*(*ft).wp).base.saved_cx).into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1399`: `static void *format_cb_alternate_saved_y(struct format_tree *ft)`
pub unsafe fn format_cb_alternate_saved_y(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            return format!("{}", (*(*ft).wp).base.saved_cy).into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1420`: `static void *format_cb_buffer_name(struct format_tree *ft)`
pub unsafe fn format_cb_buffer_name(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if let Some(pb) = NonNull::new((*ft).pb) {
            return paste_buffer_name(pb).to_string().into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1429`: `static void *format_cb_buffer_sample(struct format_tree *ft)`
pub unsafe fn format_cb_buffer_sample(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).pb.is_null() {
            return paste_make_sample((*ft).pb).into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1453`: `static void *format_cb_buffer_size(struct format_tree *ft)`
pub unsafe fn format_cb_buffer_size(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).pb.is_null() {
            let mut size = 0usize;
            paste_buffer_data((*ft).pb, &mut size);
            return format!("{size}").into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1466`: `static void *format_cb_client_cell_height(struct format_tree *ft)`
pub unsafe fn format_cb_client_cell_height(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() && (*(*ft).c).tty.flags.intersects(tty_flags::TTY_STARTED) {
            return format!("{}", (*(*ft).c).tty.ypixel).into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1475`: `static void *format_cb_client_cell_width(struct format_tree *ft)`
pub unsafe fn format_cb_client_cell_width(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() && (*(*ft).c).tty.flags.intersects(tty_flags::TTY_STARTED) {
            return format!("{}", (*(*ft).c).tty.xpixel).into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1511`: `static void *format_cb_client_control_mode(struct format_tree *ft)`
pub unsafe fn format_cb_client_control_mode(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            if (*(*ft).c).flags.intersects(client_flag::CONTROL) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1523`: `static void *format_cb_client_discarded(struct format_tree *ft)`
pub unsafe fn format_cb_client_discarded(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            return format!("{}", (*(*ft).c).discarded).into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1532`: `static void *format_cb_client_flags(struct format_tree *ft)`
pub unsafe fn format_cb_client_flags(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            return format!("{}", _s(server_client_get_flags((*ft).c))).into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1541`: `static void *format_cb_client_height(struct format_tree *ft)`
pub unsafe fn format_cb_client_height(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() && (*(*ft).c).tty.flags.intersects(tty_flags::TTY_STARTED) {
            return format!("{}", (*(*ft).c).tty.sy).into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1550`: `static void *format_cb_client_key_table(struct format_tree *ft)`
pub unsafe fn format_cb_client_key_table(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            return format!("{}", _s((*(*(*ft).c).keytable).name_ptr())).into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1559`: `static void *format_cb_client_last_session(struct format_tree *ft)`
pub unsafe fn format_cb_client_last_session(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null()
            && !(*(*ft).c).last_session.is_null()
            && session_alive((*(*ft).c).last_session)
        {
            return format!("{}", (*(*(*ft).c).last_session).name).into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1570`: `static void *format_cb_client_name(struct format_tree *ft)`
pub unsafe fn format_cb_client_name(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            return format!("{}", _s((*(*ft).c).name)).into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1579`: `static void *format_cb_client_pid(struct format_tree *ft)`
pub unsafe fn format_cb_client_pid(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            return format!("{}", (*(*ft).c).pid as c_long).into();
        }
        format_table_type::None
    }
}

/// Callback for `client_prefix`.
/// C `vendor/tmux/format.c:1588`: `static void *format_cb_client_prefix(struct format_tree *ft)`
pub unsafe fn format_cb_client_prefix(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            let name = server_client_get_key_table((*ft).c);
            if strcmp((*(*(*ft).c).keytable).name_ptr(), name) == 0 {
                return "0".into();
            }
            return "1".into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1603`: `static void *format_cb_client_readonly(struct format_tree *ft)`
pub unsafe fn format_cb_client_readonly(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            if (*(*ft).c).flags.intersects(client_flag::READONLY) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1615`: `static void *format_cb_client_session(struct format_tree *ft)`
pub unsafe fn format_cb_client_session(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() && !(*(*ft).c).session.is_null() {
            return format!("{}", (*(*(*ft).c).session).name).into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1624`: `static void *format_cb_client_termfeatures(struct format_tree *ft)`
pub unsafe fn format_cb_client_termfeatures(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            return format!("{}", _s(tty_get_features((*(*ft).c).term_features))).into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1633`: `static void *format_cb_client_termname(struct format_tree *ft)`
pub unsafe fn format_cb_client_termname(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            return format!("{}", _s((*(*ft).c).term_name)).into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1642`: `static void *format_cb_client_termtype(struct format_tree *ft)`
pub unsafe fn format_cb_client_termtype(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            if (*(*ft).c).term_type.is_null() {
                return "".into();
            }
            return format!("{}", _s((*(*ft).c).term_type)).into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1654`: `static void *format_cb_client_tty(struct format_tree *ft)`
pub unsafe fn format_cb_client_tty(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            return format!("{}", _s((*(*ft).c).ttyname)).into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1663`: `static void *format_cb_client_uid(struct format_tree *ft)`
pub unsafe fn format_cb_client_uid(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            let uid = proc_get_peer_uid((*(*ft).c).peer);
            if uid != -1_i32 as uid_t {
                return format!("{}", uid as c_long).into();
            }
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1677`: `static void *format_cb_client_user(struct format_tree *ft)`
pub unsafe fn format_cb_client_user(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            let uid = proc_get_peer_uid((*(*ft).c).peer);
            if uid != -1_i32 as uid_t
                && let Some(pw) = NonNull::new(libc::getpwuid(uid))
            {
                return format!("{}", _s((*pw.as_ptr()).pw_name)).into();
            }
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1696`: `static void *format_cb_client_utf8(struct format_tree *ft)`
pub unsafe fn format_cb_client_utf8(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            if (*(*ft).c).flags.intersects(client_flag::UTF8) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1708`: `static void *format_cb_client_width(struct format_tree *ft)`
pub unsafe fn format_cb_client_width(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            return format!("{}", (*(*ft).c).tty.sx).into();
        }
        format_table_type::None
    }
}

/// C `vendor/tmux/format.c:1717`: `static void *format_cb_client_written(struct format_tree *ft)`
pub unsafe fn format_cb_client_written(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            return format!("{}", (*(*ft).c).written).into();
        }
        format_table_type::None
    }
}

/// Callback for `config_files`.
/// C `vendor/tmux/format.c:1743`: `static void *format_cb_config_files(__unused struct format_tree *ft)`
pub unsafe fn format_cb_config_files(_ft: *mut format_tree) -> format_table_type {
    let mut s = String::new();

    for file in CFG_FILES.lock().unwrap().iter() {
        s.push_str(file.to_str().expect("cfg_files invalid utf8"));
        s.push(',');
    }

    s.into()
}

/// Callback for `pane_flags`.
/// C `vendor/tmux/format.c`: `static void *format_cb_pane_flags(struct format_tree *ft)`
pub unsafe fn format_cb_pane_flags(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            return window_pane_printable_flags((*ft).wp).into();
        }
        format_table_type::None
    }
}

/// Callback for `cursor_blinking`.
/// C `vendor/tmux/format.c`: `static void *format_cb_cursor_blinking(struct format_tree *ft)`
pub unsafe fn format_cb_cursor_blinking(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() && !(*(*ft).wp).screen.is_null() {
            if (*(*(*ft).wp).screen)
                .mode
                .intersects(mode_flag::MODE_CURSOR_BLINKING)
            {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `cursor_very_visible`.
/// C `vendor/tmux/format.c`: `static void *format_cb_cursor_very_visible(struct format_tree *ft)`
pub unsafe fn format_cb_cursor_very_visible(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() && !(*(*ft).wp).screen.is_null() {
            if (*(*(*ft).wp).screen)
                .mode
                .intersects(mode_flag::MODE_CURSOR_VERY_VISIBLE)
            {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `cursor_colour`.
/// C `vendor/tmux/format.c`: `static void *format_cb_cursor_colour(struct format_tree *ft)`
pub unsafe fn format_cb_cursor_colour(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let wp = (*ft).wp;
        if wp.is_null() || (*wp).screen.is_null() {
            return format_table_type::None;
        }
        let s = (*wp).screen;
        let c = if (*s).ccolour != -1 {
            (*s).ccolour
        } else {
            (*s).default_ccolour
        };
        colour_tostring(c).into_owned().into()
    }
}

/// Callback for `cursor_shape`.
/// C `vendor/tmux/format.c`: `static void *format_cb_cursor_shape(struct format_tree *ft)`
pub unsafe fn format_cb_cursor_shape(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() && !(*(*ft).wp).screen.is_null() {
            return match (*(*(*ft).wp).screen).cstyle {
                screen_cursor_style::SCREEN_CURSOR_BLOCK => "block".into(),
                screen_cursor_style::SCREEN_CURSOR_UNDERLINE => "underline".into(),
                screen_cursor_style::SCREEN_CURSOR_BAR => "bar".into(),
                _ => "default".into(),
            };
        }
        format_table_type::None
    }
}

/// Callback for `cursor_flag`.
/// C `vendor/tmux/format.c:1763`: `static void *format_cb_cursor_flag(struct format_tree *ft)`
pub unsafe fn format_cb_cursor_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp).base.mode.intersects(mode_flag::MODE_CURSOR) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `cursor_x`.
/// C `vendor/tmux/format.c:1806`: `static void *format_cb_cursor_x(struct format_tree *ft)`
pub unsafe fn format_cb_cursor_x(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            return format!("{}", (*(*ft).wp).base.cx).into();
        }
        format_table_type::None
    }
}

/// Callback for `cursor_y`.
/// C `vendor/tmux/format.c:1815`: `static void *format_cb_cursor_y(struct format_tree *ft)`
pub unsafe fn format_cb_cursor_y(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            return format!("{}", (*(*ft).wp).base.cy).into();
        }
        format_table_type::None
    }
}

/// Callback for `history_limit`.
/// C `vendor/tmux/format.c:1836`: `static void *format_cb_history_limit(struct format_tree *ft)`
pub unsafe fn format_cb_history_limit(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            return format!("{}", (*(*(*ft).wp).base.grid).hlimit).into();
        }
        format_table_type::None
    }
}

/// Callback for `history_size`.
/// C `vendor/tmux/format.c:1845`: `static void *format_cb_history_size(struct format_tree *ft)`
pub unsafe fn format_cb_history_size(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            return format!("{}", (*(*(*ft).wp).base.grid).hsize).into();
        }
        format_table_type::None
    }
}

/// Callback for `insert_flag`.
/// C `vendor/tmux/format.c:1854`: `static void *format_cb_insert_flag(struct format_tree *ft)`
pub unsafe fn format_cb_insert_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp).base.mode.intersects(mode_flag::MODE_INSERT) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `keypad_cursor_flag`.
/// C `vendor/tmux/format.c:1866`: `static void *format_cb_keypad_cursor_flag(struct format_tree *ft)`
pub unsafe fn format_cb_keypad_cursor_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp).base.mode.intersects(mode_flag::MODE_KCURSOR) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `keypad_flag`.
/// C `vendor/tmux/format.c:1878`: `static void *format_cb_keypad_flag(struct format_tree *ft)`
pub unsafe fn format_cb_keypad_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp).base.mode.intersects(mode_flag::MODE_KKEYPAD) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `mouse_all_flag`.
/// C `vendor/tmux/format.c:1890`: `static void *format_cb_mouse_all_flag(struct format_tree *ft)`
pub unsafe fn format_cb_mouse_all_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp).base.mode.intersects(mode_flag::MODE_MOUSE_ALL) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `mouse_any_flag`.
/// C `vendor/tmux/format.c:1902`: `static void *format_cb_mouse_any_flag(struct format_tree *ft)`
pub unsafe fn format_cb_mouse_any_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp).base.mode.intersects(ALL_MOUSE_MODES) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `mouse_button_flag`.
/// C `vendor/tmux/format.c:1914`: `static void *format_cb_mouse_button_flag(struct format_tree *ft)`
pub unsafe fn format_cb_mouse_button_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp)
                .base
                .mode
                .intersects(mode_flag::MODE_MOUSE_BUTTON)
            {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `mouse_pane`.
/// C `vendor/tmux/format.c:1926`: `static void *format_cb_mouse_pane(struct format_tree *ft)`
pub unsafe fn format_cb_mouse_pane(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if (*ft).m.valid {
            if let Some(wp) = cmd_mouse_pane(&raw mut (*ft).m, null_mut(), null_mut()) {
                return format!("%{}", (*wp.as_ptr()).id).into();
            }
            return format_table_type::None;
        }
        format_table_type::None
    }
}

/// Callback for `mouse_sgr_flag`.
/// C `vendor/tmux/format.c:1941`: `static void *format_cb_mouse_sgr_flag(struct format_tree *ft)`
pub unsafe fn format_cb_mouse_sgr_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp).base.mode.intersects(mode_flag::MODE_MOUSE_SGR) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `mouse_standard_flag`.
/// C `vendor/tmux/format.c:1953`: `static void *format_cb_mouse_standard_flag(struct format_tree *ft)`
pub unsafe fn format_cb_mouse_standard_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp)
                .base
                .mode
                .intersects(mode_flag::MODE_MOUSE_STANDARD)
            {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `mouse_utf8_flag`.
/// C `vendor/tmux/format.c:1965`: `static void *format_cb_mouse_utf8_flag(struct format_tree *ft)`
pub unsafe fn format_cb_mouse_utf8_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp).base.mode.intersects(mode_flag::MODE_MOUSE_UTF8) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `mouse_x`.
/// C `vendor/tmux/format.c:1977`: `static void *format_cb_mouse_x(struct format_tree *ft)`
pub unsafe fn format_cb_mouse_x(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).m.valid {
            return format_table_type::None;
        }
        let wp = cmd_mouse_pane(&raw mut (*ft).m, null_mut(), null_mut());
        let mut x: u32 = 0;
        let mut y: u32 = 0;
        if let Some(wp) = wp
            && cmd_mouse_at(wp.as_ptr(), &raw mut (*ft).m, &mut x, &mut y, 0) == 0
        {
            return format!("{x}").into();
        }
        if !(*ft).c.is_null() && (*(*ft).c).tty.flags.intersects(tty_flags::TTY_STARTED) {
            if (*ft).m.statusat == 0 && (*ft).m.y < (*ft).m.statuslines {
                return format!("{}", (*ft).m.x).into();
            }
            if (*ft).m.statusat > 0 && (*ft).m.y >= (*ft).m.statusat as u32 {
                return format!("{}", (*ft).m.x).into();
            }
        }
        format_table_type::None
    }
}

/// Callback for `mouse_y`.
/// C `vendor/tmux/format.c:1998`: `static void *format_cb_mouse_y(struct format_tree *ft)`
pub unsafe fn format_cb_mouse_y(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).m.valid {
            return format_table_type::None;
        }
        let wp = cmd_mouse_pane(&raw mut (*ft).m, null_mut(), null_mut());
        let mut x: u32 = 0;
        let mut y: u32 = 0;
        if let Some(wp) = wp
            && cmd_mouse_at(wp.as_ptr(), &raw mut (*ft).m, &mut x, &mut y, 0) == 0
        {
            return format!("{y}").into();
        }
        if !(*ft).c.is_null() && (*(*ft).c).tty.flags.intersects(tty_flags::TTY_STARTED) {
            if (*ft).m.statusat == 0 && (*ft).m.y < (*ft).m.statuslines {
                return format!("{}", (*ft).m.y).into();
            }
            if (*ft).m.statusat > 0 && (*ft).m.y >= (*ft).m.statusat as u32 {
                return format!("{}", (*ft).m.y - (*ft).m.statusat as u32).into();
            }
        }
        format_table_type::None
    }
}

/// Callback for `next_session_id`.
/// C `vendor/tmux/format.c:2019`: `static void *format_cb_next_session_id(__unused struct format_tree *ft)`
pub unsafe fn format_cb_next_session_id(_ft: *mut format_tree) -> format_table_type {
    let value = NEXT_SESSION_ID.load(atomic::Ordering::Relaxed);
    format!("${value}").into()
}

/// Callback for `origin_flag`.
/// C `vendor/tmux/format.c:2026`: `static void *format_cb_origin_flag(struct format_tree *ft)`
pub unsafe fn format_cb_origin_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp).base.mode.intersects(mode_flag::MODE_ORIGIN) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_active`.
/// C `vendor/tmux/format.c:2050`: `static void *format_cb_pane_active(struct format_tree *ft)`
pub unsafe fn format_cb_pane_active(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*ft).wp == (*(*(*ft).wp).window).active {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_at_left`.
/// C `vendor/tmux/format.c:2062`: `static void *format_cb_pane_at_left(struct format_tree *ft)`
pub unsafe fn format_cb_pane_at_left(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp).xoff == 0 {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_at_right`.
/// C `vendor/tmux/format.c:2074`: `static void *format_cb_pane_at_right(struct format_tree *ft)`
pub unsafe fn format_cb_pane_at_right(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp).xoff + (*(*ft).wp).sx == (*(*(*ft).wp).window).sx {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_bottom`.
/// C `vendor/tmux/format.c:2086`: `static void *format_cb_pane_bottom(struct format_tree *ft)`
pub unsafe fn format_cb_pane_bottom(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            return format!("{}", (*(*ft).wp).yoff + (*(*ft).wp).sy - 1).into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_dead`.
/// C `vendor/tmux/format.c:2097`: `static void *format_cb_pane_dead(struct format_tree *ft)`
pub unsafe fn format_cb_pane_dead(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp).fd == -1 {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_dead_signal`.
/// C `vendor/tmux/format.c:2111`: `static void *format_cb_pane_dead_signal(struct format_tree *ft)`
pub unsafe fn format_cb_pane_dead_signal(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let wp = (*ft).wp;
        if !wp.is_null() {
            if (*wp).flags.intersects(window_pane_flags::PANE_STATUSREADY)
                && WIFSIGNALED((*wp).status)
            {
                return format!("{}", WTERMSIG((*wp).status)).into();
            }
            return format_table_type::None;
        }
        format_table_type::None
    }
}

/// Callback for `pane_dead_status`.
/// C `vendor/tmux/format.c:2128`: `static void *format_cb_pane_dead_status(struct format_tree *ft)`
pub unsafe fn format_cb_pane_dead_status(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let wp = (*ft).wp;
        if !wp.is_null() {
            if (*wp).flags.intersects(window_pane_flags::PANE_STATUSREADY)
                && WIFEXITED((*wp).status)
            {
                return format!("{}", WEXITSTATUS((*wp).status)).into();
            }
            return format_table_type::None;
        }
        format_table_type::None
    }
}

/// Callback for `pane_dead_time`.
/// C `vendor/tmux/format.c:2142`: `static void *format_cb_pane_dead_time(struct format_tree *ft)`
pub unsafe fn format_cb_pane_dead_time(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let wp = (*ft).wp;
        if !wp.is_null() && (*wp).flags.intersects(window_pane_flags::PANE_STATUSDRAWN) {
            return format_table_type::Time((*wp).dead_time);
        }
        format_table_type::None
    }
}

/// Callback for `pane_format`.
/// C `vendor/tmux/format.c:2156`: `static void *format_cb_pane_format(struct format_tree *ft)`
pub unsafe fn format_cb_pane_format(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if (*ft).type_ == format_type::FORMAT_TYPE_PANE {
            return "1".into();
        }
        "0".into()
    }
}

/// Callback for `pane_height`.
/// C `vendor/tmux/format.c:2165`: `static void *format_cb_pane_height(struct format_tree *ft)`
pub unsafe fn format_cb_pane_height(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            return format!("{}", (*(*ft).wp).sy).into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_id`.
/// C `vendor/tmux/format.c:2174`: `static void *format_cb_pane_id(struct format_tree *ft)`
pub unsafe fn format_cb_pane_id(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            return format!("%{}", (*(*ft).wp).id).into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_index`.
/// C `vendor/tmux/format.c:2183`: `static void *format_cb_pane_index(struct format_tree *ft)`
pub unsafe fn format_cb_pane_index(ft: *mut format_tree) -> format_table_type {
    unsafe {
        let mut idx: u32 = 0;
        if !(*ft).wp.is_null() && window_pane_index((*ft).wp, &mut idx) == 0 {
            return format!("{idx}").into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_input_off`.
/// C `vendor/tmux/format.c:2194`: `static void *format_cb_pane_input_off(struct format_tree *ft)`
pub unsafe fn format_cb_pane_input_off(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp)
                .flags
                .intersects(window_pane_flags::PANE_INPUTOFF)
            {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_unseen_changes`.
/// C `vendor/tmux/format.c:2206`: `static void *format_cb_pane_unseen_changes(struct format_tree *ft)`
pub unsafe fn format_cb_pane_unseen_changes(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp)
                .flags
                .intersects(window_pane_flags::PANE_UNSEENCHANGES)
            {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_key_mode`.
/// C `vendor/tmux/format.c:2218`: `static void *format_cb_pane_key_mode(struct format_tree *ft)`
pub unsafe fn format_cb_pane_key_mode(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() && !(*(*ft).wp).screen.is_null() {
            match (*(*(*ft).wp).screen).mode & EXTENDED_KEY_MODES {
                mode_flag::MODE_KEYS_EXTENDED => return "Ext 1".into(),
                mode_flag::MODE_KEYS_EXTENDED_2 => {
                    return "Ext 2".into();
                }
                _ => return "VT10x".into(),
            }
        }
        format_table_type::None
    }
}

/// Callback for `pane_last`.
/// C `vendor/tmux/format.c:2235`: `static void *format_cb_pane_last(struct format_tree *ft)`
pub unsafe fn format_cb_pane_last(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*ft).wp == tailq_first(&raw mut (*(*(*ft).wp).window).last_panes) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_left`.
/// C `vendor/tmux/format.c:2247`: `static void *format_cb_pane_left(struct format_tree *ft)`
pub unsafe fn format_cb_pane_left(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            return format!("{}", (*(*ft).wp).xoff).into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_marked`.
/// C `vendor/tmux/format.c:2256`: `static void *format_cb_pane_marked(struct format_tree *ft)`
pub unsafe fn format_cb_pane_marked(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if server_check_marked() && MARKED_PANE.wp == (*ft).wp {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_marked_set`.
/// C `vendor/tmux/format.c:2268`: `static void *format_cb_pane_marked_set(struct format_tree *ft)`
pub unsafe fn format_cb_pane_marked_set(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if server_check_marked() {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_mode`.
/// C `vendor/tmux/format.c:2280`: `static void *format_cb_pane_mode(struct format_tree *ft)`
pub unsafe fn format_cb_pane_mode(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            let wme = tailq_first(&raw mut (*(*ft).wp).modes);
            if !wme.is_null() {
                return (*(*wme).mode).name.into();
            }
            return format_table_type::None;
        }
        format_table_type::None
    }
}

/// Callback for `pane_path`.
/// C `vendor/tmux/format.c:2295`: `static void *format_cb_pane_path(struct format_tree *ft)`
pub unsafe fn format_cb_pane_path(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp).base.path.is_null() {
                return "".into();
            }
            return format!("{}", _s((*(*ft).wp).base.path)).into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_pid`.
/// C `vendor/tmux/format.c:2307`: `static void *format_cb_pane_pid(struct format_tree *ft)`
pub unsafe fn format_cb_pane_pid(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            return format!("{}", (*(*ft).wp).pid as i64).into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_pipe`.
/// C `vendor/tmux/format.c:2316`: `static void *format_cb_pane_pipe(struct format_tree *ft)`
pub unsafe fn format_cb_pane_pipe(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp).pipe_fd != -1 {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_right`.
/// C `vendor/tmux/format.c:2371`: `static void *format_cb_pane_right(struct format_tree *ft)`
pub unsafe fn format_cb_pane_right(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            return format!("{}", (*(*ft).wp).xoff + (*(*ft).wp).sx - 1).into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_search_string`.
/// C `vendor/tmux/format.c:2382`: `static void *format_cb_pane_search_string(struct format_tree *ft)`
pub unsafe fn format_cb_pane_search_string(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp).searchstr.is_none() {
                return "".into();
            }
            return format!("{}", _s((*(*ft).wp).searchstr_ptr())).into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_synchronized`.
/// C `vendor/tmux/format.c:2394`: `static void *format_cb_pane_synchronized(struct format_tree *ft)`
pub unsafe fn format_cb_pane_synchronized(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if options_get_number___::<i64>(&*(*(*ft).wp).options, "synchronize-panes") != 0 {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_title`.
/// C `vendor/tmux/format.c:2406`: `static void *format_cb_pane_title(struct format_tree *ft)`
pub unsafe fn format_cb_pane_title(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            return format!("{}", _s((*(*ft).wp).base.title)).into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_top`.
/// C `vendor/tmux/format.c:2415`: `static void *format_cb_pane_top(struct format_tree *ft)`
pub unsafe fn format_cb_pane_top(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            return format!("{}", (*(*ft).wp).yoff).into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_tty`.
/// C `vendor/tmux/format.c:2424`: `static void *format_cb_pane_tty(struct format_tree *ft)`
pub unsafe fn format_cb_pane_tty(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            return format!("{}", _s((*(*ft).wp).tty.as_ptr())).into();
        }
        format_table_type::None
    }
}

/// Callback for `pane_width`.
/// C `vendor/tmux/format.c:2433`: `static void *format_cb_pane_width(struct format_tree *ft)`
pub unsafe fn format_cb_pane_width(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            return format!("{}", (*(*ft).wp).sx).into();
        }
        format_table_type::None
    }
}

/// Callback for `scroll_region_lower`.
/// C `vendor/tmux/format.c:2485`: `static void *format_cb_scroll_region_lower(struct format_tree *ft)`
pub unsafe fn format_cb_scroll_region_lower(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            return format!("{}", (*(*ft).wp).base.rlower).into();
        }
        format_table_type::None
    }
}

/// Callback for `scroll_region_upper`.
/// C `vendor/tmux/format.c:2494`: `static void *format_cb_scroll_region_upper(struct format_tree *ft)`
pub unsafe fn format_cb_scroll_region_upper(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            return format!("{}", (*(*ft).wp).base.rupper).into();
        }
        format_table_type::None
    }
}

/// Callback for `server_sessions`.
/// C `vendor/tmux/format.c:2503`: `static void *format_cb_server_sessions(__unused struct format_tree *ft)`
pub unsafe fn format_cb_server_sessions(_ft: *mut format_tree) -> format_table_type {
    unsafe {
        let n: u32 = rb_foreach(&raw mut SESSIONS).count() as u32;
        format!("{n}").into()
    }
}

/// Callback for `session_attached`.
/// C `vendor/tmux/format.c:2575`: `static void *format_cb_session_attached(struct format_tree *ft)`
pub unsafe fn format_cb_session_attached(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).s.is_null() {
            return format!("{}", (*(*ft).s).attached).into();
        }
        format_table_type::None
    }
}

/// Callback for `session_format`.
/// C `vendor/tmux/format.c:2584`: `static void *format_cb_session_format(struct format_tree *ft)`
pub unsafe fn format_cb_session_format(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if (*ft).type_ == format_type::FORMAT_TYPE_SESSION {
            return "1".into();
        }
        "0".into()
    }
}

/// Callback for `session_group`.
/// C `vendor/tmux/format.c:2593`: `static void *format_cb_session_group(struct format_tree *ft)`
pub unsafe fn format_cb_session_group(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).s.is_null() {
            let sg = session_group_contains((*ft).s);
            if !sg.is_null() {
                return format!("{}", (*sg).name).into();
            }
        }
        format_table_type::None
    }
}

/// Callback for `session_group_attached`.
/// C `vendor/tmux/format.c:2604`: `static void *format_cb_session_group_attached(struct format_tree *ft)`
pub unsafe fn format_cb_session_group_attached(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).s.is_null() {
            let sg = session_group_contains((*ft).s);
            if !sg.is_null() {
                return format!("{}", session_group_attached_count(sg)).into();
            }
        }
        format_table_type::None
    }
}

/// Callback for `session_group_many_attached`.
/// C `vendor/tmux/format.c:2615`: `static void *format_cb_session_group_many_attached(struct format_tree *ft)`
pub unsafe fn format_cb_session_group_many_attached(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).s.is_null() {
            let sg = session_group_contains((*ft).s);
            if !sg.is_null() {
                if session_group_attached_count(sg) > 1 {
                    return "1".into();
                }
                return "0".into();
            }
        }
        format_table_type::None
    }
}

/// Callback for `session_group_size`.
/// C `vendor/tmux/format.c:2629`: `static void *format_cb_session_group_size(struct format_tree *ft)`
pub unsafe fn format_cb_session_group_size(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).s.is_null() {
            let sg = session_group_contains((*ft).s);
            if !sg.is_null() {
                return format!("{}", session_group_count(sg)).into();
            }
        }
        format_table_type::None
    }
}

/// Callback for `session_grouped`.
/// C `vendor/tmux/format.c:2640`: `static void *format_cb_session_grouped(struct format_tree *ft)`
pub unsafe fn format_cb_session_grouped(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).s.is_null() {
            if !session_group_contains((*ft).s).is_null() {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `session_id`.
/// C `vendor/tmux/format.c:2652`: `static void *format_cb_session_id(struct format_tree *ft)`
pub unsafe fn format_cb_session_id(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).s.is_null() {
            return format!("${}", (*(*ft).s).id).into();
        }
        format_table_type::None
    }
}

/// Callback for `session_many_attached`.
/// C `vendor/tmux/format.c:2661`: `static void *format_cb_session_many_attached(struct format_tree *ft)`
pub unsafe fn format_cb_session_many_attached(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).s.is_null() {
            if (*(*ft).s).attached > 1 {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `session_marked`.
/// C `vendor/tmux/format.c:2673`: `static void *format_cb_session_marked(struct format_tree *ft)`
pub unsafe fn format_cb_session_marked(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).s.is_null() {
            if server_check_marked() && MARKED_PANE.s == (*ft).s {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `session_name`.
/// C `vendor/tmux/format.c:2685`: `static void *format_cb_session_name(struct format_tree *ft)`
pub unsafe fn format_cb_session_name(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).s.is_null() {
            return format!("{}", (*(*ft).s).name).into();
        }
        format_table_type::None
    }
}

/// Callback for `session_path`.
/// C `vendor/tmux/format.c:2694`: `static void *format_cb_session_path(struct format_tree *ft)`
pub unsafe fn format_cb_session_path(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).s.is_null() {
            return format!("{}", _s((*(*ft).s).cwd_ptr())).into();
        }
        format_table_type::None
    }
}

/// Callback for `session_windows`.
/// C `vendor/tmux/format.c:2703`: `static void *format_cb_session_windows(struct format_tree *ft)`
pub unsafe fn format_cb_session_windows(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).s.is_null() {
            return format!("{}", winlink_count(&raw mut (*(*ft).s).windows)).into();
        }
        format_table_type::None
    }
}

/// Callback for `socket_path`.
/// C `vendor/tmux/format.c:2712`: `static void *format_cb_socket_path(__unused struct format_tree *ft)`
pub unsafe fn format_cb_socket_path(_ft: *mut format_tree) -> format_table_type {
    unsafe { format!("{}", _s(SOCKET_PATH)).into() }
}

/// Callback for version.
/// C `vendor/tmux/format.c:2719`: `static void *format_cb_version(__unused struct format_tree *ft)`
pub fn format_cb_version(_ft: *mut format_tree) -> format_table_type {
    getversion().into()
}

/// Callback for `active_window_index`.
/// C `vendor/tmux/format.c:2737`: `static void *format_cb_active_window_index(struct format_tree *ft)`
pub unsafe fn format_cb_active_window_index(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).s.is_null() {
            return format!("{}", (*(*(*ft).s).curw).idx).into();
        }
        format_table_type::None
    }
}

/// Callback for `last_window_index`.
/// C `vendor/tmux/format.c:2746`: `static void *format_cb_last_window_index(struct format_tree *ft)`
pub unsafe fn format_cb_last_window_index(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).s.is_null() {
            let wl = rb_max(&raw mut (*(*ft).s).windows);
            return format!("{}", (*wl).idx).into();
        }
        format_table_type::None
    }
}

/// Callback for `window_active`.
/// C `vendor/tmux/format.c:2759`: `static void *format_cb_window_active(struct format_tree *ft)`
pub unsafe fn format_cb_window_active(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wl.is_null() {
            if (*ft).wl == (*(*(*ft).wl).session).curw {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `window_activity_flag`.
/// C `vendor/tmux/format.c:2771`: `static void *format_cb_window_activity_flag(struct format_tree *ft)`
pub unsafe fn format_cb_window_activity_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wl.is_null() {
            if (*(*ft).wl)
                .flags
                .intersects(winlink_flags::WINLINK_ACTIVITY)
            {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `window_bell_flag`.
/// C `vendor/tmux/format.c:2783`: `static void *format_cb_window_bell_flag(struct format_tree *ft)`
pub unsafe fn format_cb_window_bell_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wl.is_null() {
            if (*(*ft).wl).flags.intersects(winlink_flags::WINLINK_BELL) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `window_bigger`.
/// C `vendor/tmux/format.c:2795`: `static void *format_cb_window_bigger(struct format_tree *ft)`
pub unsafe fn format_cb_window_bigger(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            let mut ox: u32 = 0;
            let mut oy: u32 = 0;
            let mut sx: u32 = 0;
            let mut sy: u32 = 0;
            if tty_window_offset(&raw mut (*(*ft).c).tty, &mut ox, &mut oy, &mut sx, &mut sy) != 0 {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `window_cell_height`.
/// C `vendor/tmux/format.c:2809`: `static void *format_cb_window_cell_height(struct format_tree *ft)`
pub unsafe fn format_cb_window_cell_height(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).w.is_null() {
            return format!("{}", (*(*ft).w).ypixel).into();
        }
        format_table_type::None
    }
}

/// Callback for `window_cell_width`.
/// C `vendor/tmux/format.c:2818`: `static void *format_cb_window_cell_width(struct format_tree *ft)`
pub unsafe fn format_cb_window_cell_width(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).w.is_null() {
            return format!("{}", (*(*ft).w).xpixel).into();
        }
        format_table_type::None
    }
}

/// Callback for `window_end_flag`.
/// C `vendor/tmux/format.c:2827`: `static void *format_cb_window_end_flag(struct format_tree *ft)`
pub unsafe fn format_cb_window_end_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wl.is_null() {
            if (*ft).wl == rb_max(&raw mut (*(*(*ft).wl).session).windows) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `window_flags`.
/// C `vendor/tmux/format.c:2839`: `static void *format_cb_window_flags(struct format_tree *ft)`
pub unsafe fn format_cb_window_flags(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wl.is_null() {
            return format!("{}", _s(window_printable_flags((*ft).wl, 1))).into();
        }
        format_table_type::None
    }
}

/// Callback for `window_format`.
/// C `vendor/tmux/format.c:2848`: `static void *format_cb_window_format(struct format_tree *ft)`
pub unsafe fn format_cb_window_format(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if (*ft).type_ == format_type::FORMAT_TYPE_WINDOW {
            return "1".into();
        }
        "0".into()
    }
}

/// Callback for `window_height`.
/// C `vendor/tmux/format.c:2857`: `static void *format_cb_window_height(struct format_tree *ft)`
pub unsafe fn format_cb_window_height(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).w.is_null() {
            return format!("{}", (*(*ft).w).sy).into();
        }
        format_table_type::None
    }
}

/// Callback for `window_id`.
/// C `vendor/tmux/format.c:2866`: `static void *format_cb_window_id(struct format_tree *ft)`
pub unsafe fn format_cb_window_id(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).w.is_null() {
            return format!("@{}", (*(*ft).w).id).into();
        }
        format_table_type::None
    }
}

/// Callback for `window_index`.
/// C `vendor/tmux/format.c:2875`: `static void *format_cb_window_index(struct format_tree *ft)`
pub unsafe fn format_cb_window_index(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wl.is_null() {
            return format!("{}", (*(*ft).wl).idx).into();
        }
        format_table_type::None
    }
}

/// Callback for `window_last_flag`.
/// C `vendor/tmux/format.c:2884`: `static void *format_cb_window_last_flag(struct format_tree *ft)`
pub unsafe fn format_cb_window_last_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wl.is_null() {
            if (*ft).wl == tailq_first(&raw mut (*(*(*ft).wl).session).lastw) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `window_linked`.
/// C `vendor/tmux/format.c:2896`: `static void *format_cb_window_linked(struct format_tree *ft)`
pub unsafe fn format_cb_window_linked(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wl.is_null() {
            if session_is_linked((*(*ft).wl).session, (*(*ft).wl).window) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `window_linked_sessions`.
/// C `vendor/tmux/format.c:2919`: `static void *format_cb_window_linked_sessions(struct format_tree *ft)`
pub unsafe fn format_cb_window_linked_sessions(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wl.is_null() {
            return format!("{}", (*(*(*ft).wl).window).references).into();
        }
        format_table_type::None
    }
}

/// Callback for `window_marked_flag`.
/// C `vendor/tmux/format.c:2946`: `static void *format_cb_window_marked_flag(struct format_tree *ft)`
pub unsafe fn format_cb_window_marked_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wl.is_null() {
            if server_check_marked() && MARKED_PANE.wl == (*ft).wl {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `window_name`.
/// C `vendor/tmux/format.c:2958`: `static void *format_cb_window_name(struct format_tree *ft)`
pub unsafe fn format_cb_window_name(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).w.is_null() {
            return format!("{}", _s((*(*ft).w).name)).into();
        }
        format_table_type::None
    }
}

/// Callback for `window_offset_x`.
/// C `vendor/tmux/format.c:2967`: `static void *format_cb_window_offset_x(struct format_tree *ft)`
pub unsafe fn format_cb_window_offset_x(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            let mut ox: u32 = 0;
            let mut oy: u32 = 0;
            let mut sx: u32 = 0;
            let mut sy: u32 = 0;
            if tty_window_offset(&raw mut (*(*ft).c).tty, &mut ox, &mut oy, &mut sx, &mut sy) != 0 {
                return format!("{ox}").into();
            }
        }
        format_table_type::None
    }
}

/// Callback for `window_offset_y`.
/// C `vendor/tmux/format.c:2981`: `static void *format_cb_window_offset_y(struct format_tree *ft)`
pub unsafe fn format_cb_window_offset_y(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            let mut ox: u32 = 0;
            let mut oy: u32 = 0;
            let mut sx: u32 = 0;
            let mut sy: u32 = 0;
            if tty_window_offset(&raw mut (*(*ft).c).tty, &mut ox, &mut oy, &mut sx, &mut sy) != 0 {
                return format!("{oy}").into();
            }
        }
        format_table_type::None
    }
}

/// Callback for `window_panes`.
/// C `vendor/tmux/format.c:2995`: `static void *format_cb_window_panes(struct format_tree *ft)`
pub unsafe fn format_cb_window_panes(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).w.is_null() {
            return format!("{}", window_count_panes((*ft).w)).into();
        }
        format_table_type::None
    }
}

/// Callback for `window_raw_flags`.
/// C `vendor/tmux/format.c:3004`: `static void *format_cb_window_raw_flags(struct format_tree *ft)`
pub unsafe fn format_cb_window_raw_flags(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wl.is_null() {
            return format!("{}", _s(window_printable_flags((*ft).wl, 0))).into();
        }
        format_table_type::None
    }
}

/// Callback for `window_silence_flag`.
/// C `vendor/tmux/format.c:3013`: `static void *format_cb_window_silence_flag(struct format_tree *ft)`
pub unsafe fn format_cb_window_silence_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wl.is_null() {
            if (*(*ft).wl).flags.intersects(winlink_flags::WINLINK_SILENCE) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `window_start_flag`.
/// C `vendor/tmux/format.c:3025`: `static void *format_cb_window_start_flag(struct format_tree *ft)`
pub unsafe fn format_cb_window_start_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wl.is_null() {
            if (*ft).wl == rb_min(&raw mut (*(*(*ft).wl).session).windows) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `window_width`.
/// C `vendor/tmux/format.c:3037`: `static void *format_cb_window_width(struct format_tree *ft)`
pub unsafe fn format_cb_window_width(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).w.is_null() {
            return format!("{}", (*(*ft).w).sx).into();
        }
        format_table_type::None
    }
}

/// Callback for `window_zoomed_flag`.
/// C `vendor/tmux/format.c:3046`: `static void *format_cb_window_zoomed_flag(struct format_tree *ft)`
pub unsafe fn format_cb_window_zoomed_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).w.is_null() {
            if (*(*ft).w).flags.intersects(window_flag::ZOOMED) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `wrap_flag`.
/// C `vendor/tmux/format.c:3058`: `static void *format_cb_wrap_flag(struct format_tree *ft)`
pub unsafe fn format_cb_wrap_flag(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).wp.is_null() {
            if (*(*ft).wp).base.mode.intersects(mode_flag::MODE_WRAP) {
                return "1".into();
            }
            return "0".into();
        }
        format_table_type::None
    }
}

/// Callback for `buffer_created`.
/// C `vendor/tmux/format.c:3070`: `static void *format_cb_buffer_created(struct format_tree *ft)`
pub unsafe fn format_cb_buffer_created(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if let Some(pb) = NonNull::new((*ft).pb) {
            format_table_type::Time(timeval {
                tv_sec: paste_buffer_created(pb),
                tv_usec: 0,
            })
        } else {
            format_table_type::None
        }
    }
}

/// Callback for `client_activity`.
/// C `vendor/tmux/format.c:3084`: `static void *format_cb_client_activity(struct format_tree *ft)`
pub unsafe fn format_cb_client_activity(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            return format_table_type::Time((*(*ft).c).activity_time);
        }
        format_table_type::None
    }
}

/// Callback for `client_created`.
/// C `vendor/tmux/format.c:3093`: `static void *format_cb_client_created(struct format_tree *ft)`
pub unsafe fn format_cb_client_created(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).c.is_null() {
            return format_table_type::Time((*(*ft).c).creation_time);
        }
        format_table_type::None
    }
}

/// Callback for `session_activity`.
/// C `vendor/tmux/format.c:3102`: `static void *format_cb_session_activity(struct format_tree *ft)`
pub unsafe fn format_cb_session_activity(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).s.is_null() {
            return format_table_type::Time((*(*ft).s).activity_time);
        }
        format_table_type::None
    }
}

/// Callback for `session_created`.
/// C `vendor/tmux/format.c:3111`: `static void *format_cb_session_created(struct format_tree *ft)`
pub unsafe fn format_cb_session_created(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).s.is_null() {
            return format_table_type::Time((*(*ft).s).creation_time);
        }
        format_table_type::None
    }
}

/// Callback for `session_last_attached`.
/// C `vendor/tmux/format.c:3120`: `static void *format_cb_session_last_attached(struct format_tree *ft)`
pub unsafe fn format_cb_session_last_attached(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).s.is_null() {
            return format_table_type::Time((*(*ft).s).last_attached_time);
        }
        format_table_type::None
    }
}

/// Callback for `start_time`.
/// C `vendor/tmux/format.c:3129`: `static void *format_cb_start_time(__unused struct format_tree *ft)`
pub unsafe fn format_cb_start_time(_ft: *mut format_tree) -> format_table_type {
    format_table_type::Time(unsafe { START_TIME })
}

/// Callback for `window_activity`.
/// C `vendor/tmux/format.c:3136`: `static void *format_cb_window_activity(struct format_tree *ft)`
pub unsafe fn format_cb_window_activity(ft: *mut format_tree) -> format_table_type {
    unsafe {
        if !(*ft).w.is_null() {
            return format_table_type::Time((*(*ft).w).activity_time);
        }
        format_table_type::None
    }
}

/// Callback for `buffer_mode_format`.
/// C `vendor/tmux/format.c:3145`: `static void *format_cb_buffer_mode_format(__unused struct format_tree *ft)`
pub unsafe fn format_cb_buffer_mode_format(_ft: *mut format_tree) -> format_table_type {
    WINDOW_BUFFER_MODE.default_format.unwrap().into()
}

/// Callback for `client_mode_format`.
/// C `vendor/tmux/format.c:3152`: `static void *format_cb_client_mode_format(__unused struct format_tree *ft)`
pub unsafe fn format_cb_client_mode_format(_ft: *mut format_tree) -> format_table_type {
    WINDOW_CLIENT_MODE.default_format.unwrap().into()
}

/// Callback for `tree_mode_format`.
/// C `vendor/tmux/format.c:3159`: `static void *format_cb_tree_mode_format(__unused struct format_tree *ft)`
pub unsafe fn format_cb_tree_mode_format(_ft: *mut format_tree) -> format_table_type {
    WINDOW_TREE_MODE.default_format.unwrap().into()
}

/// Callback for uid.
/// C `vendor/tmux/format.c:3166`: `static void *format_cb_uid(__unused struct format_tree *ft)`
pub unsafe fn format_cb_uid(_ft: *mut format_tree) -> format_table_type {
    unsafe { format!("{}", getuid() as i64).into() }
}

/// Callback for user.
/// C `vendor/tmux/format.c:3173`: `static void *format_cb_user(__unused struct format_tree *ft)`
pub unsafe fn format_cb_user(_ft: *mut format_tree) -> format_table_type {
    unsafe {
        if let Some(pw) = NonNull::new(getpwuid(getuid())) {
            cstr_to_str((*pw.as_ptr()).pw_name.cast())
                .to_string()
                .into()
        } else {
            format_table_type::None
        }
    }
}

/// Format table type.
#[derive(Default)]
pub enum format_table_type {
    #[default]
    None,
    String(Cow<'static, str>),
    Time(timeval),
}

impl From<Cow<'static, str>> for format_table_type {
    fn from(value: Cow<'static, str>) -> Self {
        Self::String(value)
    }
}

impl From<String> for format_table_type {
    fn from(value: String) -> Self {
        format_table_type::String(Cow::Owned(value))
    }
}

impl From<&'static str> for format_table_type {
    fn from(value: &'static str) -> Self {
        format_table_type::String(Cow::Borrowed(value))
    }
}

/// Format table entry.
#[repr(C)]
pub struct format_table_entry {
    key: &'static str,
    cb: format_cb,
}

impl format_table_entry {
    pub const fn new(key: &'static str, cb: format_cb) -> Self {
        Self { key, cb }
    }
}

// Format table. Default format variables (that are almost always in the tree
// and where the value is expanded by a callback in this file) are listed
// here. Only variables which are added by the caller go into the tree.
static FORMAT_TABLE: &[format_table_entry] = &[
    format_table_entry::new("active_window_index", format_cb_active_window_index),
    format_table_entry::new("alternate_on", format_cb_alternate_on),
    format_table_entry::new("alternate_saved_x", format_cb_alternate_saved_x),
    format_table_entry::new("alternate_saved_y", format_cb_alternate_saved_y),
    format_table_entry::new("bracket_paste_flag", format_cb_bracket_paste_flag),
    format_table_entry::new("buffer_created", format_cb_buffer_created),
    format_table_entry::new("buffer_mode_format", format_cb_buffer_mode_format),
    format_table_entry::new("buffer_name", format_cb_buffer_name),
    format_table_entry::new("buffer_sample", format_cb_buffer_sample),
    format_table_entry::new("buffer_size", format_cb_buffer_size),
    format_table_entry::new("client_activity", format_cb_client_activity),
    format_table_entry::new("client_cell_height", format_cb_client_cell_height),
    format_table_entry::new("client_cell_width", format_cb_client_cell_width),
    format_table_entry::new("client_control_mode", format_cb_client_control_mode),
    format_table_entry::new("client_created", format_cb_client_created),
    format_table_entry::new("client_discarded", format_cb_client_discarded),
    format_table_entry::new("client_flags", format_cb_client_flags),
    format_table_entry::new("client_height", format_cb_client_height),
    format_table_entry::new("client_key_table", format_cb_client_key_table),
    format_table_entry::new("client_last_session", format_cb_client_last_session),
    format_table_entry::new("client_mode_format", format_cb_client_mode_format),
    format_table_entry::new("client_name", format_cb_client_name),
    format_table_entry::new("client_pid", format_cb_client_pid),
    format_table_entry::new("client_prefix", format_cb_client_prefix),
    format_table_entry::new("client_readonly", format_cb_client_readonly),
    format_table_entry::new("client_session", format_cb_client_session),
    format_table_entry::new("client_termfeatures", format_cb_client_termfeatures),
    format_table_entry::new("client_termname", format_cb_client_termname),
    format_table_entry::new("client_termtype", format_cb_client_termtype),
    format_table_entry::new("client_tty", format_cb_client_tty),
    format_table_entry::new("client_uid", format_cb_client_uid),
    format_table_entry::new("client_user", format_cb_client_user),
    format_table_entry::new("client_utf8", format_cb_client_utf8),
    format_table_entry::new("client_width", format_cb_client_width),
    format_table_entry::new("client_written", format_cb_client_written),
    format_table_entry::new("config_files", format_cb_config_files),
    format_table_entry::new("cursor_blinking", format_cb_cursor_blinking),
    format_table_entry::new("cursor_character", format_cb_cursor_character),
    format_table_entry::new("cursor_colour", format_cb_cursor_colour),
    format_table_entry::new("cursor_flag", format_cb_cursor_flag),
    format_table_entry::new("cursor_shape", format_cb_cursor_shape),
    format_table_entry::new("cursor_very_visible", format_cb_cursor_very_visible),
    format_table_entry::new("cursor_x", format_cb_cursor_x),
    format_table_entry::new("cursor_y", format_cb_cursor_y),
    format_table_entry::new("history_all_bytes", format_cb_history_all_bytes),
    format_table_entry::new("history_bytes", format_cb_history_bytes),
    format_table_entry::new("history_limit", format_cb_history_limit),
    format_table_entry::new("history_size", format_cb_history_size),
    format_table_entry::new("host", format_cb_host),
    format_table_entry::new("host_short", format_cb_host_short),
    format_table_entry::new("insert_flag", format_cb_insert_flag),
    format_table_entry::new("keypad_cursor_flag", format_cb_keypad_cursor_flag),
    format_table_entry::new("keypad_flag", format_cb_keypad_flag),
    format_table_entry::new("last_window_index", format_cb_last_window_index),
    format_table_entry::new("mouse_all_flag", format_cb_mouse_all_flag),
    format_table_entry::new("mouse_any_flag", format_cb_mouse_any_flag),
    format_table_entry::new("mouse_button_flag", format_cb_mouse_button_flag),
    format_table_entry::new("mouse_hyperlink", format_cb_mouse_hyperlink),
    format_table_entry::new("mouse_line", format_cb_mouse_line),
    format_table_entry::new("mouse_pane", format_cb_mouse_pane),
    format_table_entry::new("mouse_sgr_flag", format_cb_mouse_sgr_flag),
    format_table_entry::new("mouse_standard_flag", format_cb_mouse_standard_flag),
    format_table_entry::new("mouse_status_line", format_cb_mouse_status_line),
    format_table_entry::new("mouse_status_range", format_cb_mouse_status_range),
    format_table_entry::new("mouse_utf8_flag", format_cb_mouse_utf8_flag),
    format_table_entry::new("mouse_word", format_cb_mouse_word),
    format_table_entry::new("mouse_x", format_cb_mouse_x),
    format_table_entry::new("mouse_y", format_cb_mouse_y),
    format_table_entry::new("next_session_id", format_cb_next_session_id),
    format_table_entry::new("origin_flag", format_cb_origin_flag),
    format_table_entry::new("pane_active", format_cb_pane_active),
    format_table_entry::new("pane_at_bottom", format_cb_pane_at_bottom),
    format_table_entry::new("pane_at_left", format_cb_pane_at_left),
    format_table_entry::new("pane_at_right", format_cb_pane_at_right),
    format_table_entry::new("pane_at_top", format_cb_pane_at_top),
    format_table_entry::new("pane_bg", format_cb_pane_bg),
    format_table_entry::new("pane_bottom", format_cb_pane_bottom),
    format_table_entry::new("pane_current_command", format_cb_current_command),
    format_table_entry::new("pane_current_path", format_cb_current_path),
    format_table_entry::new("pane_dead", format_cb_pane_dead),
    format_table_entry::new("pane_dead_signal", format_cb_pane_dead_signal),
    format_table_entry::new("pane_dead_status", format_cb_pane_dead_status),
    format_table_entry::new("pane_dead_time", format_cb_pane_dead_time),
    format_table_entry::new("pane_fg", format_cb_pane_fg),
    format_table_entry::new("pane_flags", format_cb_pane_flags),
    format_table_entry::new("pane_format", format_cb_pane_format),
    format_table_entry::new("pane_height", format_cb_pane_height),
    format_table_entry::new("pane_id", format_cb_pane_id),
    format_table_entry::new("pane_in_mode", format_cb_pane_in_mode),
    format_table_entry::new("pane_index", format_cb_pane_index),
    format_table_entry::new("pane_input_off", format_cb_pane_input_off),
    format_table_entry::new("pane_key_mode", format_cb_pane_key_mode),
    format_table_entry::new("pane_last", format_cb_pane_last),
    format_table_entry::new("pane_left", format_cb_pane_left),
    format_table_entry::new("pane_marked", format_cb_pane_marked),
    format_table_entry::new("pane_marked_set", format_cb_pane_marked_set),
    format_table_entry::new("pane_mode", format_cb_pane_mode),
    format_table_entry::new("pane_path", format_cb_pane_path),
    format_table_entry::new("pane_pid", format_cb_pane_pid),
    format_table_entry::new("pane_pipe", format_cb_pane_pipe),
    format_table_entry::new("pane_right", format_cb_pane_right),
    format_table_entry::new("pane_search_string", format_cb_pane_search_string),
    format_table_entry::new("pane_start_command", format_cb_start_command),
    format_table_entry::new("pane_start_path", format_cb_start_path),
    format_table_entry::new("pane_synchronized", format_cb_pane_synchronized),
    format_table_entry::new("pane_tabs", format_cb_pane_tabs),
    format_table_entry::new("pane_title", format_cb_pane_title),
    format_table_entry::new("pane_top", format_cb_pane_top),
    format_table_entry::new("pane_tty", format_cb_pane_tty),
    format_table_entry::new("pane_unseen_changes", format_cb_pane_unseen_changes),
    format_table_entry::new("pane_width", format_cb_pane_width),
    format_table_entry::new("pane_zoomed_flag", format_cb_pane_zoomed_flag),
    format_table_entry::new("pid", format_cb_pid),
    format_table_entry::new("scroll_region_lower", format_cb_scroll_region_lower),
    format_table_entry::new("scroll_region_upper", format_cb_scroll_region_upper),
    format_table_entry::new("server_sessions", format_cb_server_sessions),
    format_table_entry::new("session_activity", format_cb_session_activity),
    format_table_entry::new("session_activity_flag", format_cb_session_activity_flag),
    format_table_entry::new("session_alert", format_cb_session_alert),
    format_table_entry::new("session_alerts", format_cb_session_alerts),
    format_table_entry::new("session_attached", format_cb_session_attached),
    format_table_entry::new("session_attached_list", format_cb_session_attached_list),
    format_table_entry::new("session_bell_flag", format_cb_session_bell_flag),
    format_table_entry::new("session_created", format_cb_session_created),
    format_table_entry::new("session_format", format_cb_session_format),
    format_table_entry::new("session_group", format_cb_session_group),
    format_table_entry::new("session_group_attached", format_cb_session_group_attached),
    format_table_entry::new(
        "session_group_attached_list",
        format_cb_session_group_attached_list,
    ),
    format_table_entry::new("session_group_list", format_cb_session_group_list),
    format_table_entry::new(
        "session_group_many_attached",
        format_cb_session_group_many_attached,
    ),
    format_table_entry::new("session_group_size", format_cb_session_group_size),
    format_table_entry::new("session_grouped", format_cb_session_grouped),
    format_table_entry::new("session_id", format_cb_session_id),
    format_table_entry::new("session_last_attached", format_cb_session_last_attached),
    format_table_entry::new("session_many_attached", format_cb_session_many_attached),
    format_table_entry::new("session_marked", format_cb_session_marked),
    format_table_entry::new("session_name", format_cb_session_name),
    format_table_entry::new("session_path", format_cb_session_path),
    format_table_entry::new("session_silence_flag", format_cb_session_silence_flag),
    format_table_entry::new("session_stack", format_cb_session_stack),
    format_table_entry::new("session_windows", format_cb_session_windows),
    format_table_entry::new("sixel_support", format_cb_sixel_support),
    format_table_entry::new("socket_path", format_cb_socket_path),
    format_table_entry::new("start_time", format_cb_start_time),
    format_table_entry::new(
        "synchronized_output_flag",
        format_cb_synchronized_output_flag,
    ),
    format_table_entry::new("tree_mode_format", format_cb_tree_mode_format),
    format_table_entry::new("uid", format_cb_uid),
    format_table_entry::new("user", format_cb_user),
    format_table_entry::new("version", format_cb_version),
    format_table_entry::new("window_active", format_cb_window_active),
    format_table_entry::new("window_active_clients", format_cb_window_active_clients),
    format_table_entry::new(
        "window_active_clients_list",
        format_cb_window_active_clients_list,
    ),
    format_table_entry::new("window_active_sessions", format_cb_window_active_sessions),
    format_table_entry::new(
        "window_active_sessions_list",
        format_cb_window_active_sessions_list,
    ),
    format_table_entry::new("window_activity", format_cb_window_activity),
    format_table_entry::new("window_activity_flag", format_cb_window_activity_flag),
    format_table_entry::new("window_bell_flag", format_cb_window_bell_flag),
    format_table_entry::new("window_bigger", format_cb_window_bigger),
    format_table_entry::new("window_cell_height", format_cb_window_cell_height),
    format_table_entry::new("window_cell_width", format_cb_window_cell_width),
    format_table_entry::new("window_end_flag", format_cb_window_end_flag),
    format_table_entry::new("window_flags", format_cb_window_flags),
    format_table_entry::new("window_format", format_cb_window_format),
    format_table_entry::new("window_height", format_cb_window_height),
    format_table_entry::new("window_id", format_cb_window_id),
    format_table_entry::new("window_index", format_cb_window_index),
    format_table_entry::new("window_last_flag", format_cb_window_last_flag),
    format_table_entry::new("window_layout", format_cb_window_layout),
    format_table_entry::new("window_linked", format_cb_window_linked),
    format_table_entry::new("window_linked_sessions", format_cb_window_linked_sessions),
    format_table_entry::new(
        "window_linked_sessions_list",
        format_cb_window_linked_sessions_list,
    ),
    format_table_entry::new("window_marked_flag", format_cb_window_marked_flag),
    format_table_entry::new("window_name", format_cb_window_name),
    format_table_entry::new("window_offset_x", format_cb_window_offset_x),
    format_table_entry::new("window_offset_y", format_cb_window_offset_y),
    format_table_entry::new("window_panes", format_cb_window_panes),
    format_table_entry::new("window_raw_flags", format_cb_window_raw_flags),
    format_table_entry::new("window_silence_flag", format_cb_window_silence_flag),
    format_table_entry::new("window_stack_index", format_cb_window_stack_index),
    format_table_entry::new("window_start_flag", format_cb_window_start_flag),
    format_table_entry::new("window_visible_layout", format_cb_window_visible_layout),
    format_table_entry::new("window_width", format_cb_window_width),
    format_table_entry::new("window_zoomed_flag", format_cb_window_zoomed_flag),
    format_table_entry::new("wrap_flag", format_cb_wrap_flag),
];

/// C `vendor/tmux/format.c:3793`: `static int format_table_compare(const void *key0, const void *entry0)`
pub unsafe fn format_table_compare(
    key: *const u8,
    entry: *const format_table_entry,
) -> std::cmp::Ordering {
    unsafe { strcmp_(key, (*entry).key) }
}

/// C `vendor/tmux/format.c:3803`: `static const struct format_table_entry *format_table_get(const char *key)`
pub unsafe fn format_table_get(key: *const u8) -> Option<&'static format_table_entry> {
    unsafe {
        match FORMAT_TABLE.binary_search_by(|e| format_table_compare(key, e).reverse()) {
            Ok(idx) => Some(&FORMAT_TABLE[idx]),
            Err(_) => None,
        }
    }
}

/// C `vendor/tmux/format.c:3811`: `void format_merge(struct format_tree *ft, struct format_tree *from)`
pub unsafe fn format_merge(ft: *mut format_tree, from: *mut format_tree) {
    unsafe {
        for fe in rb_foreach(&raw mut (*from).tree).map(NonNull::as_ptr) {
            if !(*fe).value.is_null() {
                format_add!(ft, cstr_to_str((*fe).key), "{}", _s((*fe).value));
            }
        }
    }
}

/// C `vendor/tmux/format.c:3823`: `struct window_pane *format_get_pane(struct format_tree *ft)`
pub unsafe fn format_get_pane(ft: *mut format_tree) -> *mut window_pane {
    unsafe { (*ft).wp }
}

/// C `vendor/tmux/format.c:3830`: `static void format_create_add_item(struct format_tree *ft, struct cmdq_item *item)`
pub unsafe fn format_create_add_item(ft: *mut format_tree, item: *mut cmdq_item) {
    unsafe {
        let event = cmdq_get_event(item);
        let m = &(*event).m;

        cmdq_merge_formats(item, ft);
        memcpy__(&raw mut (*ft).m, m);
    }
}

/// C `vendor/tmux/format.c:3841`: `struct format_tree *format_create(struct client *c, struct cmdq_item *item, int tag, int flags)`
pub unsafe fn format_create(
    c: *mut client,
    item: *mut cmdq_item,
    tag: i32,
    flags: format_flags,
) -> *mut format_tree {
    unsafe {
        let ft = xcalloc1::<format_tree>() as *mut format_tree;
        rb_init(&raw mut (*ft).tree);

        if !c.is_null() {
            (*ft).client = c;
            (*c).references += 1;
        }
        (*ft).item = item;
        (*ft).tag = tag as u32;
        (*ft).flags = flags;

        if !item.is_null() {
            format_create_add_item(ft, item);
        }

        ft
    }
}

/// C `vendor/tmux/format.c:3865`: `void format_free(struct format_tree *ft)`
pub unsafe fn format_free(ft: *mut format_tree) {
    unsafe {
        for fe in rb_foreach(&raw mut (*ft).tree).map(NonNull::as_ptr) {
            rb_remove(&raw mut (*ft).tree, fe);
            free_((*fe).value);
            free_((*fe).key);
            free_(fe);
        }

        if !(*ft).client.is_null() {
            server_client_unref((*ft).client);
        }
        free(ft as *mut c_void);
    }
}

/// C `vendor/tmux/format.c:3883`: `static void format_log_debug_cb(const char *key, const char *value, void *arg)`
pub unsafe fn format_log_debug_cb(key: &str, value: &str, prefix: *mut u8) {
    unsafe {
        log_debug!("{}: {}={}", _s(prefix), key, value);
    }
}

/// C `vendor/tmux/format.c:3892`: `void format_log_debug(struct format_tree *ft, const char *prefix)`
pub unsafe fn format_log_debug(ft: *mut format_tree, prefix: *const u8) {
    unsafe {
        format_each(ft, format_log_debug_cb, prefix.cast_mut());
    }
}

/// C `vendor/tmux/format.c:3899`: `void format_each(struct format_tree *ft, void (*cb)(const char *, const char *, void *), void *arg)`
pub unsafe fn format_each<T>(ft: *mut format_tree, cb: unsafe fn(&str, &str, *mut T), arg: *mut T) {
    unsafe {
        for fte in FORMAT_TABLE {
            let value = (fte.cb)(ft);
            match value {
                format_table_type::None => continue,
                format_table_type::Time(tv) => {
                    let s = format!("{}", tv.tv_sec);
                    cb(fte.key, &s, arg);
                }
                format_table_type::String(string) => {
                    cb(fte.key, &string, arg);
                }
            }
        }

        for fe in rb_foreach(&raw mut (*ft).tree).map(NonNull::as_ptr) {
            if (*fe).time != 0 {
                let s = format!("{}", (*fe).time);
                cb(cstr_to_str((*fe).key), &s, arg);
            } else {
                if let Some(fe_cb) = (*fe).cb
                    && (*fe).value.is_null()
                {
                    (*fe).value = match fe_cb(ft) {
                        format_table_type::None => CString::default().into_raw().cast(),
                        format_table_type::String(cow) => {
                            CString::new(cow.into_owned()).unwrap().into_raw().cast()
                        }
                        format_table_type::Time(_timeval) => unreachable!("unreachable?"),
                    }
                }
                cb(cstr_to_str((*fe).key), cstr_to_str((*fe).value), arg);
            }
        }
    }
}

macro_rules! format_add {
   ($state:expr, $key:expr, $fmt:literal $(, $args:expr)* $(,)?) => {
        crate::format::format_add_($state, $key, format_args!($fmt $(, $args)*))
    };
}
pub(crate) use format_add;

/// Add a key-value pair.
pub unsafe fn format_add_(ft: *mut format_tree, key: &str, args: std::fmt::Arguments) {
    unsafe {
        let fe = Box::leak(Box::new(format_entry {
            key: xstrdup__(key),
            value: null_mut(),
            time: 0,
            cb: None,
            entry: zeroed(),
        })) as *mut format_entry;

        let fe = match rb_insert(&raw mut (*ft).tree, fe) {
            fe_now if !fe_now.is_null() => {
                free_((*fe).key);
                free_(fe);
                free_((*fe_now).value);
                fe_now
            }
            _ => fe,
        };

        let mut value = args.to_string();
        value.push('\0');
        (*fe).value = value.leak().as_mut_ptr().cast();
    }
}

/// Add a key and time.
/// C `vendor/tmux/format.c:3968`: `void format_add_tv(struct format_tree *ft, const char *key, struct timeval *tv)`
pub unsafe fn format_add_tv(ft: *mut format_tree, key: *const u8, tv: *const timeval) {
    unsafe {
        let fe = Box::leak(Box::new(format_entry {
            key: xstrdup(key).as_ptr(),
            value: null_mut(),
            time: (*tv).tv_sec,
            cb: None,
            entry: zeroed(),
        })) as *mut format_entry;

        let fe_now = rb_insert(&raw mut (*ft).tree, fe);
        if !fe_now.is_null() {
            free_((*fe).key);
            free_(fe);
            free_((*fe_now).value);
        }
    }
}

/// Add a key and function.
/// C `vendor/tmux/format.c:3991`: `void format_add_cb(struct format_tree *ft, const char *key, format_cb cb)`
pub unsafe fn format_add_cb(ft: *mut format_tree, key: *const u8, cb: format_cb) {
    unsafe {
        let fe = Box::leak(Box::new(format_entry {
            key: xstrdup(key).as_ptr(),
            value: null_mut(),
            time: 0,
            cb: Some(cb),
            entry: zeroed(),
        })) as *mut format_entry;

        let fe_now = rb_insert(&raw mut (*ft).tree, fe);
        if !fe_now.is_null() {
            free_((*fe).key);
            free_(fe);
            free_((*fe_now).value);
        }
    }
}

/// Quote shell special characters in string.
/// C `vendor/tmux/format.c:4015`: `static char *format_quote_shell(const char *s)`
pub unsafe fn format_quote_shell(s: *const u8) -> *mut u8 {
    unsafe {
        let out: *mut u8 = xmalloc(strlen(s) * 2 + 1).as_ptr().cast();
        let mut at = out;
        let mut cp = s;
        while *cp != b'\0' {
            if !strchr(c!("|&;<>()$`\\\"'*?[# =%"), *cp as i32).is_null() {
                *at = b'\\';
                at = at.add(1);
            }
            *at = *cp;
            at = at.add(1);
            cp = cp.add(1);
        }
        *at = b'\0';
        out
    }
}

/// Quote #s in string.
/// C `vendor/tmux/format.c:4032`: `static char *format_quote_style(const char *s)`
pub unsafe fn format_quote_style(s: *const u8) -> *mut u8 {
    unsafe {
        let out: *mut u8 = xmalloc(strlen(s) * 2 + 1).as_ptr().cast();
        let mut at = out;

        let mut cp = s;
        while *cp != b'\0' {
            if *cp == b'#' {
                *at = b'#';
                at = at.add(1);
            }
            *at = *cp;
            at = at.add(1);
            cp = cp.add(1);
        }
        *at = b'\0';
        out
    }
}

/// Make a prettier time.
/// C `vendor/tmux/format.c:4049`: `char *format_pretty_time(time_t t, int seconds)`
pub unsafe fn format_pretty_time(t: time_t, seconds: i32) -> *mut u8 {
    unsafe {
        let mut now: time_t = libc::time(null_mut());
        if now < t {
            now = t;
        }
        let age = now - t;

        let mut now_tm = MaybeUninit::<tm>::uninit();
        let now_tm = now_tm.as_mut_ptr();
        let mut tm = MaybeUninit::<tm>::uninit();
        let tm = tm.as_mut_ptr();

        localtime_r(&raw const now, now_tm);
        localtime_r(&raw const t, tm);

        // Last 24 hours.
        const SIZEOF_S: usize = 9;
        let mut s = [0u8; 9];
        if age < 24 * 3600 {
            if seconds != 0 {
                strftime(s.as_mut_ptr(), SIZEOF_S, c!("%H:%M:%S"), tm);
            } else {
                strftime(s.as_mut_ptr(), SIZEOF_S, c!("%H:%M"), tm);
            }
            return xstrdup(s.as_ptr()).as_ptr();
        }

        // This month or last 28 days.
        if ((*tm).tm_year == (*now_tm).tm_year && (*tm).tm_mon == (*now_tm).tm_mon)
            || age < 28 * 24 * 3600
        {
            strftime(s.as_mut_ptr(), SIZEOF_S, c!("%a%d"), tm);
            return xstrdup(s.as_ptr()).as_ptr();
        }

        // Last 12 months.
        if ((*tm).tm_year == (*now_tm).tm_year && (*tm).tm_mon < (*now_tm).tm_mon)
            || ((*tm).tm_year == (*now_tm).tm_year - 1 && (*tm).tm_mon > (*now_tm).tm_mon)
        {
            strftime(s.as_mut_ptr(), SIZEOF_S, c!("%d%b"), tm);
            return xstrdup(s.as_ptr()).as_ptr();
        }

        // Older than that.
        strftime(s.as_mut_ptr(), SIZEOF_S, c!("%h%y"), tm);
        xstrdup(s.as_ptr()).as_ptr()
    }
}

/// Find a format entry.
/// C `vendor/tmux/format.c:4133`: `static char *format_find(struct format_tree *ft, const char *key, int modifiers, const char *time_format)`
fn format_find(
    ft: *mut format_tree,
    key: *const u8,
    modifiers: format_modifiers,
    time_format: *const u8,
) -> *mut u8 {
    unsafe {
        let mut s = MaybeUninit::<[u8; 512]>::uninit();
        let s = s.as_mut_ptr() as *mut u8;
        let mut fe_find = MaybeUninit::<format_entry>::uninit();

        const SIZEOF_S: usize = 512;
        let mut t: time_t = 0;
        let mut idx = 0;
        let mut found = null_mut();

        'found: {
            // Option names are always valid UTF-8, so an invalid-UTF-8 key (e.g.
            // from a malformed format string) can match no option. Use the
            // non-panicking conversion and skip the option lookups on invalid
            // UTF-8 rather than aborting in cstr_to_str — C passes the raw char*
            // to options_parse_get, which likewise finds no match.
            if let Some(key_str) = cstr_to_str_(key) {
                let mut o = options_parse_get(GLOBAL_OPTIONS, key_str, &raw mut idx, 0);
                if o.is_null() && !(*ft).wp.is_null() {
                    o = options_parse_get((*(*ft).wp).options, key_str, &raw mut idx, 0);
                }
                if o.is_null() && !(*ft).w.is_null() {
                    o = options_parse_get((*(*ft).w).options, key_str, &raw mut idx, 0);
                }
                if o.is_null() {
                    o = options_parse_get(GLOBAL_W_OPTIONS, key_str, &raw mut idx, 0);
                }
                if o.is_null() && !(*ft).s.is_null() {
                    o = options_parse_get((*(*ft).s).options, key_str, &raw mut idx, 0);
                }
                if o.is_null() {
                    o = options_parse_get(GLOBAL_S_OPTIONS, key_str, &raw mut idx, 0);
                }
                if !o.is_null() {
                    found = options_to_string(o, idx, 1);
                    break 'found;
                }
            }

            if let Some(fte) = format_table_get(key) {
                match (fte.cb)(ft) {
                    format_table_type::Time(tv) => t = tv.tv_sec,
                    format_table_type::String(string) => {
                        found = CString::new(string.into_owned()).unwrap().into_raw().cast();
                    }
                    format_table_type::None => found = null_mut(),
                }
                break 'found;
            }

            (*fe_find.as_mut_ptr()).key = key.cast_mut(); // TODO: check if this is correct casting away const
            let fe = rb_find(&raw mut (*ft).tree, fe_find.as_mut_ptr());
            if !fe.is_null() {
                if (*fe).time != 0 {
                    t = (*fe).time;
                    break 'found;
                }
                if let Some(cb) = (*fe).cb
                    && (*fe).value.is_null()
                {
                    (*fe).value = match cb(ft) {
                        format_table_type::None => CString::default().into_raw().cast(),
                        format_table_type::String(cow) => {
                            CString::new(cow.into_owned()).unwrap().into_raw().cast()
                        }
                        format_table_type::Time(_timeval) => unreachable!("unreachable?"),
                    };
                }
                found = xstrdup((*fe).value).as_ptr();
                break 'found;
            }

            if !modifiers.intersects(format_modifiers::FORMAT_TIMESTRING) {
                let mut envent = null_mut();
                if !(*ft).s.is_null() {
                    envent = environ_find((*(*ft).s).environ, key);
                }
                if envent.is_null() {
                    envent = environ_find(GLOBAL_ENVIRON, key);
                }
                if !envent.is_null() && (*envent).value.is_some() {
                    found = xstrdup((*envent).value_ptr()).as_ptr();
                    break 'found;
                }
            }

            return null_mut();
        }
        // found
        if modifiers.intersects(format_modifiers::FORMAT_TIMESTRING) {
            if t == 0 && !found.is_null() {
                t = strtonum(found, 0, i64::MAX).unwrap_or_default();
                free_(found);
            }
            if t == 0 {
                return null_mut();
            }
            if modifiers.intersects(format_modifiers::FORMAT_PRETTY) {
                found = format_pretty_time(t, 0);
            } else {
                if !time_format.is_null() {
                    let mut tm = MaybeUninit::<tm>::uninit();
                    let tm = tm.as_mut_ptr();

                    localtime_r(&raw const t, tm);
                    strftime(s, SIZEOF_S, time_format, tm);
                } else {
                    ctime_r(&raw const t, s.cast());
                    *s.add(strcspn(s, c!("\n"))) = b'\0';
                }
                found = xstrdup(s).as_ptr();
            }
            return found;
        }

        if t != 0 {
            found = format_nul!("{t}");
        } else if found.is_null() {
            return null_mut();
        }
        let mut saved: *mut u8;
        if modifiers.intersects(format_modifiers::FORMAT_BASENAME) {
            saved = found;
            found = xstrdup__(basename(cstr_to_str(saved)));
            free_(saved);
        }
        if modifiers.intersects(format_modifiers::FORMAT_DIRNAME) {
            saved = found;
            found = xstrdup(libc::dirname(saved.cast()).cast()).as_ptr();
            free_(saved);
        }
        if modifiers.intersects(format_modifiers::FORMAT_QUOTE_SHELL) {
            saved = found;
            found = format_quote_shell(saved);
            free_(saved);
        }
        if modifiers.intersects(format_modifiers::FORMAT_QUOTE_STYLE) {
            saved = found;
            found = format_quote_style(saved);
            free_(saved);
        }
        found
    }
}

/// C `vendor/tmux/format.c:4263`: `static int format_check_time(struct format_expand_state *es, u_int *check)`
///
/// Returns 0 once the running expansion has exceeded `FORMAT_TIME_LIMIT`
/// milliseconds; callers then abort and yield an empty string. `check` throttles
/// the clock reads — when non-null, only every `FORMAT_TIME_LOOP_CHECK`-th call
/// actually consults the timer, so the guard is cheap inside tight loops.
unsafe fn format_check_time(es: *mut format_expand_state, check: *mut u32) -> i32 {
    unsafe {
        if !check.is_null() {
            *check += 1;
            if !(*check).is_multiple_of(FORMAT_TIME_LOOP_CHECK) {
                return 1;
            }
        }

        let mut t = get_timer();
        if t - (*es).start_time < FORMAT_TIME_LIMIT {
            return 1;
        }
        t -= (*es).start_time;

        format_log1!(es, c!("format_check_time"), "reached time limit ({})", t);
        0
    }
}

/// Unescape escaped characters.
// vendor/tmux/format.c:4281  format_unescape()
//
// Faithful port of C's `for (; s != end; s++) { … continue; … }`: the `s++`
// runs on every iteration, INCLUDING when the loop `continue`s. A `while` loop
// must therefore advance `s` explicitly on every path — the previous port
// dropped it, so the normal branch never advanced (infinite loop writing past
// the `xmalloc(strlen + 1)` buffer → crash). See format_strip below, which
// ported the same loop correctly.
/// C `vendor/tmux/format.c:4281`: `static char *format_unescape(struct format_expand_state *es, const char *s, size_t n)`
pub unsafe fn format_unescape(es: *mut format_expand_state, mut s: *const u8) -> *mut u8 {
    unsafe {
        let mut cp = xmalloc(strlen(s) + 1).as_ptr().cast();
        let out = cp;
        let mut brackets = 0;
        let mut check: u32 = 0;
        while *s != b'\0' {
            if format_check_time(es, &raw mut check) == 0 {
                free_(out);
                return xstrdup(c!("")).as_ptr();
            }
            if *s == b'#' && *s.add(1) == b'{' {
                brackets += 1;
            }
            // `s + 1 != end` in C guards the trailing-`#` case so the `*++s`
            // below can't step past the terminator.
            if brackets == 0
                && *s == b'#'
                && *s.add(1) != b'\0'
                && !strchr(c!(",#{}:"), *s.add(1) as i32).is_null()
            {
                s = s.add(1); // C: `*cp++ = *++s;`
                *cp = *s;
                cp = cp.add(1);
                s = s.add(1); // C for-loop's own `s++` that runs on `continue`
                continue;
            }
            if *s == b'}' {
                brackets -= 1;
            }
            *cp = *s;
            cp = cp.add(1);
            s = s.add(1); // C for-loop's `s++`
        }
        *cp = b'\0';
        out
    }
}

/// Remove escaped characters.
/// C `vendor/tmux/format.c:4313`: `static char *format_strip(struct format_expand_state *es, const char *s)`
pub unsafe fn format_strip(es: *mut format_expand_state, mut s: *const u8) -> *mut u8 {
    unsafe {
        let out = xmalloc(strlen(s) + 1).as_ptr().cast();
        let mut cp = out;
        let mut brackets = 0;
        let mut check: u32 = 0;

        while *s != b'\0' {
            if format_check_time(es, &raw mut check) == 0 {
                free_(out);
                return xstrdup(c!("")).as_ptr();
            }
            if *s == b'#' && *s.add(1) == b'{' {
                brackets += 1;
            }
            if *s == b'#' && !strchr(c!(",#{}:"), *s.add(1) as i32).is_null() {
                if brackets != 0 {
                    *cp = *s;
                    cp = cp.add(1);
                }
                s = s.add(1);
                continue;
            }
            if *s == b'}' {
                brackets -= 1;
            }
            *cp = *s;
            cp = cp.add(1);
            s = s.add(1);
        }
        *cp = b'\0';
        out
    }
}

/// Skip until end.
/// C `vendor/tmux/format.c:4342`: `static const char *format_skip1(struct format_expand_state *es, const char *s, const char *end)`
unsafe fn format_skip1(es: *mut format_expand_state, mut s: *const u8, end: *const u8) -> *const u8 {
    unsafe {
        let mut brackets = 0;
        let mut check: u32 = 0;

        while *s != b'\0' {
            if !es.is_null() && format_check_time(es, &raw mut check) == 0 {
                return null_mut();
            }
            if *s == b'#' && *s.add(1) == b'{' {
                brackets += 1;
            }
            if *s == b'#' && *s.add(1) != b'\0' && !strchr(c!(",#{}:"), *s.add(1) as i32).is_null()
            {
                s = s.add(2);
                continue;
            }
            if *s == b'}' {
                brackets -= 1;
            }
            if !strchr(end, *s as i32).is_null() && brackets == 0 {
                break;
            }
            s = s.add(1);
        }
        if *s == b'\0' {
            return null_mut();
        }
        s
    }
}

/// Skip until end.
/// C `vendor/tmux/format.c:4370`: `const char *format_skip(const char *s, const char *end)`
pub unsafe fn format_skip(s: *const u8, end: *const u8) -> *const u8 {
    unsafe { format_skip1(null_mut(), s, end) }
}

/// Return left and right alternatives separated by commas.
/// C `vendor/tmux/format.c:4377`: `static int format_choose(struct format_expand_state *es, const char *s, char **left, char **right, int expand)`
pub unsafe fn format_choose(
    es: *mut format_expand_state,
    s: *const u8,
    left: *mut *mut u8,
    right: *mut *mut u8,
    expand: c_int,
) -> c_int {
    unsafe {
        let cp: *const u8 = format_skip1(es, s, c!(","));
        if cp.is_null() {
            return -1;
        }
        let left0 = xstrndup(s, cp.offset_from(s) as usize).as_ptr();
        let right0 = xstrdup(cp.add(1)).as_ptr();

        if expand != 0 {
            *left = format_expand1(es, left0);
            free_(left0);
            *right = format_expand1(es, right0);
            free_(right0);
        } else {
            *left = left0;
            *right = right0;
        }
        0
    }
}

/// Is this true?
/// C `vendor/tmux/format.c:4403`: `int format_true(const char *s)`
pub unsafe fn format_true(s: *const u8) -> bool {
    unsafe { !s.is_null() && *s != b'\0' && (*s != b'0' || *s.add(1) != b'\0') }
}

/// Check if modifier end.
/// C `vendor/tmux/format.c:4412`: `static int format_is_end(char c)`
pub fn format_is_end(c: u8) -> bool {
    c == b';' || c == b':'
}

/// Add to modifier list.
/// C `vendor/tmux/format.c:4419`: `static void format_add_modifier(struct format_modifier **list, u_int *count, const char *c, size_t n, char **argv, int argc)`
pub unsafe fn format_add_modifier(
    list: *mut *mut format_modifier,
    count: *mut u32,
    c: *const u8,
    n: usize,
    argv: *mut *mut u8,
    argc: i32,
) {
    unsafe {
        *list = xreallocarray_(*list, (*count) as usize + 1).as_ptr();
        let fm = (*list).add(*count as usize);
        (*count) += 1;

        memcpy((*fm).modifier.as_mut_ptr().cast(), c.cast(), n);
        (*fm).modifier[n] = b'\0';
        (*fm).size = n as u32;

        (*fm).argv = argv;
        (*fm).argc = argc;
    }
}

/// Free modifier list.
/// C `vendor/tmux/format.c:4437`: `static void format_free_modifiers(struct format_modifier *list, u_int count)`
pub unsafe fn format_free_modifiers(list: *mut format_modifier, count: u32) {
    unsafe {
        for i in 0..count as usize {
            cmd_free_argv((*list.add(i)).argc, (*list.add(i)).argv);
        }
        free_(list);
    }
}

/// Build modifier list.
/// C `vendor/tmux/format.c:4448`: `static struct format_modifier *format_build_modifiers(struct format_expand_state *es, const char **s, u_int *count)`
pub unsafe fn format_build_modifiers(
    es: *mut format_expand_state,
    s: *mut *const u8,
    count: *mut u32,
) -> *mut format_modifier {
    unsafe {
        let mut cp = *s;
        let mut end: *const u8;
        let mut list: *mut format_modifier = null_mut();

        let mut last: [u8; 4] = [b'X', b';', b':', b'\0'];
        let last: *mut u8 = last.as_mut_ptr();

        // char c, last[] = "X;:", **argv, *value;
        // int argc;

        // Modifiers are a ; separated list of the forms:
        //      l,m,C,a,b,c,d,n,t,w,q,E,T,S,W,P,<,>
        // 	=a
        // 	=/a
        //      =/a/
        // 	s/a/b/
        // 	s/a/b
        // 	||,&&,!=,==,<=,>=

        *count = 0;

        while *cp != b'\0' && *cp != b':' {
            // Skip any separator character.
            if *cp == b';' {
                cp = cp.add(1);
            }

            // Check single character modifiers with no arguments.
            if !strchr(c!("labcdnwETSWPL!<>"), *cp as i32).is_null() && format_is_end(*cp.add(1)) {
                format_add_modifier(&raw mut list, count, cp, 1, null_mut(), 0);
                cp = cp.add(1);
                continue;
            }

            // Then try double character with no arguments.
            if (memcmp(c!("||").cast(), cp.cast(), 2) == 0
                || memcmp(c!("&&").cast(), cp.cast(), 2) == 0
                || memcmp(c!("!!").cast(), cp.cast(), 2) == 0
                || memcmp(c!("!=").cast(), cp.cast(), 2) == 0
                || memcmp(c!("==").cast(), cp.cast(), 2) == 0
                || memcmp(c!("<=").cast(), cp.cast(), 2) == 0
                || memcmp(c!(">=").cast(), cp.cast(), 2) == 0)
                && format_is_end(*cp.add(2))
            {
                format_add_modifier(&raw mut list, count, cp, 2, null_mut(), 0);
                cp = cp.add(2);
                continue;
            }

            // Now try single character with arguments. C's set is
            // "ImCLNPSst=pReqWc"; the loop modifiers S/W/P/L take a sort
            // argument (e.g. `#{P/r:...}`), so they must be here as well as in
            // the no-argument set above (which handles the bare `#{S:...}`).
            // `c` takes an f/b argument for the colour-to-escape form
            // (`#{c/f:red}`), so it is here in addition to the no-arg set. `R`
            // (repeat) takes a `left,count` argument (`#{R:x,3}`).
            if strchr(c!("mCNSWPLst=peqcR"), *cp as i32).is_null() {
                break;
            }
            let mut c = *cp;

            // No arguments provided.
            if format_is_end(*cp.add(1)) {
                format_add_modifier(&raw mut list, count, cp, 1, null_mut(), 0);
                cp = cp.add(1);
                continue;
            }
            let mut argv: *mut *mut u8 = null_mut();
            let mut argc = 0;

            // Single argument with no wrapper character.
            if ispunct(*cp.add(1) as i32) == 0 || *cp.add(1) == b'-' {
                end = format_skip1(es, cp.add(1), c!(":;"));
                if end.is_null() {
                    break;
                }

                argv = xcalloc1();
                let value = xstrndup(cp.add(1), end.offset_from(cp.add(1)) as usize).as_ptr();
                *argv = format_expand1(es, value);
                free_(value);
                argc = 1;

                format_add_modifier(&raw mut list, count, &raw mut c, 1, argv, argc);
                cp = end;
                continue;
            }

            // Multiple arguments with a wrapper character.
            *last = *cp.add(1);
            cp = cp.add(1);
            loop {
                if *cp == *last && format_is_end(*cp.add(1)) {
                    cp = cp.add(1);
                    break;
                }
                end = format_skip1(es, cp.add(1), last);
                if end.is_null() {
                    break;
                }
                cp = cp.add(1);

                argv = xreallocarray_(argv, argc as usize + 1).as_ptr();
                let value = xstrndup(cp, end.offset_from(cp) as usize).as_ptr();
                *argv.add(argc as usize) = format_expand1(es, value);
                argc += 1;
                free_(value);

                cp = end;
                if format_is_end(*cp) {
                    break;
                }
            }
            format_add_modifier(&raw mut list, count, &raw mut c, 1, argv, argc);
        }
        if *cp != b':' {
            format_free_modifiers(list, *count);
            *count = 0;
            return null_mut();
        }
        *s = cp.add(1);
        list
    }
}

/// C `vendor/tmux/format.c:4603`: `static char *format_match(struct format_modifier *fm, const char *pattern, const char *text)`
pub unsafe fn format_match(
    fm: *mut format_modifier,
    pattern: *const u8,
    text: *const u8,
) -> *mut u8 {
    unsafe {
        let mut s = c!("");
        let mut r = MaybeUninit::<regex_t>::uninit();
        let r = r.as_mut_ptr();
        let mut flags: i32 = 0;

        if (*fm).argc >= 1 {
            s = *(*fm).argv;
        }
        if strchr(s, b'r' as i32).is_null() {
            if !strchr(s, b'i' as i32).is_null() {
                flags |= FNM_CASEFOLD;
            }
            if libc::fnmatch(pattern, text, flags) != 0 {
                return xstrdup(c!("0")).as_ptr();
            }
        } else {
            flags = REG_EXTENDED | REG_NOSUB;
            if !strchr(s, b'i' as i32).is_null() {
                flags |= REG_ICASE;
            }
            if regcomp(r, pattern, flags) != 0 {
                return xstrdup(c!("0")).as_ptr();
            }
            if regexec(r, text, 0, null_mut(), 0) != 0 {
                regfree(r);
                return xstrdup(c!("0")).as_ptr();
            }
            regfree(r);
        }
        xstrdup(c!("1")).as_ptr()
    }
}

/// C `vendor/tmux/format.c:4637`: `static char *format_sub(struct format_modifier *fm, const char *text, const char *pattern, const char *with)`
pub unsafe fn format_sub(
    fm: *mut format_modifier,
    text: *const u8,
    pattern: *const u8,
    with: *const u8,
) -> *mut u8 {
    unsafe {
        let mut flags: i32 = REG_EXTENDED;

        if (*fm).argc >= 3 && !strchr(*(*fm).argv.add(2), b'i' as i32).is_null() {
            flags |= REG_ICASE;
        }
        let value = regsub(pattern, with, text, flags);
        if value.is_null() {
            xstrdup(text).as_ptr()
        } else {
            value
        }
    }
}

/// C `vendor/tmux/format.c:4653`: `static char *format_search(struct format_modifier *fm, struct window_pane *wp, const char *s)`
pub unsafe fn format_search(
    fm: *mut format_modifier,
    wp: *mut window_pane,
    s: *const u8,
) -> *mut u8 {
    unsafe {
        let mut ignore = 0;
        let mut regex = 0;

        if (*fm).argc >= 1 {
            if !strchr(*(*fm).argv, b'i' as i32).is_null() {
                ignore = 1;
            }
            if !strchr(*(*fm).argv, b'r' as i32).is_null() {
                regex = 1;
            }
        }
        format_nul!("{}", window_pane_search(wp, s, regex, ignore))
    }
}

/// C `vendor/tmux/format.c:4724`: `static char *format_session_name(struct format_expand_state *es, const char *fmt)`
pub unsafe fn format_session_name(es: *mut format_expand_state, fmt: *const u8) -> *mut u8 {
    unsafe {
        let name = format_expand1(es, fmt);

        for s in rb_foreach(&raw mut SESSIONS).map(NonNull::as_ptr) {
            if streq_(name, &(*s).name) {
                free_(name);
                return xstrdup(c!("1")).as_ptr();
            }
        }

        free_(name);
        xstrdup(c!("0")).as_ptr()
    }
}

/// C `vendor/tmux/format.c:4742`: `static char *format_loop_sessions(struct format_expand_state *es, const char *fmt)`
pub unsafe fn format_loop_sessions(
    es: *mut format_expand_state,
    fmt: *const u8,
    sc: sort_criteria,
) -> *mut u8 {
    unsafe {
        let ft = (*es).ft;
        let c = (*ft).client;
        let item = (*ft).item;
        let mut value: *mut u8 = xcalloc(1, 1).as_ptr().cast();
        let mut valuelen = 1;

        // C: split fmt into the "all,active" variants; the active client's own
        // session uses the second form. Without this, `#{S:all,active}` was
        // expanded literally instead of choosing "all".
        let mut all: *mut u8 = null_mut();
        let mut active: *mut u8 = null_mut();
        if format_choose(es, fmt, &raw mut all, &raw mut active, 0) != 0 {
            all = xstrdup(fmt).as_ptr();
            active = null_mut();
        }

        // C: sort_get_sessions() collects the sessions RB tree then sorts by
        // the criteria (default SORT_INDEX for `#{S:}`).
        let sessions = sort_get_sessions(sc);
        let n = sessions.len();
        for (i, &s) in sessions.iter().enumerate() {
            format_log1!(es, c!("format_loop_sessions"), "session loop: ${}", (*s).id,);
            let use_ = if !active.is_null()
                && !(*ft).c.is_null()
                && !(*(*ft).c).session.is_null()
                && (*s).id == (*(*(*ft).c).session).id
            {
                active
            } else {
                all
            };
            let nft = format_create(c, item, FORMAT_NONE, (*ft).flags);
            format_add!(nft, "loop_index", "{}", i);
            format_add!(nft, "loop_last_flag", "{}", i32::from(i == n - 1));
            format_defaults(nft, (*ft).c, NonNull::new(s), None, None);
            let mut next = zeroed();
            format_copy_state(&mut next, es, format_expand_flags::empty());
            next.ft = nft;
            let expanded = format_expand1(&mut next, use_);
            format_free(next.ft);

            valuelen += strlen(expanded);
            value = xrealloc(value.cast(), valuelen).as_ptr().cast();
            strlcat(value, expanded, valuelen);
            free_(expanded);
        }

        free_(active);
        free_(all);
        value
    }
}

/// C `vendor/tmux/format.c:4801`: `static char *format_window_name(struct format_expand_state *es, const char *fmt)`
pub unsafe fn format_window_name(es: *mut format_expand_state, fmt: *const u8) -> *mut u8 {
    unsafe {
        let ft = (*es).ft;
        if (*ft).s.is_null() {
            format_log1!(es, c!("format_window_name"), "window name but no session",);
            return null_mut();
        }

        let name = format_expand1(es, fmt);
        for wl in rb_foreach(&raw mut (*(*ft).s).windows).map(NonNull::as_ptr) {
            if strcmp((*(*wl).window).name, name) == 0 {
                free_(name);
                return xstrdup(c!("1")).as_ptr();
            }
        }
        free_(name);
        xstrdup(c!("0")).as_ptr()
    }
}

/// C `vendor/tmux/format.c:4856`: `static char *format_loop_windows(struct format_expand_state *es, const char *fmt)`
pub unsafe fn format_loop_windows(
    es: *mut format_expand_state,
    fmt: *const u8,
    sc: sort_criteria,
) -> *mut u8 {
    unsafe {
        let ft = (*es).ft;
        let c = (*ft).client;
        let item = (*ft).item;
        let mut all: *mut u8 = null_mut();
        let mut active: *mut u8 = null_mut();
        let mut value: *mut u8 = xcalloc(1, 1).as_ptr().cast();
        let mut valuelen = 1;

        if (*ft).s.is_null() {
            format_log1!(es, c!("format_loop_windows"), "window loop but no session",);
            return null_mut();
        }

        if format_choose(es, fmt, &mut all, &mut active, 0) != 0 {
            all = xstrdup(fmt).as_ptr();
            active = null_mut();
        }

        // C: sort_get_winlinks_session() collects the winlinks then sorts
        // (default SORT_ORDER for `#{W:}` — the natural RB/index order).
        let winlinks = sort_get_winlinks_session((*ft).s, sc);
        let n = winlinks.len();
        for (i, &wl) in winlinks.iter().enumerate() {
            let w = (*wl).window;
            format_log1!(
                es,
                c!("format_loop_windows"),
                "window loop: {} @{}",
                (*wl).idx,
                (*w).id,
            );
            let use_ = if !active.is_null() && wl == (*(*ft).s).curw {
                active
            } else {
                all
            };

            let nft = format_create(c, item, FORMAT_WINDOW as i32 | (*w).id as i32, (*ft).flags);
            format_add!(nft, "loop_index", "{}", i);
            format_add!(nft, "loop_last_flag", "{}", i32::from(i == n - 1));
            format_defaults(nft, (*ft).c, NonNull::new((*ft).s), NonNull::new(wl), None);
            let mut next = zeroed();
            format_copy_state(&raw mut next, es, format_expand_flags::empty());
            next.ft = nft;
            let expanded = format_expand1(&mut next, use_);
            format_free(nft);

            valuelen += strlen(expanded);
            value = xrealloc(value.cast(), valuelen).as_ptr().cast();
            strlcat(value, expanded, valuelen);
            free_(expanded);
        }

        free_(active);
        free_(all);
        value
    }
}

/// Loop over panes.
/// C `vendor/tmux/format.c:4932`: `static char *format_loop_panes(struct format_expand_state *es, const char *fmt)`
pub unsafe fn format_loop_panes(
    es: *mut format_expand_state,
    fmt: *const u8,
    sc: sort_criteria,
) -> *mut u8 {
    unsafe {
        let ft = (*es).ft;
        let c = (*ft).client;
        let item = (*ft).item;

        if (*ft).w.is_null() {
            format_log1!(es, c!("format_loop_panes"), "pane loop but no window");
            return null_mut();
        }

        let mut all: *mut u8 = null_mut();
        let mut active: *mut u8 = null_mut();
        if format_choose(es, fmt, &raw mut all, &raw mut active, 0) != 0 {
            all = xstrdup(fmt).as_ptr();
            active = null_mut();
        }

        let mut value: *mut u8 = xcalloc(1, 1).as_ptr().cast();
        let mut valuelen = 1;

        let mut next = MaybeUninit::<format_expand_state>::uninit();
        let next = next.as_mut_ptr();
        // C: sort_get_panes_window() collects the pane TAILQ then sorts.
        // `#{P:...}` defaults to SORT_CREATION (by pane id) — see
        // vendor/tmux/format.c:5378 (`case 'P'`) and sort.c sort_pane_cmp.
        let panes = sort_get_panes_window((*ft).w, sc);
        let n = panes.len();
        for (i, &wp) in panes.iter().enumerate() {
            format_log1!(es, c!("format_loop_panes"), "pane loop: %{}", (*wp).id,);
            let use_ = if !active.is_null() && wp == (*(*ft).w).active {
                active
            } else {
                all
            };
            let nft = format_create(c, item, FORMAT_PANE as i32 | (*wp).id as i32, (*ft).flags);
            format_add!(nft, "loop_index", "{}", i);
            format_add!(nft, "loop_last_flag", "{}", i32::from(i == n - 1));
            format_defaults(
                nft,
                (*ft).c,
                NonNull::new((*ft).s),
                NonNull::new((*ft).wl),
                NonNull::new(wp),
            );
            format_copy_state(next, es, format_expand_flags::empty());
            (*next).ft = nft;
            let expanded = format_expand1(next, use_);
            format_free(nft);

            valuelen += strlen(expanded);
            value = xrealloc(value.cast(), valuelen).as_ptr().cast();

            strlcat(value, expanded, valuelen);
            free_(expanded);
        }

        free_(active);
        free_(all);

        value
    }
}

/// Loop over clients.
/// C `vendor/tmux/format.c:4995`: `static char *format_loop_clients(struct format_expand_state *es, const char *fmt)`
pub unsafe fn format_loop_clients(
    es: *mut format_expand_state,
    fmt: *const u8,
    sc: sort_criteria,
) -> *mut u8 {
    unsafe {
        let ft = (*es).ft;
        let item = (*ft).item;
        let mut next = MaybeUninit::<format_expand_state>::uninit();
        let next = next.as_mut_ptr();

        let mut value = xcalloc(1, 1).as_ptr();
        let mut valuelen = 1;

        // C: sort_get_clients() collects only attached clients, then sorts
        // (default SORT_ORDER for `#{L:}` — the natural registration order).
        let clients = sort_get_clients(sc);
        for &c in &clients {
            format_log1!(
                es,
                c!("format_loop_clients"),
                "client loop: {}",
                _s((*c).name),
            );
            let nft = format_create(c, item, 0, (*ft).flags);
            format_defaults(
                nft,
                c,
                NonNull::new((*ft).s),
                NonNull::new((*ft).wl),
                NonNull::new((*ft).wp),
            );
            format_copy_state(next, es, format_expand_flags::empty());
            (*next).ft = nft;
            let expanded = format_expand1(next, fmt);
            format_free(nft);

            valuelen += strlen(expanded);
            value = xrealloc(value.cast(), valuelen).as_ptr().cast();

            strlcat(value.cast(), expanded, valuelen);
            free_(expanded);
        }

        value.cast()
    }
}

/// C `vendor/tmux/format.c:5038`: `static char *format_replace_expression(struct format_modifier *mexp, struct format_expand_state *es, const char *copy)`
pub unsafe fn format_replace_expression(
    mexp: *mut format_modifier,
    es: *mut format_expand_state,
    copy: *const u8,
) -> *mut u8 {
    unsafe {
        let argc = (*mexp).argc;

        let mut endch: *mut u8 = null_mut();
        let value: *mut u8;

        let mut left: *mut u8 = null_mut();
        let mut right: *mut u8 = null_mut();

        'fail: {
            let mut use_fp: i32 = 0;
            let mut prec: u32 = 0;

            let mut mleft: f64;
            let mut mright: f64;

            enum Operator {
                Add,
                Subtract,
                Multiply,
                Divide,
                Modulus,
                Equal,
                NotEqual,
                GreaterThan,
                GreaterThanEqual,
                LessThan,
                LessThanEqual,
            }

            let operator;

            if streq_(*(*mexp).argv, "+") {
                operator = Operator::Add;
            } else if streq_(*(*mexp).argv, "-") {
                operator = Operator::Subtract;
            } else if streq_(*(*mexp).argv, "*") {
                operator = Operator::Multiply;
            } else if streq_(*(*mexp).argv, "/") {
                operator = Operator::Divide;
            } else if streq_(*(*mexp).argv, "%") || streq_(*(*mexp).argv, "m") {
                operator = Operator::Modulus;
            } else if streq_(*(*mexp).argv, "==") {
                operator = Operator::Equal;
            } else if streq_(*(*mexp).argv, "!=") {
                operator = Operator::NotEqual;
            } else if streq_(*(*mexp).argv, ">") {
                operator = Operator::GreaterThan;
            } else if streq_(*(*mexp).argv, "<") {
                operator = Operator::LessThan;
            } else if streq_(*(*mexp).argv, ">=") {
                operator = Operator::GreaterThanEqual;
            } else if streq_(*(*mexp).argv, "<=") {
                operator = Operator::LessThanEqual;
            } else {
                format_log1!(
                    es,
                    c!("format_replace_expression"),
                    "expression has no valid operator: '{}'",
                    _s(*(*mexp).argv),
                );
                break 'fail;
            }

            // The second argument may be flags.
            if argc >= 2 && !strchr(*(*mexp).argv.add(1), b'f' as i32).is_null() {
                use_fp = 1;
                prec = 2;
            }

            // The third argument may be precision.
            if argc >= 3 {
                prec = match strtonum(*(*mexp).argv.add(2), i32::MIN, i32::MAX) {
                    Ok(value) => value as u32,
                    Err(errstr) => {
                        format_log1!(
                            es,
                            c!("format_replace_expression"),
                            "expression precision {}: {}",
                            errstr.to_string_lossy(),
                            _s(*(*mexp).argv.add(2)),
                        );
                        break 'fail;
                    }
                }
            }

            if format_choose(es, copy, &raw mut left, &raw mut right, 1) != 0 {
                format_log1!(
                    es,
                    c!("format_replace_expression"),
                    "expression syntax error"
                );
                break 'fail;
            }

            mleft = strtod(left, &raw mut endch);
            if *endch != b'\0' {
                format_log1!(
                    es,
                    c!("format_replace_expression"),
                    "expression left side is invalid: {}",
                    _s(left),
                );
                break 'fail;
            }

            mright = strtod(right, &raw mut endch);
            if *endch != b'\0' {
                format_log1!(
                    es,
                    c!("format_replace_expression"),
                    "expression right side is invalid: {}",
                    _s(right),
                );
                break 'fail;
            }

            if use_fp == 0 {
                mleft = (mleft as c_longlong) as f64;
                mright = (mright as c_longlong) as f64;
            }
            format_log1!(
                es,
                c!("format_replace_expression"),
                "expression left side is: {1:0$}",
                prec as usize,
                mleft,
            );
            format_log1!(
                es,
                c!("format_replace_expression"),
                "expression right side is: {1:0$}",
                prec as usize,
                mright,
            );

            let result = match operator {
                Operator::Add => mleft + mright,
                Operator::Subtract => mleft - mright,
                Operator::Multiply => mleft * mright,
                Operator::Divide => mleft / mright,
                Operator::Modulus => mleft % mright,
                Operator::Equal => ((mleft - mright).abs() < 1e-9) as i32 as f64,
                Operator::NotEqual => ((mleft - mright).abs() > 1e-9) as i32 as f64,
                Operator::GreaterThan => (mleft > mright) as i32 as f64,
                Operator::GreaterThanEqual => (mleft >= mright) as i32 as f64,
                Operator::LessThan => (mleft < mright) as i32 as f64,
                Operator::LessThanEqual => (mleft <= mright) as i32 as f64,
            };

            value = if use_fp != 0 {
                format_nul!("{:.*}", prec as usize, result)
            } else {
                // C `format.c`: `(double)(long long)result`. `result` can be
                // non-finite here (`#{e|/|:5,0}` -> +inf, `#{e|%|:5,0}` -> NaN),
                // and C's `(long long)` cast of inf/NaN is undefined — on x86 it
                // yields LLONG_MIN via cvttsd2si, on arm64 it saturates. Rust's
                // `as` cast is instead *defined* (saturating), which diverges
                // from the vendored tmux on x86_64. `to_int_unchecked` emits the
                // same hardware conversion the C cast does, so ztmux stays
                // byte-for-byte identical to tmux on every arch (see the
                // arith_divzero/modzero parity cases).
                let truncated = result.to_int_unchecked::<c_longlong>();
                format_nul!("{:.*}", prec as usize, truncated as f64)
            };
            format_log1!(
                es,
                c!("format_replace_expression"),
                "expression result is {}",
                _s(value),
            );

            free_(right);
            free_(left);
            return value;
        }

        // fail:
        free_(right);
        free_(left);
        null_mut()
    }
}

/// Replace a key.
/// C `vendor/tmux/format.c:5182`: `static int format_replace(struct format_expand_state *es, const char *key, size_t keylen, char **buf, size_t *len, size_t *off)`
pub unsafe fn format_replace(
    es: *mut format_expand_state,
    key: *const u8,
    keylen: usize,
    buf: *mut *mut u8,
    len: *mut usize,
    off: *mut usize,
) -> i32 {
    let __func__: *const u8 = c!("format_replace");

    unsafe {
        let ft = (*es).ft;
        let wp = (*ft).wp;
        let mut copy: *const u8;
        let cp: *const u8;
        let mut marker: *const u8 = null();

        let mut time_format: *const u8 = null();

        let copy0: *mut u8;
        let condition: *mut u8;
        let mut found: *mut u8;
        let mut new: *mut u8;
        let mut value: *mut u8 = null_mut();
        let mut left: *mut u8 = null_mut();
        let mut right: *mut u8 = null_mut();

        let valuelen;

        let mut modifiers: format_modifiers = format_modifiers::empty();
        // C `format_replace`: sorting defaults reset per key, overridden by the
        // S/W/P/L modifier's argument (see vendor/tmux/format.c:5203, 5335).
        let mut sc = sort_criteria {
            order: sort_order::SORT_ORDER,
            reversed: false,
            order_seq: null_mut(),
        };
        let mut limit: i32 = 0;
        let mut width: i32 = 0;

        let mut c;

        let list: *mut format_modifier;
        let mut cmp: *mut format_modifier = null_mut();
        let mut search: *mut format_modifier = null_mut();

        let mut sub: *mut *mut format_modifier = null_mut();
        let mut mexp: *mut format_modifier = null_mut();

        // let mut i = 0u32;
        let mut count = 0u32;
        let mut nsub = 0u32;

        let mut next = MaybeUninit::<format_expand_state>::uninit();
        let next = next.as_mut_ptr();

        'fail: {
            'done: {
                // Make a copy of the key.
                copy0 = xstrndup(key, keylen).as_ptr();
                copy = copy0;

                // Process modifier list.
                list = format_build_modifiers(es, &raw mut copy, &raw mut count);
                for i in 0..count {
                    let fm = list.add(i as usize);
                    if format_logging(&*ft) {
                        format_log1!(
                            es,
                            __func__,
                            "modifier {} is {}",
                            i,
                            _s((&raw mut (*fm).modifier).cast::<u8>())
                        );
                        for j in 0..(*fm).argc {
                            format_log1!(
                                es,
                                __func__,
                                "modifier {} argument {}: {}",
                                i,
                                j,
                                _s(*(*fm).argv.add(j as usize)),
                            );
                        }
                    }
                    if (*fm).size == 1 {
                        match (*fm).modifier[0] {
                            b'm' | b'<' | b'>' => cmp = fm,
                            b'!' => modifiers |= format_modifiers::FORMAT_NOT,
                            b'C' => search = fm,
                            b's' => {
                                if (*fm).argc < 2 {
                                } else {
                                    sub = xreallocarray_(sub, nsub as usize + 1).as_ptr();
                                    *sub.add(nsub as usize) = fm;
                                    nsub += 1;
                                }
                            }
                            b'=' => {
                                if (*fm).argc < 1 {
                                } else {
                                    limit = strtonum(*(*fm).argv, i32::MIN, i32::MAX)
                                        .unwrap_or_default();
                                    if (*fm).argc >= 2 && !(*(*fm).argv.add(1)).is_null() {
                                        marker = *(*fm).argv.add(1);
                                    }
                                }
                            }
                            b'p' => {
                                if (*fm).argc < 1 {
                                } else {
                                    width = strtonum(*(*fm).argv, i32::MIN, i32::MAX)
                                        .unwrap_or_default();
                                }
                            }
                            b'w' => modifiers |= format_modifiers::FORMAT_WIDTH,
                            b'e' => {
                                if (*fm).argc < 1 || (*fm).argc > 3 {
                                } else {
                                    mexp = fm;
                                }
                            }
                            b'l' => modifiers |= format_modifiers::FORMAT_LITERAL,
                            b'a' => modifiers |= format_modifiers::FORMAT_CHARACTER,
                            b'b' => modifiers |= format_modifiers::FORMAT_BASENAME,
                            b'c' => {
                                modifiers |= format_modifiers::FORMAT_COLOUR;
                                if (*fm).argc >= 1 {
                                    if !strchr(*(*fm).argv, b'f' as i32).is_null() {
                                        modifiers |= format_modifiers::FORMAT_COLOUR_ESC_FG;
                                    }
                                    if !strchr(*(*fm).argv, b'b' as i32).is_null() {
                                        modifiers |= format_modifiers::FORMAT_COLOUR_ESC_BG;
                                    }
                                }
                            }
                            b'd' => modifiers |= format_modifiers::FORMAT_DIRNAME,
                            b'n' => modifiers |= format_modifiers::FORMAT_LENGTH,
                            b'R' => modifiers |= format_modifiers::FORMAT_REPEAT,
                            b't' => {
                                modifiers |= format_modifiers::FORMAT_TIMESTRING;
                                if (*fm).argc >= 1 {
                                    if !strchr(*(*fm).argv, b'p' as i32).is_null() {
                                        modifiers |= format_modifiers::FORMAT_PRETTY;
                                    } else if (*fm).argc >= 2
                                        && !strchr(*(*fm).argv, b'f' as i32).is_null()
                                    {
                                        time_format = format_strip(es, *(*fm).argv.add(1));
                                    }
                                }
                            }
                            b'q' => {
                                if (*fm).argc < 1 {
                                    modifiers |= format_modifiers::FORMAT_QUOTE_SHELL;
                                } else if !strchr(*(*fm).argv, b'e' as i32).is_null()
                                    || !strchr(*(*fm).argv, b'h' as i32).is_null()
                                {
                                    modifiers |= format_modifiers::FORMAT_QUOTE_STYLE;
                                }
                            }
                            b'E' => modifiers |= format_modifiers::FORMAT_EXPAND,
                            b'T' => modifiers |= format_modifiers::FORMAT_EXPANDTIME,
                            b'N' => {
                                if (*fm).argc < 1 || !strchr(*(*fm).argv, b'w' as i32).is_null() {
                                    modifiers |= format_modifiers::FORMAT_WINDOW_NAME;
                                } else if !strchr(*(*fm).argv, b's' as i32).is_null() {
                                    modifiers |= format_modifiers::FORMAT_SESSION_NAME;
                                }
                            }
                            b'S' => {
                                modifiers |= format_modifiers::FORMAT_SESSIONS;
                                sc.order = sort_order::SORT_INDEX;
                                sc.reversed = false;
                                if (*fm).argc >= 1 {
                                    let a = *(*fm).argv;
                                    sc.order = if !strchr(a, b'n' as i32).is_null() {
                                        sort_order::SORT_NAME
                                    } else if !strchr(a, b't' as i32).is_null() {
                                        sort_order::SORT_ACTIVITY
                                    } else {
                                        sort_order::SORT_INDEX
                                    };
                                    sc.reversed = !strchr(a, b'r' as i32).is_null();
                                }
                            }
                            b'W' => {
                                modifiers |= format_modifiers::FORMAT_WINDOWS;
                                sc.order = sort_order::SORT_ORDER;
                                sc.reversed = false;
                                if (*fm).argc >= 1 {
                                    let a = *(*fm).argv;
                                    sc.order = if !strchr(a, b'n' as i32).is_null() {
                                        sort_order::SORT_NAME
                                    } else if !strchr(a, b't' as i32).is_null() {
                                        sort_order::SORT_ACTIVITY
                                    } else {
                                        sort_order::SORT_ORDER
                                    };
                                    sc.reversed = !strchr(a, b'r' as i32).is_null();
                                }
                            }
                            b'P' => {
                                modifiers |= format_modifiers::FORMAT_PANES;
                                sc.order = sort_order::SORT_CREATION;
                                sc.reversed = false;
                                if (*fm).argc >= 1 {
                                    sc.reversed = !strchr(*(*fm).argv, b'r' as i32).is_null();
                                }
                            }
                            b'L' => {
                                modifiers |= format_modifiers::FORMAT_CLIENTS;
                                sc.order = sort_order::SORT_ORDER;
                                sc.reversed = false;
                                if (*fm).argc >= 1 {
                                    let a = *(*fm).argv;
                                    sc.order = if !strchr(a, b'n' as i32).is_null() {
                                        sort_order::SORT_NAME
                                    } else if !strchr(a, b't' as i32).is_null() {
                                        sort_order::SORT_ACTIVITY
                                    } else {
                                        sort_order::SORT_ORDER
                                    };
                                    sc.reversed = !strchr(a, b'r' as i32).is_null();
                                }
                            }
                            _ => (),
                        }
                    } else if (*fm).size == 2 && streq_((*fm).modifier.as_ptr(), "!!") {
                        modifiers |= format_modifiers::FORMAT_NOT_NOT;
                    } else if (*fm).size == 2
                        && (streq_((*fm).modifier.as_ptr(), "||")
                            || streq_((*fm).modifier.as_ptr(), "&&")
                            || streq_((*fm).modifier.as_ptr(), "==")
                            || streq_((*fm).modifier.as_ptr(), "!=")
                            || streq_((*fm).modifier.as_ptr(), ">=")
                            || streq_((*fm).modifier.as_ptr(), "<="))
                    {
                        cmp = fm;
                    }
                }

                // Is this a literal string?
                if modifiers.intersects(format_modifiers::FORMAT_LITERAL) {
                    format_log1!(es, __func__, "literal string is '{}'", _s(copy));
                    value = format_unescape(es, copy);
                    break 'done;
                }

                // Is this a character?
                if modifiers.intersects(format_modifiers::FORMAT_CHARACTER) {
                    new = format_expand1(es, copy);
                    value = match strtonum::<u8>(new, 32, 126) {
                        Ok(n) => format_nul!("{}", n as char),
                        Err(_) => xstrdup(c!("")).as_ptr(),
                    };
                    free_(new);
                    break 'done;
                }

                // Is this a colour?
                if modifiers.intersects(format_modifiers::FORMAT_COLOUR) {
                    new = format_expand1(es, copy);
                    if modifiers.intersects(
                        format_modifiers::FORMAT_COLOUR_ESC_FG
                            | format_modifiers::FORMAT_COLOUR_ESC_BG,
                    ) {
                        if strcaseeq_(new, "none") {
                            value = xstrdup(c!("\x1b[0m")).as_ptr();
                        } else {
                            // C: `else if ((c = colour_fromstring(new)) == -1)`.
                            // Rust forbids assignment-in-condition, so the
                            // assignment is hoisted into the `else` before the
                            // `if`.
                            c = colour_fromstring(cstr_to_str_(new).unwrap_or(""));
                            if c == -1 {
                                value = xstrdup(c!("")).as_ptr();
                            } else {
                                let cp = if modifiers
                                    .intersects(format_modifiers::FORMAT_COLOUR_ESC_BG)
                                {
                                    colour_toescape((*ft).c, c, 1)
                                } else {
                                    colour_toescape((*ft).c, c, 0)
                                };
                                value = if cp.is_null() {
                                    xstrdup(c!("")).as_ptr()
                                } else {
                                    xstrdup(cp).as_ptr()
                                };
                            }
                        }
                    } else {
                        c = colour_fromstring(cstr_to_str_(new).unwrap_or(""));
                        value = if c == -1
                            || ({
                                c = colour_force_rgb(c);
                                c == -1
                            }) {
                            xstrdup(c!("")).as_ptr()
                        } else {
                            format_nul!("{:06x}", c & 0xffffff)
                        };
                    }
                    free_(new);
                    break 'done;
                }

                // Is this a loop, comparison or condition?
                if modifiers.intersects(format_modifiers::FORMAT_SESSIONS) {
                    value = format_loop_sessions(es, copy, sc);
                    if value.is_null() {
                        break 'fail;
                    }
                } else if modifiers.intersects(format_modifiers::FORMAT_WINDOWS) {
                    value = format_loop_windows(es, copy, sc);
                    if value.is_null() {
                        break 'fail;
                    }
                } else if modifiers.intersects(format_modifiers::FORMAT_PANES) {
                    value = format_loop_panes(es, copy, sc);
                    if value.is_null() {
                        break 'fail;
                    }
                } else if modifiers.intersects(format_modifiers::FORMAT_CLIENTS) {
                    value = format_loop_clients(es, copy, sc);
                    if value.is_null() {
                        break 'fail;
                    }
                } else if modifiers.intersects(format_modifiers::FORMAT_WINDOW_NAME) {
                    value = format_window_name(es, copy);
                    if value.is_null() {
                        break 'fail;
                    }
                } else if modifiers.intersects(format_modifiers::FORMAT_SESSION_NAME) {
                    value = format_session_name(es, copy);
                    if value.is_null() {
                        break 'fail;
                    }
                } else if !search.is_null() {
                    // Search in pane.
                    new = format_expand1(es, copy);
                    if wp.is_null() {
                        format_log1!(es, __func__, "search '{}' but no pane", _s(new));
                        value = xstrdup(c!("0")).as_ptr();
                    } else {
                        format_log1!(es, __func__, "search '{}' pane %{}", _s(new), (*wp).id,);
                        value = format_search(search, wp, new);
                    }
                    free_(new);
                } else if modifiers.intersects(format_modifiers::FORMAT_REPEAT) {
                    // Repeat the left argument right times (format.c FORMAT_REPEAT).
                    let mut left: *mut u8 = null_mut();
                    let mut right: *mut u8 = null_mut();
                    if format_choose(es, copy, &raw mut left, &raw mut right, 1) != 0 {
                        format_log1!(es, __func__, "repeat syntax error: {}", _s(copy));
                        break 'fail;
                    }
                    match strtonum_(cstr_to_str(right), 1i32, FORMAT_MAX_REPEAT) {
                        Err(_) => value = xstrdup(c!("")).as_ptr(),
                        Ok(nrep) => {
                            value = xstrdup(c!("")).as_ptr();
                            let mut i = 0;
                            while i < nrep {
                                if format_check_time(es, null_mut()) == 0 {
                                    free_(right);
                                    free_(left);
                                    free_(value);
                                    break 'fail;
                                }
                                new = format_nul!("{}{}", _s(value), _s(left));
                                free_(value);
                                value = new;
                                i += 1;
                            }
                        }
                    }
                    free_(right);
                    free_(left);
                } else if modifiers.intersects(format_modifiers::FORMAT_NOT) {
                    // Logical NOT of the (expanded) argument.
                    // C: value = format_bool_op_1(es, copy, 1).
                    new = format_expand1(es, copy);
                    value = if format_true(new) {
                        xstrdup(c!("0")).as_ptr()
                    } else {
                        xstrdup(c!("1")).as_ptr()
                    };
                    format_log1!(es, __func__, "not of '{}' is: {}", _s(new), _s(value));
                    free_(new);
                } else if modifiers.intersects(format_modifiers::FORMAT_NOT_NOT) {
                    // Boolean coercion of the (expanded) argument.
                    // C: value = format_bool_op_1(es, copy, 0).
                    new = format_expand1(es, copy);
                    value = if format_true(new) {
                        xstrdup(c!("1")).as_ptr()
                    } else {
                        xstrdup(c!("0")).as_ptr()
                    };
                    format_log1!(es, __func__, "not-not of '{}' is: {}", _s(new), _s(value));
                    free_(new);
                } else if !cmp.is_null() {
                    // Comparison of left and right.
                    if format_choose(es, copy, &raw mut left, &raw mut right, 1) != 0 {
                        format_log1!(
                            es,
                            __func__,
                            "compare {} syntax error: {}",
                            _s((&raw const (*cmp).modifier).cast::<u8>()),
                            _s(copy),
                        );
                        break 'fail;
                    }
                    format_log1!(
                        es,
                        __func__,
                        "compare {} left is: {}",
                        _s((&raw const (*cmp).modifier).cast::<u8>()),
                        _s(left),
                    );
                    format_log1!(
                        es,
                        __func__,
                        "compare {} right is: {}",
                        _s((&raw const (*cmp).modifier).cast::<u8>()),
                        _s(right),
                    );

                    if streq_((*cmp).modifier.as_ptr(), "||") {
                        if format_true(left) || format_true(right) {
                            value = xstrdup(c!("1")).as_ptr();
                        } else {
                            value = xstrdup(c!("0")).as_ptr();
                        }
                    } else if streq_((*cmp).modifier.as_ptr(), "&&") {
                        if format_true(left) && format_true(right) {
                            value = xstrdup(c!("1")).as_ptr();
                        } else {
                            value = xstrdup(c!("0")).as_ptr();
                        }
                    } else if streq_((*cmp).modifier.as_ptr(), "==") {
                        if strcmp(left, right) == 0 {
                            value = xstrdup(c!("1")).as_ptr();
                        } else {
                            value = xstrdup(c!("0")).as_ptr();
                        }
                    } else if streq_((*cmp).modifier.as_ptr(), "!=") {
                        if strcmp(left, right) != 0 {
                            value = xstrdup(c!("1")).as_ptr();
                        } else {
                            value = xstrdup(c!("0")).as_ptr();
                        }
                    } else if streq_((*cmp).modifier.as_ptr(), "<") {
                        if strcmp(left, right) < 0 {
                            value = xstrdup(c!("1")).as_ptr();
                        } else {
                            value = xstrdup(c!("0")).as_ptr();
                        }
                    } else if streq_((*cmp).modifier.as_ptr(), ">") {
                        if strcmp(left, right) > 0 {
                            value = xstrdup(c!("1")).as_ptr();
                        } else {
                            value = xstrdup(c!("0")).as_ptr();
                        }
                    } else if streq_((*cmp).modifier.as_ptr(), "<=") {
                        if strcmp(left, right) <= 0 {
                            value = xstrdup(c!("1")).as_ptr();
                        } else {
                            value = xstrdup(c!("0")).as_ptr();
                        }
                    } else if streq_((*cmp).modifier.as_ptr(), ">=") {
                        if strcmp(left, right) >= 0 {
                            value = xstrdup(c!("1")).as_ptr();
                        } else {
                            value = xstrdup(c!("0")).as_ptr();
                        }
                    } else if streq_((*cmp).modifier.as_ptr(), "m") {
                        value = format_match(cmp, left, right);
                    }

                    free_(right);
                    free_(left);
                } else if *copy == b'?' {
                    // Conditional: check first and choose second or third.
                    cp = format_skip1(es, copy.add(1), c!(","));
                    if cp.is_null() {
                        format_log1!(es, __func__, "condition syntax error: {}", _s(copy.add(1)),);
                        break 'fail;
                    }
                    condition =
                        xstrndup(copy.add(1), cp.offset_from(copy.add(1)) as usize).as_ptr();
                    format_log1!(es, __func__, "condition is: {}", _s(condition));

                    found = format_find(ft, condition, modifiers, time_format);
                    if found.is_null() {
                        // If the condition not found, try to expand it. If
                        // the expansion doesn't have any effect, then assume
                        // false.
                        found = format_expand1(es, condition);
                        if strcmp(found, condition) == 0 {
                            free_(found);
                            found = xstrdup(c!("")).as_ptr();
                            format_log1!(
                                es,
                                __func__,
                                "condition '{}' not found; assuming false",
                                _s(condition),
                            );
                        }
                    } else {
                        format_log1!(
                            es,
                            __func__,
                            "condition '{}' found: {}",
                            _s(condition),
                            _s(found),
                        );
                    }

                    if format_choose(es, cp.add(1), &raw mut left, &raw mut right, 0) != 0 {
                        format_log1!(
                            es,
                            __func__,
                            "condition '{}' syntax error: {}",
                            _s(condition),
                            _s(cp.add(1)),
                        );
                        free_(found);
                        break 'fail;
                    }
                    if format_true(found) {
                        format_log1!(es, __func__, "condition '{}' is true", _s(condition));
                        value = format_expand1(es, left);
                    } else {
                        format_log1!(es, __func__, "condition '{}' is false", _s(condition));
                        value = format_expand1(es, right);
                    }
                    free_(right);
                    free_(left);

                    free_(condition);
                    free_(found);
                } else if !mexp.is_null() {
                    value = format_replace_expression(mexp, es, copy);
                    if value.is_null() {
                        value = xstrdup(c!("")).as_ptr();
                    }
                } else if !strstr(copy, c!("#{")).is_null() {
                    format_log1!(es, __func__, "expanding inner format '{}'", _s(copy));
                    value = format_expand1(es, copy);
                } else {
                    value = format_find(ft, copy, modifiers, time_format);
                    if value.is_null() {
                        format_log1!(es, __func__, "format '{}' not found", _s(copy));
                        value = xstrdup(c!("")).as_ptr();
                    } else {
                        format_log1!(es, __func__, "format '{}' found: {}", _s(copy), _s(value),);
                    }
                }
            }
            // done:

            // Expand again if required.
            if modifiers.intersects(format_modifiers::FORMAT_EXPAND) {
                new = format_expand1(es, value);
                free_(value);
                value = new;
            } else if modifiers.intersects(format_modifiers::FORMAT_EXPANDTIME) {
                format_copy_state(next, es, format_expand_flags::FORMAT_EXPAND_TIME);
                new = format_expand1(next, value);
                free_(value);
                value = new;
            }

            // Perform substitution if any.
            for i in 0..nsub {
                left = format_expand1(es, *(**sub.add(i as usize)).argv);
                right = format_expand1(es, *(**sub.add(i as usize)).argv.add(1));
                new = format_sub(*sub.add(i as usize), value, left, right);
                format_log1!(
                    es,
                    __func__,
                    "substitute '{}' to '{}': {}",
                    _s(left),
                    _s(right),
                    _s(new),
                );
                free_(value);
                value = new;
                free_(right);
                free_(left);
            }

            // Truncate the value if needed.
            if limit > 0 {
                new = format_trim_left(value, limit as u32);
                value = if !marker.is_null() && strcmp(new, value) != 0 {
                    free_(value);
                    format_nul!("{}{}", _s(new), _s(marker))
                } else {
                    free_(value);
                    new
                };
                format_log1!(
                    es,
                    __func__,
                    "applied length limit {}: {}",
                    limit,
                    _s(value),
                );
            } else if limit < 0 {
                new = format_trim_right(value, (-limit) as u32);
                value = if !marker.is_null() && strcmp(new, value) != 0 {
                    free_(value);
                    format_nul!("{}{}", _s(marker), _s(new))
                } else {
                    free_(value);
                    new
                };
                format_log1!(
                    es,
                    __func__,
                    "applied length limit {}: {}",
                    limit,
                    _s(value),
                );
            }

            // Pad the value if needed.
            if width > 0 {
                new = utf8_padcstr(value, width as u32);
                free_(value);
                value = new;
                format_log1!(
                    es,
                    __func__,
                    "applied padding width {}: {}",
                    width,
                    _s(value),
                );
            } else if width < 0 {
                new = utf8_rpadcstr(value, (-width) as u32);
                free_(value);
                value = new;
                format_log1!(
                    es,
                    __func__,
                    "applied padding width {}: {}",
                    width,
                    _s(value),
                );
            }

            // Replace with the length or width if needed.
            if modifiers.intersects(format_modifiers::FORMAT_LENGTH) {
                new = format_nul!("{}", strlen(value));
                free_(value);
                value = new;
                format_log1!(es, __func__, "replacing with length: {}", _s(new));
            }
            if modifiers.intersects(format_modifiers::FORMAT_WIDTH) {
                // On invalid UTF-8, C's byte-based width counts each stray byte
                // as ~1 column, so fall back to the byte length rather than
                // panicking in cstr_to_str.
                let w = match cstr_to_str_(value) {
                    Some(v) => format_width(v),
                    None => strlen(value) as u32,
                };
                new = format_nul!("{}", w);
                free_(value);
                value = new;
                format_log1!(es, __func__, "replacing with width: {}", _s(new));
            }

            // Expand the buffer and copy in the value.
            valuelen = strlen(value);
            while *len - *off < valuelen + 1 {
                *buf = xreallocarray((*buf).cast(), 2, *len).as_ptr().cast();
                *len *= 2;
            }
            memcpy((*buf).add(*off).cast(), value.cast(), valuelen);
            *off += valuelen;

            format_log1!(
                es,
                __func__,
                "replaced '{}' with '{}'",
                _s(copy0),
                _s(value),
            );
            free_(value);

            free_(sub);
            format_free_modifiers(list, count);
            free_(copy0);
            return 0;
        }

        // fail:
        format_log1!(es, __func__, "failed {}", _s(copy0));

        free_(sub);
        format_free_modifiers(list, count);
        free_(copy0);
        -1
    }
}

/// Expand keys in a template.
/// C `vendor/tmux/format.c:5826`: `static char *format_expand1(struct format_expand_state *es, const char *fmt)`
pub unsafe fn format_expand1(es: *mut format_expand_state, mut fmt: *const u8) -> *mut u8 {
    unsafe {
        let ft = (*es).ft;
        let mut out: *mut u8;

        let mut s: *const u8;
        let mut style_end: *const u8 = null();

        const SIZEOF_EXPANDED: usize = 8192;
        let mut expanded = MaybeUninit::<[u8; SIZEOF_EXPANDED]>::uninit();
        let expanded = expanded.as_mut_ptr() as *mut u8;

        if fmt.is_null() || *fmt == b'\0' || format_check_time(es, null_mut()) == 0 {
            return xstrdup(c!("")).as_ptr();
        }

        if (*es).loop_ == FORMAT_LOOP_LIMIT as u32 {
            format_log1!(
                es,
                c!("format_expand1"),
                "reached loop limit ({})",
                FORMAT_LOOP_LIMIT,
            );
            return xstrdup(c!("")).as_ptr();
        }
        (*es).loop_ += 1;

        format_log1!(es, c!("format_expand1"), "expanding format: {}", _s(fmt),);

        if ((*es)
            .flags
            .intersects(format_expand_flags::FORMAT_EXPAND_TIME))
            && !strchr(fmt, b'%' as i32).is_null()
        {
            if (*es).time == 0 {
                (*es).time = libc::time(null_mut());
                localtime_r(&raw mut (*es).time, &raw mut (*es).tm);
            }
            if strftime(expanded, SIZEOF_EXPANDED, fmt, &raw mut (*es).tm) == 0 {
                format_log1!(es, c!("format_expand1"), "format is too long",);
                return xstrdup(c!("")).as_ptr();
            }
            if format_logging(&*ft) && strcmp(expanded, fmt) != 0 {
                format_log1!(
                    es,
                    c!("format_expand1"),
                    "after time expanded: {}",
                    _s(expanded),
                );
            }
            fmt = expanded;
        }

        let mut len = 64;
        let mut buf: *mut u8 = xmalloc(len).as_ptr().cast();
        let mut off = 0;
        let mut n;

        while *fmt != b'\0' {
            if *fmt != b'#' {
                while len - off < 2 {
                    buf = xreallocarray(buf.cast(), 2, len).as_ptr().cast();
                    len *= 2;
                }
                *buf.add(off) = *fmt;
                off += 1;
                fmt = fmt.add(1);
                continue;
            }
            fmt = fmt.add(1);
            if *fmt == b'\0' {
                break;
            }

            let ch: u8 = *fmt;
            fmt = fmt.add(1);
            let mut brackets;

            let mut ptr: *const u8;
            match ch {
                b'(' => {
                    brackets = 1;
                    ptr = fmt;
                    while *ptr != b'\0' {
                        if *ptr == b'(' {
                            brackets += 1;
                        }
                        if *ptr == b')'
                            && ({
                                brackets -= 1;
                                brackets == 0
                            })
                        {
                            break;
                        }
                        ptr = ptr.add(1);
                    }
                    if *ptr != b')' || brackets != 0 {
                        break;
                    }
                    n = ptr.offset_from(fmt) as usize;

                    let name = xstrndup(fmt, n).as_ptr();
                    format_log1!(es, c!("format_expand1"), "found #(): {}", _s(name),);

                    if ((*ft).flags.intersects(format_flags::FORMAT_NOJOBS))
                        || ((*es)
                            .flags
                            .intersects(format_expand_flags::FORMAT_EXPAND_NOJOBS))
                    {
                        out = xstrdup(c!("")).as_ptr();
                        format_log1!(es, c!("format_expand1"), "#() is disabled");
                    } else {
                        out = format_job_get(es, name);
                        format_log1!(es, c!("format_expand1"), "#() result: {}", _s(out),);
                    }
                    free_(name);

                    let outlen = strlen(out);
                    while len - off < outlen + 1 {
                        buf = xreallocarray(buf.cast(), 2, len).as_ptr().cast();
                        len *= 2;
                    }
                    memcpy(buf.add(off).cast(), out.cast(), outlen);
                    off += outlen;

                    free_(out);

                    fmt = fmt.add(n + 1);
                    continue;
                }
                b'{' => {
                    ptr = format_skip1(es, fmt.sub(2), c!("}"));
                    if ptr.is_null() {
                        break;
                    }
                    n = ptr.offset_from(fmt) as usize;

                    format_log1!(es, c!("format_expand1"), "found #{}: {1:0$}", n, _s(fmt),);
                    if format_replace(es, fmt, n, &raw mut buf, &raw mut len, &raw mut off) != 0 {
                        break;
                    }
                    fmt = fmt.add(n + 1);
                    continue;
                }
                b'[' | b'#' => {
                    // If ##[ (with two or more #s), then it is a style and
                    // can be left for format_draw to handle.
                    ptr = fmt.sub((ch == b'[') as usize);
                    n = 2 - (ch == b'[') as usize;
                    while *ptr == b'#' {
                        ptr = ptr.add(1);
                        n += 1;
                    }
                    if *ptr == b'[' {
                        style_end = format_skip1(es, fmt.sub(2), c!("]"));
                        format_log1!(es, c!("format_expand1"), "found #*{}[", n);
                        while len - off < n + 2 {
                            buf = xreallocarray(buf.cast(), 2, len).as_ptr().cast();
                            len *= 2;
                        }
                        memcpy(buf.add(off).cast(), fmt.sub(2).cast(), n + 1);
                        off += n + 1;
                        fmt = ptr.add(1);
                        continue;
                    }
                    // FALLTHROUGH
                    format_log1!(es, c!("format_expand1"), "found #{}", ch as char);
                    while len - off < 2 {
                        buf = xreallocarray(buf.cast(), 2, len).as_ptr().cast();
                        len *= 2;
                    }
                    *buf.add(off) = ch;
                    off += 1;
                    continue;
                }
                // FALLTHROUGH
                b'}' | b',' => {
                    format_log1!(es, c!("format_expand1"), "found #{}", ch as char,);
                    while len - off < 2 {
                        buf = xreallocarray(buf.cast(), 2, len).as_ptr().cast();
                        len *= 2;
                    }
                    *buf.add(off) = ch;
                    off += 1;
                    continue;
                }
                _ => {
                    s = null_mut();
                    if fmt > style_end {
                        if ch.is_ascii_uppercase() {
                            s = FORMAT_UPPER[(ch - b'A') as usize].as_ptr();
                        } else if ch.is_ascii_lowercase() {
                            s = FORMAT_LOWER[(ch - b'a') as usize].as_ptr();
                        }
                    } /* skip inside #[] */
                    if s.is_null() {
                        while len - off < 3 {
                            buf = xreallocarray(buf.cast(), 2, len).as_ptr().cast();
                            len *= 2;
                        }
                        *buf.add(off) = b'#';
                        off += 1;
                        *buf.add(off) = ch;
                        off += 1;

                        continue;
                    }
                    n = strlen(s);
                    format_log1!(es, c!("format_expand1"), "found #{}: {}", ch as char, _s(s),);
                    if format_replace(es, s, n, &raw mut buf, &raw mut len, &raw mut off) != 0 {
                        break;
                    }
                    continue;
                }
            }

            #[expect(unreachable_code)]
            {
                break;
            }
        }
        *buf.add(off) = b'\0';

        format_log1!(es, c!("format_expand1"), "result is: {}", _s(buf),);
        (*es).loop_ -= 1;

        buf
    }
}

/// Expand keys in a template, passing through strftime first.
/// C `vendor/tmux/format.c:5997`: `char *format_expand_time(struct format_tree *ft, const char *fmt)`
pub unsafe fn format_expand_time(ft: *mut format_tree, fmt: *const u8) -> *mut u8 {
    unsafe {
        let mut es = MaybeUninit::<format_expand_state>::uninit();
        let es = es.as_mut_ptr();

        memset0(es);
        (*es).ft = ft;
        (*es).flags = format_expand_flags::FORMAT_EXPAND_TIME;
        (*es).start_time = get_timer();
        format_expand1(es, fmt)
    }
}

/// Expand keys in a template.
/// C `vendor/tmux/format.c:6010`: `char *format_expand(struct format_tree *ft, const char *fmt)`
pub unsafe fn format_expand(ft: *mut format_tree, fmt: *const u8) -> *mut u8 {
    unsafe {
        let mut es = MaybeUninit::<format_expand_state>::uninit();
        let es = es.as_mut_ptr();

        memset0(es);
        (*es).ft = ft;
        (*es).flags = format_expand_flags::empty();
        (*es).start_time = get_timer();
        format_expand1(es, fmt)
    }
}

/// Expand a single string.
/// C `vendor/tmux/format.c:6023`: `char *format_single(struct cmdq_item *item, const char *fmt, struct client *c, struct session *s, struct winlink *wl, struct window_pane *wp)`
pub unsafe fn format_single(
    item: *mut cmdq_item,
    fmt: &str,
    c: *mut client,
    s: *mut session,
    wl: *mut winlink,
    wp: *mut window_pane,
) -> *mut u8 {
    unsafe {
        let ft = format_create_defaults(item, c, s, wl, wp);
        let fmt = CString::new(fmt).unwrap(); // TODO shim to not have to rewrite
                                                       // format_expand now, remove later
        let expanded: *mut u8 = format_expand(ft, fmt.as_ptr().cast());
        format_free(ft);
        expanded
    }
}

/// Expand a single string using state.
/// C `vendor/tmux/format.c:6037`: `char *format_single_from_state(struct cmdq_item *item, const char *fmt, struct client *c, struct cmd_find_state *fs)`
pub unsafe fn format_single_from_state(
    item: *mut cmdq_item,
    fmt: &str,
    c: *mut client,
    fs: *mut cmd_find_state,
) -> *mut u8 {
    unsafe { format_single(item, fmt, c, (*fs).s, (*fs).wl, (*fs).wp) }
}

/// Expand a single string using target.
/// C `vendor/tmux/format.c:6045`: `char *format_single_from_target(struct cmdq_item *item, const char *fmt)`
pub unsafe fn format_single_from_target(item: *mut cmdq_item, fmt: *const u8) -> *mut u8 {
    unsafe {
        let tc = cmdq_get_target_client(item);

        format_single_from_state(item, cstr_to_str(fmt), tc, cmdq_get_target(item))
    }
}

/// Create and add defaults.
/// C `vendor/tmux/format.c:6054`: `struct format_tree *format_create_defaults(struct cmdq_item *item, struct client *c, struct session *s, struct winlink *wl, struct window_pane *wp)`
pub unsafe fn format_create_defaults(
    item: *mut cmdq_item,
    c: *mut client,
    s: *mut session,
    wl: *mut winlink,
    wp: *mut window_pane,
) -> *mut format_tree {
    unsafe {
        let ft = if !item.is_null() {
            format_create(
                cmdq_get_client(item),
                item,
                FORMAT_NONE,
                format_flags::empty(),
            )
        } else {
            format_create(null_mut(), item, FORMAT_NONE, format_flags::empty())
        };
        format_defaults(ft, c, NonNull::new(s), NonNull::new(wl), NonNull::new(wp));
        ft
    }
}

/// Create and add defaults using state.
/// C `vendor/tmux/format.c:6069`: `struct format_tree *format_create_from_state(struct cmdq_item *item, struct client *c, struct cmd_find_state *fs)`
pub unsafe fn format_create_from_state(
    item: *mut cmdq_item,
    c: *mut client,
    fs: *mut cmd_find_state,
) -> *mut format_tree {
    unsafe { format_create_defaults(item, c, (*fs).s, (*fs).wl, (*fs).wp) }
}

/// Create and add defaults using target.
/// C `vendor/tmux/format.c:6077`: `struct format_tree *format_create_from_target(struct cmdq_item *item)`
pub unsafe fn format_create_from_target(item: *mut cmdq_item) -> *mut format_tree {
    unsafe {
        let tc = cmdq_get_target_client(item);

        format_create_from_state(item, tc, cmdq_get_target(item))
    }
}

/// Set defaults for any of arguments that are not NULL.
/// C `vendor/tmux/format.c:6086`: `void format_defaults(struct format_tree *ft, struct client *c, struct session *s, struct winlink *wl, struct window_pane *wp)`
pub unsafe fn format_defaults(
    ft: *mut format_tree,
    c: *mut client,
    s: Option<NonNull<session>>,
    wl: Option<NonNull<winlink>>,
    wp: Option<NonNull<window_pane>>,
) {
    unsafe {
        let mut s = transmute_ptr(s);
        let mut wl = transmute_ptr(wl);
        let mut wp = transmute_ptr(wp);

        if !c.is_null() && !(*c).name.is_null() {
            log_debug!("{}: c={}", function_name!(), _s((*c).name));
        } else {
            log_debug!("{}: c=none", function_name!());
        }
        if !s.is_null() {
            log_debug!("{}: s=${}", function_name!(), (*s).id);
        } else {
            log_debug!("{}: s=none", function_name!());
        }
        if !wl.is_null() {
            log_debug!("{}: wl={}", function_name!(), (*wl).idx);
        } else {
            log_debug!("{}: wl=none", function_name!());
        }
        if !wp.is_null() {
            log_debug!("{}: wp=%%{}", function_name!(), (*wp).id);
        } else {
            log_debug!("{}: wp=none", function_name!());
        }

        if !c.is_null() && !s.is_null() && (*c).session != s {
            log_debug!("{}: session does not match", function_name!());
        }

        (*ft).type_ = if !wp.is_null() {
            format_type::FORMAT_TYPE_PANE
        } else if !wl.is_null() {
            format_type::FORMAT_TYPE_WINDOW
        } else if !s.is_null() {
            format_type::FORMAT_TYPE_SESSION
        } else {
            format_type::FORMAT_TYPE_UNKNOWN
        };

        if s.is_null() && !c.is_null() {
            s = (*c).session;
        }
        if wl.is_null() && !s.is_null() {
            wl = (*s).curw;
        }
        if wp.is_null() && !wl.is_null() {
            wp = (*(*wl).window).active;
        }

        if !c.is_null() {
            format_defaults_client(ft, c);
        }
        if !s.is_null() {
            format_defaults_session(ft, s);
        }
        if !wl.is_null() {
            format_defaults_winlink(ft, wl);
        }
        if !wp.is_null() {
            format_defaults_pane(ft, wp);
        }

        let pb = paste_get_top(null_mut());
        if !pb.is_null() {
            format_defaults_paste_buffer(ft, pb);
        }
    }
}

/// Set default format keys for a session.
/// C `vendor/tmux/format.c:6143`: `static void format_defaults_session(struct format_tree *ft, struct session *s)`
pub unsafe fn format_defaults_session(ft: *mut format_tree, s: *mut session) {
    unsafe {
        (*ft).s = s;
    }
}

/// Set default format keys for a client.
/// C `vendor/tmux/format.c:6150`: `static void format_defaults_client(struct format_tree *ft, struct client *c)`
pub unsafe fn format_defaults_client(ft: *mut format_tree, c: *mut client) {
    unsafe {
        if (*ft).s.is_null() {
            (*ft).s = (*c).session;
        }
        (*ft).c = c;
    }
}

/// Set default format keys for a window.
/// C `vendor/tmux/format.c:6159`: `void format_defaults_window(struct format_tree *ft, struct window *w)`
pub unsafe fn format_defaults_window(ft: *mut format_tree, w: *mut window) {
    unsafe {
        (*ft).w = w;
    }
}

/// Set default format keys for a winlink.
/// C `vendor/tmux/format.c:6166`: `static void format_defaults_winlink(struct format_tree *ft, struct winlink *wl)`
pub unsafe fn format_defaults_winlink(ft: *mut format_tree, wl: *mut winlink) {
    unsafe {
        if (*ft).w.is_null() {
            format_defaults_window(ft, (*wl).window);
        }
        (*ft).wl = wl;
    }
}

/// Set default format keys for a window pane.
/// C `vendor/tmux/format.c:6175`: `void format_defaults_pane(struct format_tree *ft, struct window_pane *wp)`
pub unsafe fn format_defaults_pane(ft: *mut format_tree, wp: *mut window_pane) {
    unsafe {
        if (*ft).w.is_null() {
            format_defaults_window(ft, (*wp).window);
        }
        (*ft).wp = wp;

        if let Some(wme) = NonNull::new(tailq_first(&raw mut (*wp).modes))
            && let Some(formats) = (*(*wme.as_ptr()).mode).formats
        {
            formats(wme.as_ptr(), ft);
        }
    }
}

/// Set default format keys for paste buffer.
/// C `vendor/tmux/format.c:6190`: `void format_defaults_paste_buffer(struct format_tree *ft, struct paste_buffer *pb)`
pub unsafe fn format_defaults_paste_buffer(ft: *mut format_tree, pb: *mut paste_buffer) {
    unsafe {
        (*ft).pb = pb;
    }
}

/// Return word at given coordinates. Caller frees.
/// C `vendor/tmux/format.c:6207`: `char *format_grid_word(struct grid *gd, u_int x, u_int y)`
pub unsafe fn format_grid_word(gd: *mut grid, mut x: u32, mut y: u32) -> String {
    unsafe {
        let mut ud: Vec<utf8_data> = Vec::new();
        let mut gc = MaybeUninit::<grid_cell>::uninit();
        let gc = gc.as_mut_ptr();
        let mut found = false;

        let ws: *const u8 = options_get_string_(GLOBAL_S_OPTIONS, "word-separators");

        loop {
            grid_get_cell(gd, x, y, gc);
            if (*gc).flags.intersects(grid_flag::PADDING) {
                break;
            }
            if utf8_cstrhas(ws, &raw const (*gc).data)
                || ((*gc).data.size == 1 && (*gc).data.data[0] == b' ')
            {
                found = true;
                break;
            }

            if x == 0 {
                if y == 0 {
                    break;
                }
                let gl = grid_peek_line(gd, y - 1);
                if !(*gl).flags.intersects(grid_line_flag::WRAPPED) {
                    break;
                }
                y -= 1;
                x = grid_line_length(gd, y);
                if x == 0 {
                    break;
                }
            }
            x -= 1;
        }
        loop {
            if found {
                let end = grid_line_length(gd, y);
                if end == 0 || x == end - 1 {
                    if y == (*gd).hsize + (*gd).sy - 1 {
                        break;
                    }
                    let gl = grid_peek_line(gd, y);
                    if !(*gl).flags.intersects(grid_line_flag::WRAPPED) {
                        break;
                    }
                    y += 1;
                    x = 0;
                } else {
                    x += 1;
                }
            }
            found = true;

            grid_get_cell(gd, x, y, gc);
            if (*gc).flags.intersects(grid_flag::PADDING) {
                break;
            }
            if utf8_cstrhas(ws, &raw mut (*gc).data)
                || ((*gc).data.size == 1 && (*gc).data.data[0] == b' ')
            {
                break;
            }

            ud.push((*gc).data);
        }

        utf8_to_string(&ud)
    }
}

/// Return line at given coordinates. Caller frees.
/// C `vendor/tmux/format.c:6276`: `char *format_grid_line(struct grid *gd, u_int y)`
pub unsafe fn format_grid_line(gd: *mut grid, y: u32) -> String {
    unsafe {
        let mut ud: Vec<utf8_data> = Vec::new();
        let mut gc = MaybeUninit::<grid_cell>::uninit();
        let gc = gc.as_mut_ptr();
        for x in 0..grid_line_length(gd, y) {
            grid_get_cell(gd, x, y, gc);
            if (*gc).flags.intersects(grid_flag::PADDING) {
                break;
            }

            ud.push((*gc).data);
        }
        utf8_to_string(&ud)
    }
}

/// Return hyperlink at given coordinates. Caller frees.
/// C `vendor/tmux/format.c:6305`: `char *format_grid_hyperlink(struct grid *gd, u_int x, u_int y, struct screen* s)`
pub unsafe fn format_grid_hyperlink(
    gd: *mut grid,
    x: u32,
    y: u32,
    s: *mut screen,
) -> Option<String> {
    unsafe {
        let mut uri: *const u8 = null();
        let mut gc = MaybeUninit::<grid_cell>::uninit();
        let gc = gc.as_mut_ptr();

        grid_get_cell(gd, x, y, gc);
        if (*gc).flags.intersects(grid_flag::PADDING) {
            return None;
        }
        if (*s).hyperlinks.is_null() || (*gc).link == 0 {
            return None;
        }
        if !hyperlinks_get(
            (*s).hyperlinks,
            (*gc).link,
            &mut uri,
            null_mut(),
            null_mut(),
        ) {
            return None;
        }
        Some(cstr_to_str(uri).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Copy a NUL-terminated C string produced by one of the format helpers into
    // an owned Vec<u8> (excluding the terminator) so it can be compared, then
    // free the heap buffer the helper xmalloc'd.
    unsafe fn take(p: *mut u8) -> Vec<u8> {
        unsafe {
            let v = std::ffi::CStr::from_ptr(p.cast()).to_bytes().to_vec();
            free_(p);
            v
        }
    }

    // format_true: "" and "0" are false; anything else (incl. "00") is true; a
    // NULL pointer is false.
    #[test]
    fn test_format_true() {
        unsafe {
            assert!(!format_true(std::ptr::null()));
            assert!(!format_true(crate::c!("")));
            assert!(!format_true(crate::c!("0")));
            assert!(format_true(crate::c!("1")));
            assert!(format_true(crate::c!("00")));
            assert!(format_true(crate::c!("false")));
        }
    }

    // format_is_end: only ';' and ':' terminate a modifier.
    #[test]
    fn test_format_is_end() {
        assert!(format_is_end(b';'));
        assert!(format_is_end(b':'));
        assert!(!format_is_end(b','));
        assert!(!format_is_end(b'a'));
        assert!(!format_is_end(b'}'));
    }

    // format_quote_shell: backslash-escapes shell metacharacters (space, $, etc.)
    // and leaves ordinary text untouched.
    #[test]
    fn test_format_quote_shell() {
        unsafe {
            assert_eq!(take(format_quote_shell(crate::c!("abc"))), b"abc");
            assert_eq!(take(format_quote_shell(crate::c!("a b"))), b"a\\ b");
            assert_eq!(take(format_quote_shell(crate::c!("a$b"))), b"a\\$b");
        }
    }

    // format_quote_style: doubles '#' (the style escape) and leaves the rest.
    #[test]
    fn test_format_quote_style() {
        unsafe {
            assert_eq!(take(format_quote_style(crate::c!("abc"))), b"abc");
            assert_eq!(take(format_quote_style(crate::c!("a#b"))), b"a##b");
        }
    }

    // format_unescape: an escape '#' before one of ",#{}:" is dropped, leaving
    // the following byte literal; text outside any escape passes through.
    #[test]
    fn test_format_unescape() {
        unsafe {
            let mut es: format_expand_state = zeroed();
            es.start_time = get_timer();
            let esp = &raw mut es;
            assert_eq!(take(format_unescape(esp, crate::c!("#,"))), b",");
            assert_eq!(take(format_unescape(esp, crate::c!("a##b"))), b"a#b");
            assert_eq!(take(format_unescape(esp, crate::c!("plain"))), b"plain");
        }
    }

    // Regression (bug 7, commit 7a3fd1f983): a dropped `s = s.add(1)` on the
    // non-escape branch of format_unescape left the scan pointer stalled, so the
    // loop ran off the end of the xmalloc(strlen+1) buffer and crashed the
    // server on any `#{l:...}` literal. These inputs all drive the non-escape
    // and bracket branches, and must terminate with the expected output rather
    // than overrun.
    #[test]
    fn test_format_unescape_no_overrun() {
        unsafe {
            let mut es: format_expand_state = zeroed();
            es.start_time = get_timer();
            let esp = &raw mut es;
            // A trailing lone '#' (nothing to escape): the `*s.add(1) != 0`
            // guard keeps it literal instead of consuming the NUL terminator.
            assert_eq!(take(format_unescape(esp, crate::c!("a#"))), b"a#");
            assert_eq!(take(format_unescape(esp, crate::c!("#"))), b"#");

            // Inside #{...} the escape is suppressed (brackets != 0), so the
            // group passes through verbatim. Exercises the bracket inc/dec plus
            // the plain-copy branch that once failed to advance.
            assert_eq!(take(format_unescape(esp, crate::c!("#{l:x}"))), b"#{l:x}");
            assert_eq!(take(format_unescape(esp, crate::c!("#{a}#{b}"))), b"#{a}#{b}");

            // A run of ordinary characters must advance one per iteration.
            assert_eq!(
                take(format_unescape(esp, crate::c!("abcdefghij"))),
                b"abcdefghij"
            );

            // Escape immediately followed by another '#' collapses to a single
            // literal '#', then continues scanning.
            assert_eq!(take(format_unescape(esp, crate::c!("##,"))), b"#,");
        }
    }

    // format_strip: removes the escape '#' before ",#{}:" (outside #{...}),
    // keeping the escaped character.
    #[test]
    fn test_format_strip() {
        unsafe {
            let mut es: format_expand_state = zeroed();
            es.start_time = get_timer();
            let esp = &raw mut es;
            assert_eq!(take(format_strip(esp, crate::c!("a#,b"))), b"a,b");
            assert_eq!(take(format_strip(esp, crate::c!("plain"))), b"plain");
        }
    }

    // Regression: format_expand1 must guard the end of string after an
    // unescaped '#', mirroring C format.c:5874 `if (*++fmt == '\0') break;`.
    // A format ending in a lone '#' has nothing after it: C advances past the
    // '#', sees the NUL, and breaks — the trailing '#' is dropped. Without the
    // guard the port read ch=0, wrote '#'+NUL, and ran the outer loop past the
    // terminator. A real format_tree is required because format_expand1 logs
    // through format_log1_, which dereferences es->ft.
    #[test]
    fn test_format_expand_trailing_hash() {
        unsafe {
            let ft = format_create(
                null_mut(),
                null_mut(),
                0,
                format_flags::empty(),
            );

            // Trailing '#' with preceding text: the '#' is dropped.
            assert_eq!(take(format_expand(ft, crate::c!("abc#"))), b"abc");
            // A lone '#' expands to empty (break before anything is written).
            assert_eq!(take(format_expand(ft, crate::c!("#"))), b"");
            // A doubled '#' still escapes to a single literal '#' (control:
            // proves the guard doesn't swallow legitimate escapes).
            assert_eq!(take(format_expand(ft, crate::c!("a##b"))), b"a#b");

            format_free(ft);
        }
    }

    // format_skip: returns a pointer to the first delimiter at bracket depth 0,
    // skipping over #{...} groups; NULL when no delimiter is found.
    #[test]
    fn test_format_skip() {
        unsafe {
            let s = crate::c!("abc,def");
            let r = format_skip(s, crate::c!(","));
            assert!(!r.is_null());
            assert_eq!(r.offset_from(s), 3);
            assert_eq!(*r, b',');

            // No delimiter present -> NULL.
            assert!(format_skip(crate::c!("abc"), crate::c!(",")).is_null());

            // A comma inside #{...} is ignored; the top-level one is found.
            let s2 = crate::c!("a#{x,y}b,c");
            let r2 = format_skip(s2, crate::c!(","));
            assert!(!r2.is_null());
            assert_eq!(*r2, b',');
            assert_eq!(r2.offset_from(s2), 8);
        }
    }

    // format_table_get binary-searches FORMAT_TABLE for a key: known keys
    // resolve to the matching entry, unknown/empty keys return None.
    #[test]
    fn test_format_table_get() {
        unsafe {
            let hit = format_table_get(crate::c!("window_width"));
            assert!(hit.is_some());
            assert_eq!(hit.unwrap().key, "window_width");

            // First and last entries are reachable (exercises both search ends).
            assert_eq!(
                format_table_get(crate::c!("active_window_index"))
                    .unwrap()
                    .key,
                "active_window_index"
            );
            assert_eq!(
                format_table_get(crate::c!("wrap_flag")).unwrap().key,
                "wrap_flag"
            );

            // Misses return None.
            assert!(format_table_get(crate::c!("not_a_real_format")).is_none());
            assert!(format_table_get(crate::c!("")).is_none());
            // A prefix of a real key is still a miss (exact match required).
            assert!(format_table_get(crate::c!("window_widt")).is_none());
        }
    }

    // format_table_get relies on binary_search, which is only correct when
    // FORMAT_TABLE is sorted strictly ascending by key. A misplaced or duplicate
    // entry would make some lookups silently return None; this locks the
    // invariant so a new entry added out of order fails loudly.
    #[test]
    fn test_format_table_is_sorted() {
        for pair in FORMAT_TABLE.windows(2) {
            assert!(
                pair[0].key < pair[1].key,
                "FORMAT_TABLE not sorted: {:?} !< {:?}",
                pair[0].key,
                pair[1].key
            );
        }
    }

    // format_true (format.c:4403): true iff the string is non-empty and not the
    // single character '0'. "00" and " " are true; a lone "0" is false.
    #[test]
    fn test_format_true_edge_cases() {
        unsafe {
            assert!(!format_true(crate::c!("0")));
            assert!(format_true(crate::c!("00")));
            assert!(format_true(crate::c!("01")));
            assert!(format_true(crate::c!("0x"))); // '0' followed by more -> true
            assert!(format_true(crate::c!(" "))); // a space is not "0" -> true
            assert!(format_true(crate::c!("-")));
        }
    }

    // format_is_end (format.c:4412): only ';' and ':' end a modifier; the other
    // structural characters do not.
    #[test]
    fn test_format_is_end_all_bytes() {
        assert!(format_is_end(b';'));
        assert!(format_is_end(b':'));
        for c in [b'{', b'}', b'#', b',', b'|', b' ', b'\0', b'0', b'A'] {
            assert!(!format_is_end(c), "byte {c:#x} should not be an end");
        }
    }

    // format_skip: the ':' delimiter is honoured just like ',', returning a
    // pointer to the first top-level occurrence (format.c:4370).
    #[test]
    fn test_format_skip_colon_delimiter() {
        unsafe {
            let s = crate::c!("abc:def");
            let r = format_skip(s, crate::c!(":"));
            assert!(!r.is_null());
            assert_eq!(r.offset_from(s), 3);
            assert_eq!(*r, b':');
        }
    }

    // format_skip: an escaped delimiter ("#," / "#:" etc.) is consumed by the
    // two-byte escape skip and does not terminate the scan; the first
    // *unescaped* delimiter does (format.c:4340 escape branch).
    #[test]
    fn test_format_skip_escaped_delimiter() {
        unsafe {
            // The "#," at offset 1 is an escape and is skipped; the real comma
            // is at offset 4.
            let s = crate::c!("a#,b,c");
            let r = format_skip(s, crate::c!(","));
            assert!(!r.is_null());
            assert_eq!(r.offset_from(s), 4);
            assert_eq!(*r, b',');
        }
    }

    // format_skip: brackets nest, so a delimiter is only recognised at depth 0.
    // Here the comma inside two levels of #{...} is ignored.
    #[test]
    fn test_format_skip_nested_brackets() {
        unsafe {
            let s = crate::c!("#{a#{b,c}d},e");
            let r = format_skip(s, crate::c!(","));
            assert!(!r.is_null());
            // The only top-level comma is the one right before 'e'.
            assert_eq!(*r, b',');
            assert_eq!(*r.add(1), b'e');
        }
    }

    // format_choose splits a "left,right" string at the first top-level comma.
    // With es = NULL and expand = 0 it performs no expansion, so the two halves
    // are copied verbatim (format.c:4377).
    #[test]
    fn test_format_choose_basic() {
        unsafe {
            let mut left: *mut u8 = std::ptr::null_mut();
            let mut right: *mut u8 = std::ptr::null_mut();
            let rc = format_choose(
                std::ptr::null_mut(),
                crate::c!("abc,def"),
                &raw mut left,
                &raw mut right,
                0,
            );
            assert_eq!(rc, 0);
            assert_eq!(take(left), b"abc");
            assert_eq!(take(right), b"def");
        }
    }

    // format_choose returns -1 when there is no top-level comma to split on.
    #[test]
    fn test_format_choose_no_comma() {
        unsafe {
            let mut left: *mut u8 = std::ptr::null_mut();
            let mut right: *mut u8 = std::ptr::null_mut();
            let rc = format_choose(
                std::ptr::null_mut(),
                crate::c!("abcdef"),
                &raw mut left,
                &raw mut right,
                0,
            );
            assert_eq!(rc, -1);
        }
    }

    // format_choose ignores an escaped comma and a comma inside #{...} when
    // finding the split point; the right side keeps everything after the first
    // real comma.
    #[test]
    fn test_format_choose_escaped_and_nested() {
        unsafe {
            let mut left: *mut u8 = std::ptr::null_mut();
            let mut right: *mut u8 = std::ptr::null_mut();
            // "a#,b" has an escaped comma; the real split is before 'c'.
            let rc = format_choose(
                std::ptr::null_mut(),
                crate::c!("a#,b,c"),
                &raw mut left,
                &raw mut right,
                0,
            );
            assert_eq!(rc, 0);
            assert_eq!(take(left), b"a#,b");
            assert_eq!(take(right), b"c");
        }
    }

    // format_quote_shell backslash-escapes each character in the set
    // "|&;<>()$`\\\"'*?[# =%" (format.c:4013) and copies everything else.
    #[test]
    fn test_format_quote_shell_metachars() {
        unsafe {
            assert_eq!(take(format_quote_shell(crate::c!("a=b"))), b"a\\=b");
            assert_eq!(take(format_quote_shell(crate::c!("50%"))), b"50\\%");
            assert_eq!(take(format_quote_shell(crate::c!("a*b?"))), b"a\\*b\\?");
            assert_eq!(take(format_quote_shell(crate::c!("x[0]"))), b"x\\[0]"); // ']' not in set
            assert_eq!(take(format_quote_shell(crate::c!("a#b"))), b"a\\#b");
            // A backslash and a backtick are both escaped.
            assert_eq!(take(format_quote_shell(crate::c!("a\\b"))), b"a\\\\b");
            assert_eq!(take(format_quote_shell(crate::c!("a`b"))), b"a\\`b");
        }
    }

    // format_quote_style doubles every '#' and leaves the rest untouched
    // (format.c:4032). Runs of hashes double individually.
    #[test]
    fn test_format_quote_style_runs() {
        unsafe {
            assert_eq!(take(format_quote_style(crate::c!("###"))), b"######");
            assert_eq!(take(format_quote_style(crate::c!("a#b#c"))), b"a##b##c");
            assert_eq!(take(format_quote_style(crate::c!("plain"))), b"plain");
        }
    }

    // format_unescape drops the escaping '#' before ",#}:" when outside a group,
    // leaving the literal following byte. A "#{" is special: it is treated as a
    // group opener (brackets++) BEFORE the escape branch, so the escape is
    // suppressed and "#{" passes through verbatim (format.c:4287, the
    // `brackets == 0` guard).
    #[test]
    fn test_format_unescape_all_escapes() {
        unsafe {
            let mut es: format_expand_state = zeroed();
            es.start_time = get_timer();
            let esp = &raw mut es;
            assert_eq!(take(format_unescape(esp, crate::c!("#:"))), b":");
            // "#{" opens a group, so it is not unescaped — stays "#{".
            assert_eq!(take(format_unescape(esp, crate::c!("#{"))), b"#{");
            // "#}" is a plain escape (next byte is '}', not '{'), so -> "}".
            assert_eq!(take(format_unescape(esp, crate::c!("#}"))), b"}");
            assert_eq!(take(format_unescape(esp, crate::c!("a#,b#:c"))), b"a,b:c");
        }
    }

    // format_strip removes the escape '#' before ",#{}:" outside a group but
    // preserves an entire #{...} group verbatim (format.c strip rules).
    #[test]
    fn test_format_strip_group_preserved() {
        unsafe {
            let mut es: format_expand_state = zeroed();
            es.start_time = get_timer();
            let esp = &raw mut es;
            assert_eq!(take(format_strip(esp, crate::c!("a#:b"))), b"a:b");
            assert_eq!(take(format_strip(esp, crate::c!("a#{b#}c"))), b"a#{b#}c");
            assert_eq!(take(format_strip(esp, crate::c!("no#,escapes"))), b"no,escapes");
        }
    }

    // format_table_get: reachable known keys resolve to their entry; the lookup
    // is exact so trailing/leading noise misses (format.c binary search).
    #[test]
    fn test_format_table_get_more() {
        unsafe {
            assert_eq!(
                format_table_get(crate::c!("host")).unwrap().key,
                "host"
            );
            assert_eq!(
                format_table_get(crate::c!("pane_id")).unwrap().key,
                "pane_id"
            );
            // Case matters (keys are lowercase) and a trailing space misses.
            assert!(format_table_get(crate::c!("Host")).is_none());
            assert!(format_table_get(crate::c!("host ")).is_none());
        }
    }

    // format_skip: a comma inside a #{...} group is protected. The `#{` both
    // increments brackets (format.c:4350) and is consumed as an escaped '{'
    // (format.c:4352), while the closing '}' decrements (format.c:4358), so the
    // group nets depth 0 and the top-level comma after it is the hit.
    #[test]
    fn test_format_skip_group_protects_comma() {
        unsafe {
            let s = crate::c!("a#{b}c,d");
            let r = format_skip(s, crate::c!(","));
            assert!(!r.is_null());
            assert_eq!(r.offset_from(s), 6);
            assert_eq!(*r, b',');
            assert_eq!(*r.add(1), b'd');
        }
    }

    // format_skip: a lone '}' with no matching '#{' drives brackets negative
    // (format.c:4358 `if (*s == '}') brackets--;`), so the subsequent
    // `brackets == 0` guard (format.c:4360) is false and the '}' is NOT treated
    // as the delimiter. With no other match the scan runs to NUL and returns
    // NULL (format.c:4363).
    #[test]
    fn test_format_skip_unbalanced_close_returns_null() {
        unsafe {
            assert!(format_skip(crate::c!("ab}cd"), crate::c!("}")).is_null());
        }
    }

    // format_skip: the `end` argument is a *set* of delimiter bytes; the first
    // byte at depth 0 matching any of them stops the scan (format.c:4360
    // `strchr(end, *s)`).
    #[test]
    fn test_format_skip_multi_char_end_set() {
        unsafe {
            // ':' comes before ',' here and both are in the set -> ':' wins.
            let s = crate::c!("abc:de,f");
            let r = format_skip(s, crate::c!(",:"));
            assert!(!r.is_null());
            assert_eq!(r.offset_from(s), 3);
            assert_eq!(*r, b':');
        }
    }

    // format_choose with expand=1 runs each half through format_expand1
    // (format.c:4389). A real format_tree is required because format_expand1
    // logs through es->ft. Pure-literal halves expand to themselves.
    #[test]
    fn test_format_choose_expand_literals() {
        unsafe {
            let ft = format_create(null_mut(), null_mut(), 0, format_flags::empty());
            let mut es: format_expand_state = zeroed();
            es.ft = ft;
            es.flags = format_expand_flags::empty();
            es.start_time = get_timer();
            let esp = &raw mut es;

            let mut left: *mut u8 = std::ptr::null_mut();
            let mut right: *mut u8 = std::ptr::null_mut();
            let rc = format_choose(esp, crate::c!("abc,def"), &raw mut left, &raw mut right, 1);
            assert_eq!(rc, 0);
            assert_eq!(take(left), b"abc");
            assert_eq!(take(right), b"def");

            format_free(ft);
        }
    }

    // format_choose with expand=1: the left half "a##b" is a doubled-hash
    // escape, so format_expand1 collapses it to "a#b" (format.c:4390). Proves
    // the split point (first top-level comma) is found past the escaped '##'
    // and that each half is genuinely expanded, not copied verbatim.
    #[test]
    fn test_format_choose_expand_unescapes() {
        unsafe {
            let ft = format_create(null_mut(), null_mut(), 0, format_flags::empty());
            let mut es: format_expand_state = zeroed();
            es.ft = ft;
            es.flags = format_expand_flags::empty();
            es.start_time = get_timer();
            let esp = &raw mut es;

            let mut left: *mut u8 = std::ptr::null_mut();
            let mut right: *mut u8 = std::ptr::null_mut();
            let rc = format_choose(esp, crate::c!("a##b,c"), &raw mut left, &raw mut right, 1);
            assert_eq!(rc, 0);
            assert_eq!(take(left), b"a#b");
            assert_eq!(take(right), b"c");

            format_free(ft);
        }
    }

    // format_choose with expand=1 still returns -1 when there is no top-level
    // comma (format.c:4384), leaving the out-pointers untouched.
    #[test]
    fn test_format_choose_expand_no_comma() {
        unsafe {
            let ft = format_create(null_mut(), null_mut(), 0, format_flags::empty());
            let mut es: format_expand_state = zeroed();
            es.ft = ft;
            es.start_time = get_timer();
            let esp = &raw mut es;

            let mut left: *mut u8 = std::ptr::null_mut();
            let mut right: *mut u8 = std::ptr::null_mut();
            let rc = format_choose(esp, crate::c!("noComma"), &raw mut left, &raw mut right, 1);
            assert_eq!(rc, -1);

            format_free(ft);
        }
    }

    // format_quote_shell escapes every byte in "|&;<>()$`\\\"'*?[# =%"
    // (format.c:4015). Shell operators and both quote characters are backslash
    // -prefixed.
    #[test]
    fn test_format_quote_shell_shell_operators() {
        unsafe {
            assert_eq!(take(format_quote_shell(crate::c!("a|b"))), b"a\\|b");
            assert_eq!(take(format_quote_shell(crate::c!("a&b"))), b"a\\&b");
            assert_eq!(take(format_quote_shell(crate::c!("a;b"))), b"a\\;b");
            assert_eq!(take(format_quote_shell(crate::c!("a<b>c"))), b"a\\<b\\>c");
            assert_eq!(take(format_quote_shell(crate::c!("(x)"))), b"\\(x\\)");
            assert_eq!(take(format_quote_shell(crate::c!("a'b\"c"))), b"a\\'b\\\"c");
        }
    }

    // format_quote_shell leaves bytes NOT in the set untouched: closing bracket
    // ']', braces, slash, colon, at, tilde, plus and minus are all copied
    // verbatim (format.c:4015 — none are in the metachar string).
    #[test]
    fn test_format_quote_shell_non_metachars() {
        unsafe {
            assert_eq!(take(format_quote_shell(crate::c!("]}{/@:~+-"))), b"]}{/@:~+-");
        }
    }

    // format_quote_shell on an empty string returns an empty string
    // (format.c:4015 — the loop body never runs).
    #[test]
    fn test_format_quote_shell_empty() {
        unsafe {
            assert_eq!(take(format_quote_shell(crate::c!(""))), b"");
        }
    }

    // format_quote_style on an empty string returns an empty string
    // (format.c:4032).
    #[test]
    fn test_format_quote_style_empty() {
        unsafe {
            assert_eq!(take(format_quote_style(crate::c!(""))), b"");
        }
    }

    // format_unescape: a balanced #{...} group is copied verbatim because the
    // opening `#{` sets brackets>0, suppressing the escape branch inside, and
    // the closing '}' restores depth (format.c:4294/4303). Nested groups keep
    // their inner braces.
    #[test]
    fn test_format_unescape_nested_group_verbatim() {
        unsafe {
            let mut es: format_expand_state = zeroed();
            es.start_time = get_timer();
            let esp = &raw mut es;
            assert_eq!(
                take(format_unescape(esp, crate::c!("#{a#{b}c}"))),
                b"#{a#{b}c}"
            );
        }
    }

    // format_strip: inside a group (brackets != 0) an escape '#' before ",#{}:"
    // is KEPT rather than dropped (format.c:4327-4330 `if (brackets != 0)
    // *cp++ = *s;`), so the whole group survives intact.
    #[test]
    fn test_format_strip_nested_group_inner_escape() {
        unsafe {
            let mut es: format_expand_state = zeroed();
            es.start_time = get_timer();
            let esp = &raw mut es;
            assert_eq!(
                take(format_strip(esp, crate::c!("#{a#,b}"))),
                b"#{a#,b}"
            );
        }
    }

    // format_strip: outside any group a "##" escape collapses to a single '#'
    // (format.c:4327 drops the escaping '#', the following '#' is then copied
    // when its own successor is not in the set).
    #[test]
    fn test_format_strip_double_hash() {
        unsafe {
            let mut es: format_expand_state = zeroed();
            es.start_time = get_timer();
            let esp = &raw mut es;
            assert_eq!(take(format_strip(esp, crate::c!("a##b"))), b"a#b");
        }
    }

    // format_strip removes each escaping '#' before a member of ",#{}:"
    // (format.c:4327). A run of distinct escapes all collapse.
    #[test]
    fn test_format_strip_multiple_escapes() {
        unsafe {
            let mut es: format_expand_state = zeroed();
            es.start_time = get_timer();
            let esp = &raw mut es;
            assert_eq!(
                take(format_strip(esp, crate::c!("a#:b#,c#}d"))),
                b"a:b,c}d"
            );
        }
    }

    // format_table_get binary search resolves a key that sits mid-table, and a
    // string that would sort between two real keys is a clean miss
    // (format.c:3803 bsearch requires an exact compare hit).
    #[test]
    fn test_format_table_get_midtable() {
        unsafe {
            assert_eq!(
                format_table_get(crate::c!("session_name")).unwrap().key,
                "session_name"
            );
            assert_eq!(
                format_table_get(crate::c!("client_name")).unwrap().key,
                "client_name"
            );
            // Lexically valid but non-existent -> None.
            assert!(format_table_get(crate::c!("session_zzz")).is_none());
        }
    }

    // format_width counts display columns of plain ASCII one per byte and
    // returns 0 for the empty string (format-draw.c:1100).
    #[test]
    fn test_format_width_ascii_and_empty() {
        unsafe {
            assert_eq!(format_width("hello"), 5);
            assert_eq!(format_width(""), 0);
            assert_eq!(format_width("a b c"), 5);
        }
    }

    // format_width uses format_leading_hashes for '#' runs (format-draw.c:1110):
    // an even run "##" is a fully-escaped single '#' of width 1, and an odd run
    // "###" is width (n/2)+1 = 2 (format-draw.c:648-661).
    #[test]
    fn test_format_width_escaped_hashes() {
        unsafe {
            assert_eq!(format_width("a##b"), 3);
            assert_eq!(format_width("###"), 2);
        }
    }

    // format_width skips a style range "#[...]" entirely: leading_hashes marks
    // it a style, then format_skip advances past the ']' (format-draw.c:1113).
    // Only the trailing literal text contributes width.
    #[test]
    fn test_format_width_style_range_skipped() {
        unsafe {
            assert_eq!(format_width("#[fg=red]abc"), 3);
        }
    }

    // format_width returns 0 for an unterminated style: format_skip cannot find
    // the ']' and returns NULL, which format_width propagates as 0
    // (format-draw.c:1114-1116 `if (end == NULL) return (0);`).
    #[test]
    fn test_format_width_unterminated_style_zero() {
        unsafe {
            assert_eq!(format_width("#[fg=red"), 0);
        }
    }

    // format_trim_left keeps the leftmost `limit` display columns
    // (format-draw.c:1139). Plain ASCII truncates one byte per column.
    #[test]
    fn test_format_trim_left_ascii() {
        unsafe {
            assert_eq!(take(format_trim_left(crate::c!("abcdef"), 3)), b"abc");
        }
    }

    // format_trim_left with limit 0 stops before copying anything
    // (format-draw.c:1148 `if (width >= limit) break;`), and a limit at or
    // beyond the width copies the whole string.
    #[test]
    fn test_format_trim_left_zero_and_over_limit() {
        unsafe {
            assert_eq!(take(format_trim_left(crate::c!("abcdef"), 0)), b"");
            assert_eq!(take(format_trim_left(crate::c!("ab"), 5)), b"ab");
        }
    }

    // format_trim_right keeps the rightmost `limit` columns by skipping the
    // leading (total-limit) columns (format-draw.c:1200).
    #[test]
    fn test_format_trim_right_ascii() {
        unsafe {
            assert_eq!(take(format_trim_right(crate::c!("abcdef"), 3)), b"def");
        }
    }

    // format_trim_right short-circuits when the total width already fits the
    // limit, returning an unmodified copy (format-draw.c:1209-1210
    // `if (total_width <= limit) return (xstrdup(expanded));`).
    #[test]
    fn test_format_trim_right_within_limit_unchanged() {
        unsafe {
            assert_eq!(take(format_trim_right(crate::c!("abc"), 5)), b"abc");
        }
    }

    // Regression: format_trim_right must not panic on invalid UTF-8. The port
    // computes total_width via cstr_to_str_(); on invalid UTF-8 that returns
    // None and the code falls back to the byte length (strlen) instead of
    // aborting in the str conversion. Here strlen(2) <= limit(5), so the
    // early-return path yields the bytes unchanged including the stray 0xff.
    #[test]
    fn test_format_trim_right_invalid_utf8_fallback() {
        unsafe {
            let raw: [u8; 3] = [0xff, b'a', 0];
            let out = format_trim_right(raw.as_ptr(), 5);
            assert_eq!(take(out), vec![0xffu8, b'a']);
        }
    }

    // format_expand end-to-end: the `l` modifier marks a literal string, which
    // is returned via format_unescape with no variable lookup
    // (format.c:5203 FORMAT_LITERAL). A bare test tree has no GLOBAL_OPTIONS, so
    // literal-only formats are the safe end-to-end surface.
    #[test]
    fn test_format_expand_literal_modifier() {
        unsafe {
            let ft = format_create(null_mut(), null_mut(), 0, format_flags::empty());
            assert_eq!(take(format_expand(ft, crate::c!("#{l:abc}"))), b"abc");
            format_free(ft);
        }
    }

    // format_expand: the literal modifier runs its argument through
    // format_unescape, so "#," and "##" inside `#{l:...}` yield the unescaped
    // bytes (format.c FORMAT_LITERAL -> format_unescape).
    #[test]
    fn test_format_expand_literal_unescapes() {
        unsafe {
            let ft = format_create(null_mut(), null_mut(), 0, format_flags::empty());
            assert_eq!(take(format_expand(ft, crate::c!("#{l:a#,b}"))), b"a,b");
            assert_eq!(take(format_expand(ft, crate::c!("#{l:a##b}"))), b"a#b");
            format_free(ft);
        }
    }

    // format_expand: the `=N` modifier truncates the (recursively expanded)
    // value to N columns from the left via format_trim_left (format.c:5272).
    // The value is itself a literal to avoid any GLOBAL_OPTIONS lookup.
    #[test]
    fn test_format_expand_limit_left_modifier() {
        unsafe {
            let ft = format_create(null_mut(), null_mut(), 0, format_flags::empty());
            assert_eq!(take(format_expand(ft, crate::c!("#{=3:#{l:abcdef}}"))), b"abc");
            format_free(ft);
        }
    }

    // format_expand: a negative `=-N` limit truncates from the right via
    // format_trim_right (format.c:5288), keeping the last N columns.
    #[test]
    fn test_format_expand_limit_right_modifier() {
        unsafe {
            let ft = format_create(null_mut(), null_mut(), 0, format_flags::empty());
            assert_eq!(take(format_expand(ft, crate::c!("#{=-3:#{l:abcdef}}"))), b"def");
            format_free(ft);
        }
    }

    // format_expand: the `pN` modifier pads the value to N columns with trailing
    // spaces via utf8_padcstr (format.c:5307, utf8.c:940-style left-justify).
    #[test]
    fn test_format_expand_pad_modifier() {
        unsafe {
            let ft = format_create(null_mut(), null_mut(), 0, format_flags::empty());
            assert_eq!(take(format_expand(ft, crate::c!("#{p6:#{l:ab}}"))), b"ab    ");
            format_free(ft);
        }
    }

    // format_expand: the `n` modifier replaces the value with its byte length
    // (format.c:5332 FORMAT_LENGTH -> strlen).
    #[test]
    fn test_format_expand_length_modifier() {
        unsafe {
            let ft = format_create(null_mut(), null_mut(), 0, format_flags::empty());
            assert_eq!(take(format_expand(ft, crate::c!("#{n:#{l:abcde}}"))), b"5");
            format_free(ft);
        }
    }

    // format_expand: the `w` modifier replaces the value with its display width
    // (format.c:5338 FORMAT_WIDTH -> format_width).
    #[test]
    fn test_format_expand_width_modifier() {
        unsafe {
            let ft = format_create(null_mut(), null_mut(), 0, format_flags::empty());
            assert_eq!(take(format_expand(ft, crate::c!("#{w:#{l:abc}}"))), b"3");
            format_free(ft);
        }
    }

    // format_expand: the `s/a/b/` modifier substitutes ALL matches (regsub is
    // global) in the expanded value (format.c:5253 -> format_sub -> regsub).
    // "banana" with a->b becomes "bbnbnb".
    #[test]
    fn test_format_expand_substitute_modifier() {
        unsafe {
            let ft = format_create(null_mut(), null_mut(), 0, format_flags::empty());
            assert_eq!(
                take(format_expand(ft, crate::c!("#{s/a/b/:#{l:banana}}"))),
                b"bbnbnb"
            );
            format_free(ft);
        }
    }

    // format_expand: outside #{...}, a top-level "#," and "#}" are structural
    // escapes handled directly in format_expand1's switch (format.c:5565 arm
    // for ',' and '}'), emitting the bare delimiter byte.
    #[test]
    fn test_format_expand_toplevel_escapes() {
        unsafe {
            let ft = format_create(null_mut(), null_mut(), 0, format_flags::empty());
            assert_eq!(take(format_expand(ft, crate::c!("a#,b"))), b"a,b");
            assert_eq!(take(format_expand(ft, crate::c!("a#}b"))), b"a}b");
            format_free(ft);
        }
    }
}
