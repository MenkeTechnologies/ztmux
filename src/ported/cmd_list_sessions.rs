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

pub static CMD_LIST_SESSIONS_ENTRY: cmd_entry = cmd_entry {
    name: "list-sessions",
    alias: Some("ls"),

    args: args_parse::new("F:f:O:o:r", 0, 0, None),
    usage: "[-r] [-F format] [-f filter] [-O order] [-o json|jsonl|csv|tsv|table|yaml]",

    flags: cmd_flag::CMD_AFTERHOOK,
    exec: cmd_list_sessions_exec,
    source: cmd_entry_flag::zeroed(),
    target: cmd_entry_flag::zeroed(),
};

const LIST_SESSIONS_TEMPLATE: *const u8 = c!(
    "#{session_name}: #{session_windows} windows (created #{t:session_created})#{?session_grouped, (group ,}#{session_group}#{?session_grouped,),}#{?session_attached, (attached),}"
);

// Default schema for `-o` structured output (ztmux extension).
use crate::structured::{Field, FieldKind, OutputFormat, Structured};
const LIST_SESSIONS_FIELDS: &[Field] = &[
    Field { key: "name", fmt: c!("#{session_name}"), kind: FieldKind::Str },
    Field { key: "id", fmt: c!("#{session_id}"), kind: FieldKind::Str },
    Field { key: "windows", fmt: c!("#{session_windows}"), kind: FieldKind::Int },
    Field { key: "created", fmt: c!("#{session_created}"), kind: FieldKind::Int },
    Field { key: "attached", fmt: c!("#{session_attached}"), kind: FieldKind::Bool },
    Field { key: "grouped", fmt: c!("#{session_grouped}"), kind: FieldKind::Bool },
    Field { key: "group", fmt: c!("#{session_group}"), kind: FieldKind::Str },
    Field { key: "activity", fmt: c!("#{session_activity}"), kind: FieldKind::Int },
];

/// C `vendor/tmux/cmd-list-sessions.c:53`: `static enum cmd_retval cmd_list_sessions_exec(struct cmd *self, struct cmdq_item *item)`
unsafe fn cmd_list_sessions_exec(self_: *mut cmd, item: *mut cmdq_item) -> cmd_retval {
    unsafe {
        let args = cmd_get_args(self_);

        let mut template = args_get(args, b'F');
        if template.is_null() {
            template = LIST_SESSIONS_TEMPLATE;
        }
        let filter = args_get(args, b'f');

        // -O sort order / -r reverse (C cmd-list-sessions.c:68-73). A missing or
        // unknown -O yields SORT_END, which sort_get_sessions treats as natural
        // (RB-tree) order — matching the previous unsorted iteration.
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

        // -o requests structured output (ztmux extension); collect rows and
        // emit once at the end instead of printing a text line per session.
        let mut structured = match OutputFormat::parse(args_get(args, b'o')) {
            Ok(fmt) => fmt.map(|f| Structured::new(f, LIST_SESSIONS_FIELDS)),
            Err(()) => {
                cmdq_error!(item, "unknown -o format (want json, jsonl, csv or tsv)");
                return cmd_retval::CMD_RETURN_ERROR;
            }
        };

        for (n, s) in sort_get_sessions(sort_crit).into_iter().enumerate() {
            let ft = format_create(
                cmdq_get_client(item),
                item,
                FORMAT_NONE,
                format_flags::empty(),
            );
            format_add!(ft, "line", "{n}");
            format_defaults(ft, null_mut(), NonNull::new(s), None, None);

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
