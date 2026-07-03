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
use crate::compat::tree::rb_foreach;
use crate::*;

pub static CMD_LIST_PANES_ENTRY: cmd_entry = cmd_entry {
    name: "list-panes",
    alias: Some("lsp"),

    args: args_parse::new("asF:f:O:o:rt:", 0, 0, None),
    usage: "[-asr] [-F format] [-f filter] [-O order] [-o json|jsonl|csv|tsv|table|yaml] [-t target-window]",

    target: cmd_entry_flag::new(
        b't',
        cmd_find_type::CMD_FIND_WINDOW,
        cmd_find_flags::empty(),
    ),

    flags: cmd_flag::CMD_AFTERHOOK,
    exec: cmd_list_panes_exec,
    source: cmd_entry_flag::zeroed(),
};

use crate::structured::{Field, FieldKind, OutputFormat, Structured};
const LIST_PANES_FIELDS: &[Field] = &[
    Field { key: "session", fmt: c!("#{session_name}"), kind: FieldKind::Str },
    Field { key: "window", fmt: c!("#{window_index}"), kind: FieldKind::Int },
    Field { key: "index", fmt: c!("#{pane_index}"), kind: FieldKind::Int },
    Field { key: "id", fmt: c!("#{pane_id}"), kind: FieldKind::Str },
    Field { key: "active", fmt: c!("#{pane_active}"), kind: FieldKind::Bool },
    Field { key: "dead", fmt: c!("#{pane_dead}"), kind: FieldKind::Bool },
    Field { key: "pid", fmt: c!("#{pane_pid}"), kind: FieldKind::Int },
    Field { key: "tty", fmt: c!("#{pane_tty}"), kind: FieldKind::Str },
    Field { key: "command", fmt: c!("#{pane_current_command}"), kind: FieldKind::Str },
    Field { key: "path", fmt: c!("#{pane_current_path}"), kind: FieldKind::Str },
    Field { key: "title", fmt: c!("#{pane_title}"), kind: FieldKind::Str },
    Field { key: "width", fmt: c!("#{pane_width}"), kind: FieldKind::Int },
    Field { key: "height", fmt: c!("#{pane_height}"), kind: FieldKind::Int },
    Field { key: "history_size", fmt: c!("#{history_size}"), kind: FieldKind::Int },
    Field { key: "history_limit", fmt: c!("#{history_limit}"), kind: FieldKind::Int },
    Field { key: "in_mode", fmt: c!("#{pane_in_mode}"), kind: FieldKind::Bool },
    Field { key: "mode", fmt: c!("#{pane_mode}"), kind: FieldKind::Str },
    Field { key: "marked", fmt: c!("#{pane_marked}"), kind: FieldKind::Bool },
];

/// C `vendor/tmux/cmd-list-panes.c:52`: `static enum cmd_retval cmd_list_panes_exec(struct cmd *self, struct cmdq_item *item)`
unsafe fn cmd_list_panes_exec(self_: *mut cmd, item: *mut cmdq_item) -> cmd_retval {
    unsafe {
        let args = cmd_get_args(self_);
        let target = cmdq_get_target(item);
        let s = (*target).s;
        let wl = (*target).wl;

        // -O sort order validated once here (C cmd-list-panes.c:66-70); each
        // window helper re-reads it and sorts its own panes.
        let order = sort_order_from_string(args_get(args, b'O'));
        if order == sort_order::SORT_END && args_has(args, 'O') {
            cmdq_error!(item, "invalid sort order");
            return cmd_retval::CMD_RETURN_ERROR;
        }

        let mut structured = match OutputFormat::parse(args_get(args, b'o')) {
            Ok(fmt) => fmt.map(|f| Structured::new(f, LIST_PANES_FIELDS)),
            Err(()) => {
                cmdq_error!(item, "unknown -o format (want json, jsonl, csv or tsv)");
                return cmd_retval::CMD_RETURN_ERROR;
            }
        };

        if args_has(args, 'a') {
            cmd_list_panes_server(self_, item, &mut structured);
        } else if args_has(args, 's') {
            cmd_list_panes_session(self_, s, item, 1, &mut structured);
        } else {
            cmd_list_panes_window(self_, s, wl, item, 0, &mut structured);
        }

        if let Some(out) = structured.as_ref() {
            cmdq_print!(item, "{}", out.render());
        }

        cmd_retval::CMD_RETURN_NORMAL
    }
}

