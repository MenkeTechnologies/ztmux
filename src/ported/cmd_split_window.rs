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
use crate::options_::*;

const SPLIT_WINDOW_TEMPLATE: *const u8 = c!("#{session_name}:#{window_index}.#{pane_index}");

pub static CMD_SPLIT_WINDOW_ENTRY: cmd_entry = cmd_entry {
    name: "split-window",
    alias: Some("splitw"),

    args: args_parse::new("bc:de:EfF:hIkl:m:p:PR:s:S:t:T:vWZ", 0, -1, None),
    usage: "[-bdefhIklPvWZ] [-c start-directory] [-e environment] [-F format] [-l size] [-m message] [-p percentage] [-s style] [-S active-border-style] [-R inactive-border-style] [-T title] [-t target-pane] [shell-command [argument ...]]",

    target: cmd_entry_flag::new(b't', cmd_find_type::CMD_FIND_PANE, cmd_find_flags::empty()),

    flags: cmd_flag::empty(),
    exec: cmd_split_window_exec,
    source: cmd_entry_flag::zeroed(),
};

/// C `vendor/tmux/cmd-split-window.c:37`: `const struct cmd_entry cmd_new_pane_entry`
pub static CMD_NEW_PANE_ENTRY: cmd_entry = cmd_entry {
    name: "new-pane",
    alias: Some("newp"),

    args: args_parse::new("bB:c:de:EfF:hIkl:Lm:p:PR:s:S:t:T:vWx:X:y:Y:Z", 0, -1, None),
    usage: "[-bdefhIklPvWZ] [-B border-lines] [-c start-directory] [-e environment] [-F format] [-l size] [-m message] [-p percentage] [-s style] [-S active-border-style] [-R inactive-border-style] [-T title] [-x width] [-y height] [-X x-position] [-Y y-position] [-t target-pane] [shell-command [argument ...]]",

    target: cmd_entry_flag::new(b't', cmd_find_type::CMD_FIND_PANE, cmd_find_flags::empty()),

    flags: cmd_flag::empty(),
    exec: cmd_split_window_exec,
    source: cmd_entry_flag::zeroed(),
};

