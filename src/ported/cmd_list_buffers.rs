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

pub static CMD_LIST_BUFFERS_ENTRY: cmd_entry = cmd_entry {
    name: "list-buffers",
    alias: Some("lsb"),

    args: args_parse::new("F:f:O:o:r", 0, 0, None),
    usage: "[-F format] [-f filter] [-O order] [-o json|jsonl|csv|tsv|table|yaml] [-r]",

    flags: cmd_flag::CMD_AFTERHOOK,
    exec: cmd_list_buffers_exec,
    source: cmd_entry_flag::zeroed(),
    target: cmd_entry_flag::zeroed(),
};

use crate::structured::{Field, FieldKind, OutputFormat, Structured};
const LIST_BUFFERS_FIELDS: &[Field] = &[
    Field { key: "name", fmt: c!("#{buffer_name}"), kind: FieldKind::Str },
    Field { key: "size", fmt: c!("#{buffer_size}"), kind: FieldKind::Int },
    Field { key: "sample", fmt: c!("#{buffer_sample}"), kind: FieldKind::Str },
];

/// C `vendor/tmux/cmd-list-buffers.c:47`: `static enum cmd_retval cmd_list_buffers_exec(struct cmd *self, struct cmdq_item *item)`
unsafe fn cmd_list_buffers_exec(self_: *mut cmd, item: *mut cmdq_item) -> cmd_retval {
    unsafe {
        let args = cmd_get_args(self_);

        let mut template: *const u8 = args_get(args, b'F');
        if template.is_null() {
            template = c!("#{buffer_name}: #{buffer_size} bytes: \"#{buffer_sample}\"");
        }
        let filter = args_get(args, b'f');

        let mut structured = match OutputFormat::parse(args_get(args, b'o')) {
            Ok(fmt) => fmt.map(|f| Structured::new(f, LIST_BUFFERS_FIELDS)),
            Err(()) => {
                cmdq_error!(item, "unknown -o format (want json, jsonl, csv or tsv)");
                return cmd_retval::CMD_RETURN_ERROR;
            }
        };

        // Collect buffers (paste_walk = insertion/time order), then honour -O/-r
        // (vendor/tmux sort_get_buffers). ztmux already keeps buffers sorted in
        // RB trees, but collecting + sorting mirrors the C's per-order criteria.
        let mut list: Vec<*mut paste_buffer> = Vec::new();
        let mut pb = null_mut();
        loop {
            pb = paste_walk(pb);
            if pb.is_null() {
                break;
            }
            list.push(pb);
        }
        let order = args_get(args, b'O');
        if !order.is_null() {
            match cstr_to_str_(order) {
                Some("name") | Some("title") => {
                    list.sort_by(|&a, &b| (*a).name.cmp(&(*b).name));
                }
                // C SORT_CREATION: newest (higher order) first, tie-break by name.
                Some("creation") => list.sort_by(|&a, &b| {
                    (*b).order.cmp(&(*a).order).then_with(|| (*a).name.cmp(&(*b).name))
                }),
                Some("size") => list.sort_by(|&a, &b| {
                    (*a).size.cmp(&(*b).size).then_with(|| (*a).name.cmp(&(*b).name))
                }),
                _ => {
                    cmdq_error!(item, "invalid sort order");
                    return cmd_retval::CMD_RETURN_ERROR;
                }
            }
            // C `sort_qsort` (sort.c): `-r` reverses only when an order is
            // active. With no `-O` the order is SORT_END and sort_qsort returns
            // *before* honouring `reversed`, so bare `list-buffers -r` keeps the
            // (newest-first) paste_walk order. Only reverse inside the `-O` arm.
            if args_has(args, 'r') {
                list.reverse();
            }
        }

        for &pb in &list {
            let ft = format_create(
                cmdq_get_client(item),
                item,
                FORMAT_NONE,
                format_flags::empty(),
            );
            format_defaults_paste_buffer(ft, pb);

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
