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
use crate::compat::tree::rb_foreach;
use crate::*;

pub static CMD_KILL_SESSION_ENTRY: cmd_entry = cmd_entry {
    name: "kill-session",
    alias: None,

    args: args_parse::new("aCgf:t:", 0, 0, None),
    usage: "[-aCg] [-f filter] [-t target-session]",

    target: cmd_entry_flag::new(
        b't',
        cmd_find_type::CMD_FIND_SESSION,
        cmd_find_flags::empty(),
    ),
    source: cmd_entry_flag::zeroed(),

    flags: cmd_flag::empty(),
    exec: cmd_kill_session_exec,
};

/// Whether session `s` passes the `-f` filter format (true when no filter).
/// C `vendor/tmux/cmd-kill-session.c`: `static int cmd_kill_session_filter(struct cmdq_item *item, struct session *s, const char *filter)`
unsafe fn cmd_kill_session_filter(item: *mut cmdq_item, s: *mut session, filter: *const u8) -> bool {
    unsafe {
        if filter.is_null() {
            return true;
        }
        let ft = format_create(cmdq_get_client(item), item, FORMAT_NONE, format_flags::empty());
        format_defaults(ft, null_mut(), NonNull::new(s), None, None);
        let expanded = format_expand(ft, filter);
        let flag = format_true(expanded);
        free_(expanded);
        format_free(ft);
        flag
    }
}

/// C `vendor/tmux/cmd-kill-session.c:51`: `static enum cmd_retval cmd_kill_session_exec(struct cmd *self, struct cmdq_item *item)`
unsafe fn cmd_kill_session_exec(self_: *mut cmd, item: *mut cmdq_item) -> cmd_retval {
    unsafe {
        let args = cmd_get_args(self_);
        let target = cmdq_get_target(item);
        let s = (*target).s;
        let filter = args_get(args, b'f');

        // -f only filters the -a batch (C cmd-kill-session.c:60-63).
        if !filter.is_null() && (!args_has(args, 'a') || args_has(args, 'C')) {
            cmdq_error!(item, "-f only valid with -a");
            return cmd_retval::CMD_RETURN_ERROR;
        }

        if args_has(args, 'C') {
            for wl in rb_foreach(&raw mut (*s).windows).map(NonNull::as_ptr) {
                (*(*wl).window).flags &= !WINDOW_ALERTFLAGS;
                (*wl).flags &= !WINLINK_ALERTFLAGS;
            }
            server_redraw_session(s);
        } else if args_has(args, 'a') {
            for sloop in rb_foreach(&raw mut SESSIONS).map(NonNull::as_ptr) {
                if sloop == s || !cmd_kill_session_filter(item, sloop, filter) {
                    continue;
                }
                server_destroy_session(sloop);
                session_destroy(sloop, 1, c!("cmd_kill_session_exec"));
            }
        } else if args_has(args, 'g')
            && let sg = session_group_contains(s)
            && !sg.is_null()
        {
            // Collect first: session_destroy removes the session from the group
            // (C uses TAILQ_FOREACH_SAFE).
            let group: Vec<*mut session> = tailq_foreach(&raw mut (*sg).sessions)
                .map(NonNull::as_ptr)
                .collect();
            for sloop in group {
                server_destroy_session(sloop);
                session_destroy(sloop, 1, c!("cmd_kill_session_exec"));
            }
        } else {
            server_destroy_session(s);
            session_destroy(s, 1, c!("cmd_kill_session_exec"));
        }
        cmd_retval::CMD_RETURN_NORMAL
    }
}
