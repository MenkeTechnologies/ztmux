// Copyright (c) 2009 Nicholas Marriott <nicholas.marriott@gmail.com>
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

use crate::*;

pub static CMD_KILL_PANE_ENTRY: cmd_entry = cmd_entry {
    name: "kill-pane",
    alias: Some("killp"),

    args: args_parse::new("af:t:", 0, 0, None),
    usage: "[-a] [-f filter] [-t target-pane]",

    target: cmd_entry_flag::new(b't', cmd_find_type::CMD_FIND_PANE, cmd_find_flags::empty()),

    flags: cmd_flag::CMD_AFTERHOOK,
    exec: cmd_kill_pane_exec,
    source: cmd_entry_flag::zeroed(),
};

/// Whether pane `wp` passes the `-f` filter format (true when no filter).
/// C `vendor/tmux/cmd-kill-pane.c`: `static int cmd_kill_pane_filter(struct cmdq_item *item, struct session *s, struct winlink *wl, struct window_pane *wp, const char *filter)`
unsafe fn cmd_kill_pane_filter(
    item: *mut cmdq_item,
    s: *mut session,
    wl: *mut winlink,
    wp: *mut window_pane,
    filter: *const u8,
) -> bool {
    unsafe {
        if filter.is_null() {
            return true;
        }
        let ft = format_create(cmdq_get_client(item), item, FORMAT_NONE, format_flags::empty());
        format_defaults(ft, null_mut(), NonNull::new(s), NonNull::new(wl), NonNull::new(wp));
        let expanded = format_expand(ft, filter);
        let flag = format_true(expanded);
        free_(expanded);
        format_free(ft);
        flag
    }
}

/// C `vendor/tmux/cmd-kill-pane.c:49`: `static enum cmd_retval cmd_kill_pane_exec(struct cmd *self, struct cmdq_item *item)`
unsafe fn cmd_kill_pane_exec(self_: *mut cmd, item: *mut cmdq_item) -> cmd_retval {
    unsafe {
        let args = cmd_get_args(self_);
        let target = cmdq_get_target(item);
        let s = (*target).s;
        let wl = (*target).wl;
        let wp = (*target).wp;
        let filter = args_get(args, b'f');

        // -f only filters the -a batch (C cmd-kill-pane.c).
        if !filter.is_null() && !args_has(args, 'a') {
            cmdq_error!(item, "-f only valid with -a");
            return cmd_retval::CMD_RETURN_ERROR;
        }

        if args_has(args, 'a') {
            server_unzoom_window((*wl).window);
            for loopwp in
                tailq_foreach::<_, discr_entry>(&raw mut (*(*wl).window).panes).map(NonNull::as_ptr)
            {
                if loopwp == wp || !cmd_kill_pane_filter(item, s, wl, loopwp, filter) {
                    continue;
                }
                server_client_remove_pane(loopwp);
                layout_close_pane(loopwp);
                window_remove_pane((*wl).window, loopwp);
            }
            server_redraw_window((*wl).window);
            return cmd_retval::CMD_RETURN_NORMAL;
        }

        server_kill_pane(wp);
        cmd_retval::CMD_RETURN_NORMAL
    }
}