/// C `vendor/tmux/cmd-split-window.c:74`: `static enum cmd_retval cmd_split_window_exec(struct cmd *self, struct cmdq_item *item)`
unsafe fn cmd_split_window_exec(self_: *mut cmd, item: *mut cmdq_item) -> cmd_retval {
    unsafe {
        let args = cmd_get_args(self_);
        let current = cmdq_get_current(item);
        let target = cmdq_get_target(item);
        let tc = cmdq_get_target_client(item);
        let s = (*target).s;
        let wl = (*target).wl;
        let w = (*wl).window;
        let wp = (*target).wp;
        let mut cause: *mut u8 = null_mut();
        let count = args_count(args);

        // new-pane creates a floating pane unless -L is given; split-window
        // makes a floating pane only when splitting a floating pane.
        let is_floating = if std::ptr::eq(cmd_get_entry(self_), &raw const CMD_NEW_PANE_ENTRY) {
            !args_has(args, 'L')
        } else {
            window_pane_is_floating(wp) != 0
        };

        let mut flags = if is_floating {
            SPAWN_FLOATING
        } else {
            spawn_flags::empty()
        };
        if args_has(args, 'b') {
            flags |= SPAWN_BEFORE;
        }
        if args_has(args, 'f') {
            flags |= SPAWN_FULLSIZE;
        }

        let mut input = args_has(args, 'I');
        let empty = if input { true } else { args_has(args, 'E') };
        if empty && count != 0 && (count != 1 || *args_string(args, 0) != b'\0') {
            cmdq_error!(item, "command cannot be given for empty pane");
            return cmd_retval::CMD_RETURN_ERROR;
        }
        if empty {
            flags |= SPAWN_EMPTY;
        }

        let lines;
        let value = args_get_(args, 'B');
        if value.is_null() {
            lines = window_get_pane_lines(w);
        } else {
            let oe = options_search("pane-border-lines");
            match options_find_choice(oe, value) {
                Ok(choice) => lines = std::mem::transmute::<i32, pane_lines>(choice),
                Err(err) => {
                    cmdq_error!(item, "pane-border-lines {}", err.to_string_lossy());
                    return cmd_retval::CMD_RETURN_ERROR;
                }
            }
        }

        let lc = if is_floating {
            layout_get_floating_cell(item, args, lines, w, wp, &raw mut cause)
        } else {
            layout_get_tiled_cell(item, args, w, wp, flags, &raw mut cause)
        };
        if !cause.is_null() {
            cmdq_error!(item, "{}", _s(cause));
            free_(cause);
            return cmd_retval::CMD_RETURN_ERROR;
        }

        let mut sc: spawn_context = zeroed();
        sc.item = item;
        sc.s = s;
        sc.wl = wl;

        sc.wp0 = wp;
        sc.lc = lc;

        args_to_vector(args, &raw mut sc.argc, &raw mut sc.argv);
        sc.environ = environ_create().as_ptr();

        let mut av = args_first_value(args, b'e');
        while !av.is_null() {
            environ_put(sc.environ, (*av).union_.string, environ_flags::empty());
            av = args_next_value(av);
        }

        sc.idx = -1;
        sc.cwd = args_get_(args, 'c');

        sc.flags = flags;
        if args_has(args, 'd') {
            sc.flags |= SPAWN_DETACHED;
        }
        if args_has(args, 'Z') {
            sc.flags |= SPAWN_ZOOM;
        }

        // The C's `fail:` label: free the argv/environ and destroy the cell the
        // pane never took over.
        macro_rules! fail {
            () => {{
                if !sc.argv.is_null() {
                    cmd_free_argv(sc.argc, sc.argv);
                }
                environ_free(sc.environ);
                layout_destroy_cell(w, lc, &raw mut (*w).layout_root);
                return cmd_retval::CMD_RETURN_ERROR;
            }};
        }

        let new_wp = spawn_pane(&raw mut sc, &raw mut cause);
        if new_wp.is_null() {
            cmdq_error!(item, "create pane failed: {}", _s(cause));
            free_(cause);
            fail!();
        }

        let mut style = args_get_(args, 's');
        if !style.is_null() {
            if options_set_string!((*new_wp).options, "window-style", false, "{}", _s(style))
                .is_null()
            {
                cmdq_error!(item, "bad style: {}", _s(style));
                fail!();
            }
            options_set_string!(
                (*new_wp).options,
                "window-active-style",
                false,
                "{}",
                _s(style)
            );
            (*new_wp).flags |= window_pane_flags::PANE_REDRAW
                | window_pane_flags::PANE_STYLECHANGED
                | window_pane_flags::PANE_THEMECHANGED;
        }
        style = args_get_(args, 'S');
        if !style.is_null()
            && options_set_string!(
                (*new_wp).options,
                "pane-active-border-style",
                false,
                "{}",
                _s(style)
            )
            .is_null()
        {
            cmdq_error!(item, "bad active border style: {}", _s(style));
            fail!();
        }
        style = args_get_(args, 'R');
        if !style.is_null()
            && options_set_string!(
                (*new_wp).options,
                "pane-border-style",
                false,
                "{}",
                _s(style)
            )
            .is_null()
        {
            cmdq_error!(item, "bad inactive border style: {}", _s(style));
            fail!();
        }
        if args_has(args, 'B') {
            options_set_number((*new_wp).options, "pane-border-lines", lines as i64);
        }
        if args_has(args, 'k') || args_has(args, 'm') {
            options_set_number((*new_wp).options, "remain-on-exit", 3);
            if args_has(args, 'm') {
                options_set_string!(
                    (*new_wp).options,
                    "remain-on-exit-format",
                    false,
                    "{}",
                    _s(args_get_(args, 'm'))
                );
            }
        }
        if args_has(args, 'T') {
            let title = format_single_from_target(item, args_get_(args, 'T'));
            screen_set_title(&raw mut (*new_wp).base, title);
            notify_pane(c"pane-title-changed", new_wp);
            free_(title);
        }

        if input {
            match window_pane_start_input(new_wp, item, &raw mut cause) {
                -1 => {
                    server_client_remove_pane(new_wp);
                    if !is_floating {
                        layout_close_pane(new_wp);
                    }
                    window_remove_pane((*wp).window, new_wp);
                    cmdq_error!(item, "{}", _s(cause));
                    free_(cause);
                    fail!();
                }
                1 => {
                    input = false;
                }
                _ => (),
            }
        }
        if !args_has(args, 'd') {
            cmd_find_from_winlink_pane(current, wl, new_wp, cmd_find_flags::empty());
        }

        if !is_floating {
            window_pop_zoom((*wp).window);
            server_redraw_window((*wp).window);
        }
        server_redraw_session(s);

        if args_has(args, 'P') {
            let mut template = args_get_(args, 'F');
            if template.is_null() {
                template = SPLIT_WINDOW_TEMPLATE;
            }
            let cp = format_single(item, cstr_to_str(template), tc, s, wl, new_wp);
            cmdq_print!(item, "{}", _s(cp));
            free_(cp);
        }

        let mut fs: cmd_find_state = zeroed();
        cmd_find_from_winlink_pane(&raw mut fs, wl, new_wp, cmd_find_flags::empty());
        cmdq_insert_hook!(s, item, &raw mut fs, "after-split-window");

        if !sc.argv.is_null() {
            cmd_free_argv(sc.argc, sc.argv);
        }
        environ_free(sc.environ);
        if input {
            return cmd_retval::CMD_RETURN_WAIT;
        }

        // The C also blocks here for -W (window_pane->wait_item), which needs
        // window_pane_wait_finish; neither is ported yet.
        cmd_retval::CMD_RETURN_NORMAL
    }
}
