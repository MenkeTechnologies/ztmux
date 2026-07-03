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
use crate::*;

const LIST_WINDOWS_TEMPLATE: *const u8 = c!(
    "#{window_index}: #{window_name}#{window_raw_flags} (#{window_panes} panes) [#{window_width}x#{window_height}] [layout #{window_layout}] #{window_id}#{?window_active, (active),}"
);
const LIST_WINDOWS_WITH_SESSION_TEMPLATE: *const u8 = c!(
    "#{session_name}:#{window_index}: #{window_name}#{window_raw_flags} (#{window_panes} panes) [#{window_width}x#{window_height}] "
);

pub static CMD_LIST_WINDOWS_ENTRY: cmd_entry = cmd_entry {
    name: "list-windows",
    alias: Some("lsw"),

    args: args_parse::new("aF:f:O:o:rt:", 0, 0, None),
    usage: "[-ar] [-F format] [-f filter] [-O order] [-o json|jsonl|csv|tsv|table|yaml] [-t target-session]",

    target: cmd_entry_flag::new(
        b't',
        cmd_find_type::CMD_FIND_SESSION,
        cmd_find_flags::empty(),
    ),

    flags: cmd_flag::CMD_AFTERHOOK,
    exec: cmd_list_windows_exec,
    source: cmd_entry_flag::zeroed(),
};

use crate::structured::{Field, FieldKind, OutputFormat, Structured};
const LIST_WINDOWS_FIELDS: &[Field] = &[
    Field { key: "session", fmt: c!("#{session_name}"), kind: FieldKind::Str },
    Field { key: "index", fmt: c!("#{window_index}"), kind: FieldKind::Int },
    Field { key: "name", fmt: c!("#{window_name}"), kind: FieldKind::Str },
    Field { key: "id", fmt: c!("#{window_id}"), kind: FieldKind::Str },
    Field { key: "active", fmt: c!("#{window_active}"), kind: FieldKind::Bool },
    Field { key: "panes", fmt: c!("#{window_panes}"), kind: FieldKind::Int },
    Field { key: "width", fmt: c!("#{window_width}"), kind: FieldKind::Int },
    Field { key: "height", fmt: c!("#{window_height}"), kind: FieldKind::Int },
    Field { key: "layout", fmt: c!("#{window_layout}"), kind: FieldKind::Str },
    Field { key: "zoomed", fmt: c!("#{window_zoomed_flag}"), kind: FieldKind::Bool },
    Field { key: "bell", fmt: c!("#{window_bell_flag}"), kind: FieldKind::Bool },
    Field { key: "activity", fmt: c!("#{window_activity_flag}"), kind: FieldKind::Bool },
    Field { key: "silence", fmt: c!("#{window_silence_flag}"), kind: FieldKind::Bool },
];

/// C `vendor/tmux/cmd-list-windows.c:59`: `static enum cmd_retval cmd_list_windows_exec(struct cmd *self, struct cmdq_item *item)`
unsafe fn cmd_list_windows_exec(self_: *mut cmd, item: *mut cmdq_item) -> cmd_retval {
    unsafe {
        let args = cmd_get_args(self_);
        let target = cmdq_get_target(item);

        let mut structured = match OutputFormat::parse(args_get(args, b'o')) {
            Ok(fmt) => fmt.map(|f| Structured::new(f, LIST_WINDOWS_FIELDS)),
            Err(()) => {
                cmdq_error!(item, "unknown -o format (want json, jsonl, csv or tsv)");
                return cmd_retval::CMD_RETURN_ERROR;
            }
        };

        // -O sort order / -r reverse (C cmd-list-windows.c:75-88). A missing -O
        // is SORT_END = natural (per-session RB) order, matching the previous
        // nested iteration.
        let order = sort_order_from_string(args_get(args, b'O'));
        if order == sort_order::SORT_END && args_has(args, 'O') {
            cmdq_error!(item, "invalid sort order");
            return cmd_retval::CMD_RETURN_ERROR;
        }
        let sort_crit = sort_criteria {
            order,
            reversed: args_has(args, 'r'),
            order_seq: null_mut(),
        };

        // -a lists every window server-wide (globally sorted, with the session
        // shown); otherwise the target session's windows.
        let mut template = args_get(args, b'F');
        let winlinks = if args_has(args, 'a') {
            if template.is_null() {
                template = LIST_WINDOWS_WITH_SESSION_TEMPLATE;
            }
            sort_get_winlinks(sort_crit)
        } else {
            if template.is_null() {
                template = LIST_WINDOWS_TEMPLATE;
            }
            sort_get_winlinks_session((*target).s, sort_crit)
        };
        let filter = args_get(args, b'f');

        for (n, wl) in winlinks.into_iter().enumerate() {
            let ft = format_create(
                cmdq_get_client(item),
                item,
                FORMAT_NONE,
                format_flags::empty(),
            );
            format_add!(ft, "line", "{n}");
            format_defaults(
                ft,
                null_mut(),
                NonNull::new((*wl).session),
                NonNull::new(wl),
                None,
            );

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

        if let Some(out) = structured.as_ref() {
            cmdq_print!(item, "{}", out.render());
        }

        cmd_retval::CMD_RETURN_NORMAL
    }
}
