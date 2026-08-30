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
use crate::options_::options_get_number___;

pub static CMD_LIST_KEYS_ENTRY: cmd_entry = cmd_entry {
    name: "list-keys",
    alias: Some("lsk"),

    args: args_parse::new("1aF:NO:P:rT:", 0, 1, None),
    usage: "[-1aNr] [-F format] [-O order] [-P prefix-string][-T key-table] [key]",

    flags: cmd_flag::CMD_STARTSERVER.union(cmd_flag::CMD_AFTERHOOK),
    exec: cmd_list_keys_exec,
    source: cmd_entry_flag::zeroed(),
    target: cmd_entry_flag::zeroed(),
};

pub static CMD_LIST_COMMANDS_ENTRY: cmd_entry = cmd_entry {
    name: "list-commands",
    alias: Some("lscm"),

    args: args_parse::new("F:", 0, 1, None),
    usage: "[-F format] [command]",

    flags: cmd_flag::CMD_STARTSERVER.union(cmd_flag::CMD_AFTERHOOK),
    exec: cmd_list_keys_exec,
    source: cmd_entry_flag::zeroed(),
    target: cmd_entry_flag::zeroed(),
};

/// The default `-F` template (`vendor/tmux/cmd-list-keys.c:30`).
const LIST_KEYS_TEMPLATE: &str = concat!(
    "#{?notes_only,",
    "#{key_prefix} ",
    "#{p|#{key_string_width}:key_string} ",
    "#{?key_note,#{key_note},#{key_command}}",
    ",",
    "bind-key #{?key_has_repeat,#{?key_repeat,-r,  },} ",
    "-T #{p|#{key_table_width}:key_table} ",
    "#{p|#{key_string_width}:#{q|a:key_string}} ",
    "#{key_command}}",
);

/// C `vendor/tmux/cmd-list-keys.c:56`: `static char *cmd_list_keys_get_prefix(struct args *args)`
unsafe fn cmd_list_keys_get_prefix(args: *mut args) -> *mut u8 {
    unsafe {
        if args_has(args, 'P') {
            return xstrdup(args_get_(args, 'P')).as_ptr();
        }
        let prefix: key_code = options_get_number___::<i64>(&*GLOBAL_S_OPTIONS, "prefix") as _;
        if prefix == KEYC_NONE {
            return xstrdup_(c"").as_ptr();
        }
        xstrdup(key_string_lookup_key(prefix, 0)).as_ptr()
    }
}

/// C `vendor/tmux/cmd-list-keys.c:70`: `static u_int cmd_list_keys_get_width(struct key_binding **l, u_int n)`
unsafe fn cmd_list_keys_get_width(l: &[*mut key_binding]) -> u32 {
    unsafe {
        l.iter()
            .map(|&bd| utf8_cstrwidth(key_string_lookup_key((*bd).key, 0)))
            .max()
            .unwrap_or(0)
    }
}

/// C `vendor/tmux/cmd-list-keys.c:83`: `static u_int cmd_list_keys_get_table_width(struct key_binding **l, u_int n)`
unsafe fn cmd_list_keys_get_table_width(l: &[*mut key_binding]) -> u32 {
    unsafe {
        l.iter()
            .map(|&bd| utf8_cstrwidth((*bd).tablename))
            .max()
            .unwrap_or(0)
    }
}

/// C `vendor/tmux/cmd-list-keys.c:96`: `static struct key_binding **cmd_list_keys_get_root_and_prefix(u_int *n, struct sort_criteria *sort_crit)`
unsafe fn cmd_list_keys_get_root_and_prefix(sc: sort_criteria) -> Vec<*mut key_binding> {
    unsafe {
        let mut l: Vec<*mut key_binding> = Vec::new();
        for name in [c!("prefix"), c!("root")] {
            let t = key_bindings_get_table(name, false);
            if !t.is_null() {
                l.extend(sort_get_key_bindings_table(t, sc));
            }
        }
        l
    }
}