/// C `vendor/tmux/cmd-list-panes.c:77`: `static void cmd_list_panes_server(struct cmd *self, struct cmdq_item *item)`
unsafe fn cmd_list_panes_server(
    self_: *mut cmd,
    item: *mut cmdq_item,
    structured: &mut Option<Structured>,
) {
    unsafe {
        for s in rb_foreach(&raw mut SESSIONS).map(NonNull::as_ptr) {
            cmd_list_panes_session(self_, s, item, 2, structured);
        }
    }
}

/// C `vendor/tmux/cmd-list-panes.c:86`: `static void cmd_list_panes_session(struct cmd *self, struct session *s, struct cmdq_item *item, int type)`
unsafe fn cmd_list_panes_session(
    self_: *mut cmd,
    s: *mut session,
    item: *mut cmdq_item,
    type_: i32,
    structured: &mut Option<Structured>,
) {
    unsafe {
        for wl in rb_foreach(&raw mut (*s).windows).map(NonNull::as_ptr) {
            cmd_list_panes_window(self_, s, wl, item, type_, structured);
        }
    }
}

/// C `vendor/tmux/cmd-list-panes.c:96`: `static void cmd_list_panes_window(struct cmd *self, struct session *s, struct winlink *wl, struct cmdq_item *item, int type)`
fn cmd_list_panes_window(
    self_: *mut cmd,
    s: *mut session,
    wl: *mut winlink,
    item: *mut cmdq_item,
    type_: i32,
    structured: &mut Option<Structured>,
) {
    unsafe {
        let args = cmd_get_args(self_);

        let mut template = args_get_(args, 'F');
        if template.is_null() {
            match type_ {
                0 => {
                    template = cstring_concat!(
                        "#{pane_index}: ",
                        "[#{pane_width}x#{pane_height}] [history ",
                        "#{history_size}/#{history_limit}, ",
                        "#{history_bytes} bytes] #{pane_id}",
                        "#{?pane_active, (active),}#{?pane_dead, (dead),}"
                    )
                    .as_ptr()
                    .cast();
                }
                1 => {
                    template = cstring_concat!(
                        "#{window_index}.#{pane_index}: ",
                        "[#{pane_width}x#{pane_height}] [history ",
                        "#{history_size}/#{history_limit}, ",
                        "#{history_bytes} bytes] #{pane_id}",
                        "#{?pane_active, (active),}#{?pane_dead, (dead),}"
                    )
                    .as_ptr()
                    .cast();
                }
                2 => {
                    template = cstring_concat!(
                        "#{session_name}:#{window_index}.",
                        "#{pane_index}: [#{pane_width}x#{pane_height}] ",
                        "[history #{history_size}/#{history_limit}, ",
                        "#{history_bytes} bytes] #{pane_id}",
                        "#{?pane_active, (active),}#{?pane_dead, (dead),}"
                    )
                    .as_ptr()
                    .cast();
                }
                _ => (),
            }
        }
        let filter = args_get_(args, 'f');

        // -O sort order / -r reverse the panes within this window
        // (C cmd-list-panes.c: sort_get_panes_window). SORT_END = natural (tailq)
        // order, matching the previous iteration.
        let sort_crit = sort_criteria {
            order: sort_order_from_string(args_get(args, b'O')),
            reversed: args_has(args, 'r'),
            order_seq: null_mut(),
        };

        for (n, wp) in sort_get_panes_window((*wl).window, sort_crit)
            .into_iter()
            .enumerate()
        {
            let ft = format_create(
                cmdq_get_client(item),
                item,
                FORMAT_NONE,
                format_flags::empty(),
            );
            format_add!(ft, "line", "{n}");
            format_defaults(ft, null_mut(), NonNull::new(s), NonNull::new(wl), NonNull::new(wp));

            let flag;
            if !filter.is_null() {
                let expanded = format_expand(ft, filter);
                flag = format_true(expanded);
                free_(expanded);
            } else {
                flag = true;
            }
            if flag {
                if let Some(out) = structured.as_mut() {
                    out.add(ft);
                } else {
                    let line = format_expand(ft, template);
                    cmdq_print!(item, "{}", _s(line));
                    free_(line);
                }
            }

            format_free(ft);
        }
    }
}
