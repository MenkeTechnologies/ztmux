// Copyright (c) 2021 Dallas Lyons <dallasdlyons@gmail.com>
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
use crate::libc::{getpwnam, getuid};
use crate::*;

pub static CMD_SERVER_ACCESS_ENTRY: cmd_entry = cmd_entry {
    name: "server-access",
    alias: None,

    args: args_parse::new("adglrw", 0, 1, None),
    usage: "[-adglrw] [user|group]",

    flags: cmd_flag::CMD_CLIENT_CANFAIL,
    exec: cmd_server_access_exec,
    source: cmd_entry_flag::zeroed(),
    target: cmd_entry_flag::zeroed(),
};

/// C `vendor/tmux/cmd-server-access.c:49`: `static enum cmd_retval cmd_server_access_deny(struct cmdq_item *item, id_t id, int flags, const char *type, const char *name)`
unsafe fn cmd_server_access_deny(
    item: *mut cmdq_item,
    id: uid_t,
    flags: server_acl_user_flags,
    type_: &str,
    name: *const u8,
) -> cmd_retval {
    unsafe {
        if server_acl_user_find(id, flags).is_null() {
            cmdq_error!(item, "{} {} not found", type_, _s(name));
            return cmd_retval::CMD_RETURN_ERROR;
        }
        // The C leaves dropping the affected clients to server_acl_update,
        // which server_acl_deny calls (cmd-server-access.c:56).
        server_acl_user_deny(id, flags);
        cmd_retval::CMD_RETURN_NORMAL
    }
}

/// C `vendor/tmux/cmd-server-access.c:61`: `static enum cmd_retval cmd_server_access_exec(struct cmd *self, struct cmdq_item *item)`
unsafe fn cmd_server_access_exec(self_: *mut cmd, item: *mut cmdq_item) -> cmd_retval {
    unsafe {
        let args = cmd_get_args(self_);
        let c = cmdq_get_target_client(item);

        if args_has(args, 'l') {
            server_acl_display(item);
            return cmd_retval::CMD_RETURN_NORMAL;
        }
        if args_count(args) == 0 {
            cmdq_error!(item, "missing user or group argument");
            return cmd_retval::CMD_RETURN_ERROR;
        }

        let arg = format_single(
            item,
            cstr_to_str(args_string(args, 0)),
            c,
            null_mut(),
            null_mut(),
            null_mut(),
        );

        let mut id: uid_t = 0;
        let mut name: *const u8 = null_mut();
        let mut flags = server_acl_user_flags::empty();
        let type_ = if args_has(args, 'g') {
            let gr = libc::getgrnam(arg.cast());
            if !gr.is_null() {
                id = (*gr).gr_gid;
                name = (*gr).gr_name.cast();
                flags |= server_acl_user_flags::SERVER_ACL_IS_GROUP;
            }
            "group"
        } else {
            let pw = getpwnam(arg.cast());
            if !pw.is_null() {
                id = (*pw).pw_uid;
                name = (*pw).pw_name.cast();
            }
            "user"
        };
        if name.is_null() {
            cmdq_error!(item, "unknown {}: {}", type_, _s(arg));
            free_(arg);
            return cmd_retval::CMD_RETURN_ERROR;
        }
        free_(arg);

        // Only a user can own the server (cmd-server-access.c:103).
        if !flags.contains(server_acl_user_flags::SERVER_ACL_IS_GROUP)
            && (id == 0 || id == getuid())
        {
            cmdq_error!(item, "{} owns the server, can't change access", _s(name));
            return cmd_retval::CMD_RETURN_ERROR;
        }

        if args_has(args, 'a') && args_has(args, 'd') {
            cmdq_error!(item, "-a and -d cannot be used together");
            return cmd_retval::CMD_RETURN_ERROR;
        }
        if args_has(args, 'w') && args_has(args, 'r') {
            cmdq_error!(item, "-r and -w cannot be used together");
            return cmd_retval::CMD_RETURN_ERROR;
        }

        if args_has(args, 'd') {
            return cmd_server_access_deny(item, id, flags, type_, name);
        }
        if args_has(args, 'a') {
            if !server_acl_user_find(id, flags).is_null() {
                cmdq_error!(item, "{} {} is already added", type_, _s(name));
                return cmd_retval::CMD_RETURN_ERROR;
            }
            server_acl_user_allow(id, flags);
            // Do not return - allow -r or -w with -a.
        } else if (args_has(args, 'r') || args_has(args, 'w'))
            && server_acl_user_find(id, flags).is_null()
        {
            // -r or -w implies -a if the entry does not exist.
            server_acl_user_allow(id, flags);
        }

        if args_has(args, 'w') {
            if server_acl_user_find(id, flags).is_null() {
                cmdq_error!(item, "{} {} not found", type_, _s(name));
                return cmd_retval::CMD_RETURN_ERROR;
            }
            server_acl_user_allow_write(id, flags);
            return cmd_retval::CMD_RETURN_NORMAL;
        }

        if args_has(args, 'r') {
            if server_acl_user_find(id, flags).is_null() {
                cmdq_error!(item, "{} {} not found", type_, _s(name));
                return cmd_retval::CMD_RETURN_ERROR;
            }
            server_acl_user_deny_write(id, flags);
            return cmd_retval::CMD_RETURN_NORMAL;
        }

        cmd_retval::CMD_RETURN_NORMAL
    }
}