/// C `vendor/tmux/cmd-list-keys.c:122`: `static void cmd_list_keys_filter_key_list(int filter_notes, int filter_key, key_code only, struct key_binding **l, u_int *n)`
unsafe fn cmd_list_keys_filter_key_list(
    filter_notes: bool,
    filter_key: bool,
    only: key_code,
    l: &mut Vec<*mut key_binding>,
) {
    unsafe {
        l.retain(|&bd| {
            let key = (*bd).key & (KEYC_MASK_KEY | KEYC_MASK_MODIFIERS);
            if filter_key && only != key {
                return false;
            }
            if filter_notes && (*bd).note.is_none() {
                return false;
            }
            true
        });
    }
}

/// C `vendor/tmux/cmd-list-keys.c:140`: `static void cmd_list_keys_format_add_key_binding(struct format_tree *ft, const struct key_binding *bd, const char *prefix)`
unsafe fn cmd_list_keys_format_add_key_binding(
    ft: *mut format_tree,
    bd: *mut key_binding,
    prefix: *const u8,
) {
    unsafe {
        if (*bd).flags & KEY_BINDING_REPEAT != 0 {
            format_add!(ft, "key_repeat", "1");
        } else {
            format_add!(ft, "key_repeat", "0");
        }

        match &(*bd).note {
            Some(note) => format_add!(ft, "key_note", "{}", note.to_string_lossy()),
            None => format_add!(ft, "key_note", "{}", ""),
        }

        format_add!(ft, "key_prefix", "{}", _s(prefix));
        format_add!(ft, "key_table", "{}", _s((*bd).tablename));
        format_add!(ft, "key_string", "{}", _s(key_string_lookup_key((*bd).key, 0)));

        let s = cmd_list_print(
            &*(*bd).cmdlist,
            CMD_LIST_PRINT_ESCAPED | CMD_LIST_PRINT_NO_GROUPS,
        );
        format_add!(ft, "key_command", "{}", _s(s));
        free_(s);
    }
}

/// C `vendor/tmux/cmd-list-keys.c:167`: `static enum cmd_retval cmd_list_keys_exec(struct cmd *self, struct cmdq_item *item)`
unsafe fn cmd_list_keys_exec(self_: *mut cmd, item: *mut cmdq_item) -> cmd_retval {
    unsafe {
        let args = cmd_get_args(self_);
        let tc = cmdq_get_target_client(item);
        let mut only: key_code = KEYC_UNKNOWN;
        let mut table: *mut key_table = null_mut();

        if std::ptr::eq(cmd_get_entry(self_), &CMD_LIST_COMMANDS_ENTRY) {
            return cmd_list_keys_commands(self_, item);
        }

        let keystr = args_string(args, 0);
        if !keystr.is_null() {
            only = key_string_lookup_string(keystr);
            if only == KEYC_UNKNOWN {
                cmdq_error!(item, "invalid key: {}", _s(keystr));
                return cmd_retval::CMD_RETURN_ERROR;
            }
            only &= KEYC_MASK_KEY | KEYC_MASK_MODIFIERS;
        }

        let mut sort_crit = sort_criteria {
            order: sort_order_from_string(args_get(args, b'O')),
            ..zeroed()
        };
        if sort_crit.order == sort_order::SORT_END && args_has(args, 'O') {
            cmdq_error!(item, "invalid sort order");
            return cmd_retval::CMD_RETURN_ERROR;
        }
        sort_crit.reversed = args_has(args, 'r');

        let tablename = args_get(args, b'T');
        if !tablename.is_null() {
            table = key_bindings_get_table(tablename, false);
            if table.is_null() {
                cmdq_error!(item, "table {} doesn't exist", _s(tablename));
                return cmd_retval::CMD_RETURN_ERROR;
            }
        }

        let prefix = cmd_list_keys_get_prefix(args);
        let single = args_has(args, '1');
        let notes_only = args_has(args, 'N');

        let template = args_get(args, b'F');
        let template_owned;
        let template: *const u8 = if template.is_null() {
            template_owned = std::ffi::CString::new(LIST_KEYS_TEMPLATE).unwrap();
            template_owned.as_ptr().cast()
        } else {
            template
        };

        let mut l = if !table.is_null() {
            sort_get_key_bindings_table(table, sort_crit)
        } else if notes_only {
            cmd_list_keys_get_root_and_prefix(sort_crit)
        } else {
            sort_get_key_bindings(sort_crit)
        };

        let filter_notes = notes_only && !args_has(args, 'a');
        let filter_key = only != KEYC_UNKNOWN;
        if filter_notes || filter_key {
            cmd_list_keys_filter_key_list(filter_notes, filter_key, only, &mut l);
        }
        if filter_key && l.is_empty() {
            cmdq_error!(item, "unknown key: {}", _s(keystr));
            free_(prefix);
            return cmd_retval::CMD_RETURN_ERROR;
        }
        if single && l.len() > 1 {
            l.truncate(1);
        }

        let ft = format_create(cmdq_get_client(item), item, FORMAT_NONE, format_flags::empty());
        format_defaults(ft, tc, None, None, None);
        format_add!(ft, "notes_only", "{}", i32::from(notes_only));
        format_add!(
            ft,
            "key_has_repeat",
            "{}",
            i32::from(key_bindings_has_repeat(&l))
        );
        format_add!(ft, "key_string_width", "{}", cmd_list_keys_get_width(&l));
        format_add!(
            ft,
            "key_table_width",
            "{}",
            cmd_list_keys_get_table_width(&l)
        );

        for &bd in &l {
            cmd_list_keys_format_add_key_binding(ft, bd, prefix);

            let line = format_expand(ft, template);
            if single && !tc.is_null() && !(*tc).flags.intersects(client_flag::CONTROL) {
                status_message_set!(tc, -1, 1, false, 0, "{}", _s(line));
            } else if *line != b'\0' {
                cmdq_print!(item, "{}", _s(line));
            }
            free_(line);

            if single {
                break;
            }
        }
        format_free(ft);
        free_(prefix);

        cmd_retval::CMD_RETURN_NORMAL
    }
}



