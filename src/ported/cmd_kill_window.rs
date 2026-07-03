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

use crate::compat::tree::{rb_foreach, rb_next, rb_prev};
use crate::*;

pub static CMD_KILL_WINDOW_ENTRY: cmd_entry = cmd_entry {
    name: "kill-window",
    alias: Some("killw"),

    args: args_parse::new("af:t:", 0, 0, None),
    usage: "[-a] [-f filter] [-t target-window]",

    target: cmd_entry_flag::new(
        b't',
        cmd_find_type::CMD_FIND_WINDOW,
        cmd_find_flags::empty(),
    ),

    flags: cmd_flag::empty(),
    exec: cmd_kill_window_exec,
    source: cmd_entry_flag::zeroed(),
};

pub static CMD_UNLINK_WINDOW_ENTRY: cmd_entry = cmd_entry {
    name: "unlink-window",
    alias: Some("unlinkw"),

    args: args_parse::new("kt:", 0, 0, None),
    usage: "[-k] [-t target-window]",

    target: cmd_entry_flag::new(
        b't',
        cmd_find_type::CMD_FIND_WINDOW,
        cmd_find_flags::empty(),
    ),

    flags: cmd_flag::empty(),
    exec: cmd_kill_window_exec,
    source: cmd_entry_flag::zeroed(),
};

/// C `vendor/tmux/cmd-kill-window.c:61`: `static enum cmd_retval cmd_kill_window_exec(struct cmd *self, struct cmdq_item *item)`
/// Whether window `wl` passes the `-f` filter format (true when no filter).
/// C `vendor/tmux/cmd-kill-window.c`: `static int cmd_kill_window_filter(struct cmdq_item *item, struct session *s, struct winlink *wl, const char *filter)`
unsafe fn cmd_kill_window_filter(
    item: *mut cmdq_item,
    s: *mut session,
    wl: *mut winlink,
    filter: *const u8,
) -> bool {
    unsafe {
        if filter.is_null() {
            return true;
        }
        let ft = format_create(cmdq_get_client(item), item, FORMAT_NONE, format_flags::empty());
        format_defaults(ft, null_mut(), NonNull::new(s), NonNull::new(wl), None);
        let expanded = format_expand(ft, filter);
        let flag = format_true(expanded);
        free_(expanded);
        format_free(ft);
        flag
    }
}

unsafe fn cmd_kill_window_exec(self_: *mut cmd, item: *mut cmdq_item) -> cmd_retval {
    unsafe {
        let args = cmd_get_args(self_);
        let target = cmdq_get_target(item);
        let wl = (*target).wl;
        //*loop;
        let w = (*wl).window;
        let s = (*target).s;
        let mut found;

        if std::ptr::eq(cmd_get_entry(self_), &CMD_UNLINK_WINDOW_ENTRY) {
            if !args_has(args, 'k') && !session_is_linked(s, w) {
                cmdq_error!(item, "window only linked to one session");
                return cmd_retval::CMD_RETURN_ERROR;
            }
            server_unlink_window(s, wl);
            recalculate_sizes();
            return cmd_retval::CMD_RETURN_NORMAL;
        }

        // -f only filters the -a batch (C cmd-kill-window.c).
        let filter = args_get(args, b'f');
        if !filter.is_null() && !args_has(args, 'a') {
            cmdq_error!(item, "-f only valid with -a");
            return cmd_retval::CMD_RETURN_ERROR;
        }

        if args_has(args, 'a') {
            if rb_prev(wl).is_null() && rb_next(wl).is_null() {
                return cmd_retval::CMD_RETURN_NORMAL;
            }

            // Kill all windows except the current one (that pass the filter).
            loop {
                found = 0;
                for loop_ in rb_foreach(&raw mut (*s).windows).map(NonNull::as_ptr) {
                    if (*loop_).window != (*wl).window
                        && cmd_kill_window_filter(item, s, loop_, filter)
                    {
                        server_kill_window((*loop_).window, 0);
                        found += 1;
                        break;
                    }
                }

                if found == 0 {
                    break;
                }
            }

            // If the current window appears in the session more than once, kill
            // it as well if it matches the filter.
            found = 0;
            let mut kill_current = false;
            for loop_ in rb_foreach(&raw mut (*s).windows).map(NonNull::as_ptr) {
                if (*loop_).window == (*wl).window {
                    found += 1;
                    if cmd_kill_window_filter(item, s, loop_, filter) {
                        kill_current = true;
                    }
                }
            }
            if kill_current && found > 1 {
                server_kill_window((*wl).window, 0);
            }

            server_renumber_all();
            return cmd_retval::CMD_RETURN_NORMAL;
        }

        server_kill_window((*wl).window, 1);
        cmd_retval::CMD_RETURN_NORMAL
    }
}