unsafe fn cmd_list_keys_commands(self_: *mut cmd, item: *mut cmdq_item) -> cmd_retval {
    unsafe {
        let args = cmd_get_args(self_);

        let mut template = args_get_(args, 'F');
        if template.is_null() {
            template = cstring_concat!(
                "#{command_list_name}",
                "#{?command_list_alias, (#{command_list_alias}),} ",
                "#{command_list_usage}"
            )
            .as_ptr()
            .cast();
        }

        let ft = format_create(
            cmdq_get_client(item),
            item,
            FORMAT_NONE,
            format_flags::empty(),
        );
        format_defaults(ft, null_mut(), None, None, None);

        let command = args_string(args, 0);
        if command.is_null() {
            for entry in CMD_TABLE {
                cmd_list_single_command(entry, ft, template, item);
            }
        } else {
            // The C looks the name up with cmd_find (`cmd-list-commands.c:95`),
            // so an abbreviation resolves the same way it does on a command
            // line and an unknown name reports cmd_find's own error.
            match cmd_find(cstr_to_str(command)) {
                Ok(entry) => cmd_list_single_command(entry, ft, template, item),
                Err(cause) => {
                    cmdq_error!(item, "{}", cause);
                    format_free(ft);
                    return cmd_retval::CMD_RETURN_ERROR;
                }
            }
        }

        format_free(ft);
        cmd_retval::CMD_RETURN_NORMAL
    }
}

/// C `vendor/tmux/cmd-list-commands.c:48`: `static void cmd_list_single_command(const struct cmd_entry *entry, struct format_tree *ft, const char *template, struct cmdq_item *item)`
unsafe fn cmd_list_single_command(
    entry: &cmd_entry,
    ft: *mut format_tree,
    template: *const u8,
    item: *mut cmdq_item,
) {
    unsafe {
        format_add!(ft, "command_list_name", "{}", entry.name);
        format_add!(
            ft,
            "command_list_alias",
            "{}",
            entry.alias.unwrap_or_default()
        );
        format_add!(ft, "command_list_usage", "{}", entry.usage);

        let line = format_expand(ft, template);
        if *line != b'\0' {
            cmdq_print!(item, "{}", _s(line));
        }
        free_(line);
    }
}
