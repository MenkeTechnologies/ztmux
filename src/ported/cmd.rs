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
use crate::compat::{
    queue::{
        tailq_concat, tailq_first, tailq_foreach, tailq_init_, tailq_insert_tail, tailq_next,
        tailq_remove,
    },
    strlcat, strlcpy,
};
use crate::libc::{strchr, strlen, strncmp};
use crate::xmalloc::{xrealloc_, xreallocarray_};
use crate::*;
use crate::options_::*;

#[path = "cmd_attach_session.rs"]
pub mod cmd_attach_session;
#[path = "cmd_bind_key.rs"]
pub mod cmd_bind_key;
#[path = "cmd_break_pane.rs"]
pub mod cmd_break_pane;
#[path = "cmd_capture_pane.rs"]
pub mod cmd_capture_pane;
#[path = "cmd_choose_tree.rs"]
pub mod cmd_choose_tree;
#[path = "cmd_command_prompt.rs"]
pub mod cmd_command_prompt;
#[path = "cmd_confirm_before.rs"]
pub mod cmd_confirm_before;
#[path = "cmd_copy_mode.rs"]
pub mod cmd_copy_mode;
#[path = "cmd_detach_client.rs"]
pub mod cmd_detach_client;
#[path = "cmd_display_menu.rs"]
pub mod cmd_display_menu;
#[path = "cmd_display_message.rs"]
pub mod cmd_display_message;
#[path = "cmd_display_panes.rs"]
pub mod cmd_display_panes;
#[path = "cmd_find.rs"]
pub mod cmd_find;
#[path = "cmd_find_window.rs"]
pub mod cmd_find_window;
#[path = "cmd_if_shell.rs"]
pub mod cmd_if_shell;
#[path = "cmd_join_pane.rs"]
pub mod cmd_join_pane;
#[path = "cmd_kill_pane.rs"]
pub mod cmd_kill_pane;
#[path = "cmd_kill_server.rs"]
pub mod cmd_kill_server;
#[path = "cmd_kill_session.rs"]
pub mod cmd_kill_session;
#[path = "cmd_kill_window.rs"]
pub mod cmd_kill_window;
#[path = "cmd_list_buffers.rs"]
pub mod cmd_list_buffers;
#[path = "cmd_list_clients.rs"]
pub mod cmd_list_clients;
#[path = "cmd_list_keys.rs"]
pub mod cmd_list_keys;
#[path = "cmd_list_panes.rs"]
pub mod cmd_list_panes;
#[path = "cmd_list_sessions.rs"]
pub mod cmd_list_sessions;
#[path = "cmd_list_windows.rs"]
pub mod cmd_list_windows;
#[path = "cmd_load_buffer.rs"]
pub mod cmd_load_buffer;
#[path = "cmd_lock_server.rs"]
pub mod cmd_lock_server;
#[path = "cmd_move_window.rs"]
pub mod cmd_move_window;
#[path = "cmd_new_session.rs"]
pub mod cmd_new_session;
#[path = "cmd_new_window.rs"]
pub mod cmd_new_window;
#[path = "cmd_paste_buffer.rs"]
pub mod cmd_paste_buffer;
#[path = "cmd_pipe_pane.rs"]
pub mod cmd_pipe_pane;
#[path = "cmd_queue.rs"]
pub mod cmd_queue;
#[path = "cmd_refresh_client.rs"]
pub mod cmd_refresh_client;
#[path = "cmd_rename_session.rs"]
pub mod cmd_rename_session;
#[path = "cmd_rename_window.rs"]
pub mod cmd_rename_window;
#[path = "cmd_resize_pane.rs"]
pub mod cmd_resize_pane;
#[path = "cmd_resize_window.rs"]
pub mod cmd_resize_window;
#[path = "cmd_respawn_pane.rs"]
pub mod cmd_respawn_pane;
#[path = "cmd_respawn_window.rs"]
pub mod cmd_respawn_window;
#[path = "cmd_rotate_window.rs"]
pub mod cmd_rotate_window;
#[path = "cmd_run_shell.rs"]
pub mod cmd_run_shell;
#[path = "cmd_save_buffer.rs"]
pub mod cmd_save_buffer;
#[path = "cmd_select_layout.rs"]
pub mod cmd_select_layout;
#[path = "cmd_select_pane.rs"]
pub mod cmd_select_pane;
#[path = "cmd_select_window.rs"]
pub mod cmd_select_window;
#[path = "cmd_send_keys.rs"]
pub mod cmd_send_keys;
#[path = "cmd_server_access.rs"]
pub mod cmd_server_access;
#[path = "cmd_set_buffer.rs"]
pub mod cmd_set_buffer;
#[path = "cmd_set_environment.rs"]
pub mod cmd_set_environment;
#[path = "cmd_set_option.rs"]
pub mod cmd_set_option;
#[path = "cmd_show_environment.rs"]
pub mod cmd_show_environment;
#[path = "cmd_show_messages.rs"]
pub mod cmd_show_messages;
#[path = "cmd_show_options.rs"]
pub mod cmd_show_options;
#[path = "cmd_show_prompt_history.rs"]
pub mod cmd_show_prompt_history;
#[path = "cmd_source_file.rs"]
pub mod cmd_source_file;
#[path = "cmd_split_window.rs"]
pub mod cmd_split_window;
#[path = "cmd_swap_pane.rs"]
pub mod cmd_swap_pane;
#[path = "cmd_swap_window.rs"]
pub mod cmd_swap_window;
#[path = "cmd_switch_client.rs"]
pub mod cmd_switch_client;
#[path = "cmd_unbind_key.rs"]
pub mod cmd_unbind_key;
#[path = "cmd_wait_for.rs"]
pub mod cmd_wait_for;

use cmd_attach_session::CMD_ATTACH_SESSION_ENTRY;
use cmd_bind_key::CMD_BIND_KEY_ENTRY;
use cmd_break_pane::CMD_BREAK_PANE_ENTRY;
use cmd_capture_pane::{CMD_CAPTURE_PANE_ENTRY, CMD_CLEAR_HISTORY_ENTRY};
use cmd_choose_tree::{
    CMD_CHOOSE_BUFFER_ENTRY, CMD_CHOOSE_CLIENT_ENTRY, CMD_CHOOSE_TREE_ENTRY,
    CMD_CUSTOMIZE_MODE_ENTRY,
};
use cmd_command_prompt::CMD_COMMAND_PROMPT_ENTRY;
use cmd_confirm_before::CMD_CONFIRM_BEFORE_ENTRY;
use cmd_copy_mode::{CMD_CLOCK_MODE_ENTRY, CMD_COPY_MODE_ENTRY};
use cmd_detach_client::CMD_DETACH_CLIENT_ENTRY;
use cmd_detach_client::CMD_SUSPEND_CLIENT_ENTRY;
use cmd_display_menu::{CMD_DISPLAY_MENU_ENTRY, CMD_DISPLAY_POPUP_ENTRY};
use cmd_display_message::CMD_DISPLAY_MESSAGE_ENTRY;
use cmd_display_panes::CMD_DISPLAY_PANES_ENTRY;
use cmd_find_window::CMD_FIND_WINDOW_ENTRY;
use cmd_if_shell::CMD_IF_SHELL_ENTRY;
use cmd_join_pane::{CMD_JOIN_PANE_ENTRY, CMD_MOVE_PANE_ENTRY};
use cmd_kill_pane::CMD_KILL_PANE_ENTRY;
use cmd_kill_server::CMD_KILL_SERVER_ENTRY;
use cmd_kill_server::CMD_START_SERVER_ENTRY;
use cmd_kill_session::CMD_KILL_SESSION_ENTRY;
use cmd_kill_window::CMD_KILL_WINDOW_ENTRY;
use cmd_kill_window::CMD_UNLINK_WINDOW_ENTRY;
use cmd_list_buffers::CMD_LIST_BUFFERS_ENTRY;
use cmd_list_clients::CMD_LIST_CLIENTS_ENTRY;
use cmd_list_keys::{CMD_LIST_COMMANDS_ENTRY, CMD_LIST_KEYS_ENTRY};
use cmd_list_panes::CMD_LIST_PANES_ENTRY;
use cmd_list_sessions::CMD_LIST_SESSIONS_ENTRY;
use cmd_list_windows::CMD_LIST_WINDOWS_ENTRY;
use cmd_load_buffer::CMD_LOAD_BUFFER_ENTRY;
use cmd_lock_server::{CMD_LOCK_CLIENT_ENTRY, CMD_LOCK_SERVER_ENTRY, CMD_LOCK_SESSION_ENTRY};
use cmd_move_window::CMD_LINK_WINDOW_ENTRY;
use cmd_move_window::CMD_MOVE_WINDOW_ENTRY;
use cmd_new_session::CMD_HAS_SESSION_ENTRY;
use cmd_new_session::CMD_NEW_SESSION_ENTRY;
use cmd_new_window::CMD_NEW_WINDOW_ENTRY;
use cmd_paste_buffer::CMD_PASTE_BUFFER_ENTRY;
use cmd_pipe_pane::CMD_PIPE_PANE_ENTRY;
use cmd_refresh_client::CMD_REFRESH_CLIENT_ENTRY;
use cmd_rename_session::CMD_RENAME_SESSION_ENTRY;
use cmd_rename_window::CMD_RENAME_WINDOW_ENTRY;
use cmd_resize_pane::CMD_RESIZE_PANE_ENTRY;
use cmd_resize_window::CMD_RESIZE_WINDOW_ENTRY;
use cmd_respawn_pane::CMD_RESPAWN_PANE_ENTRY;
use cmd_respawn_window::CMD_RESPAWN_WINDOW_ENTRY;
use cmd_rotate_window::CMD_ROTATE_WINDOW_ENTRY;
use cmd_run_shell::CMD_RUN_SHELL_ENTRY;
use cmd_save_buffer::CMD_SAVE_BUFFER_ENTRY;
use cmd_save_buffer::CMD_SHOW_BUFFER_ENTRY;
use cmd_select_layout::CMD_NEXT_LAYOUT_ENTRY;
use cmd_select_layout::CMD_PREVIOUS_LAYOUT_ENTRY;
use cmd_select_layout::CMD_SELECT_LAYOUT_ENTRY;
use cmd_select_pane::CMD_LAST_PANE_ENTRY;
use cmd_select_pane::CMD_SELECT_PANE_ENTRY;
use cmd_select_window::CMD_LAST_WINDOW_ENTRY;
use cmd_select_window::CMD_NEXT_WINDOW_ENTRY;
use cmd_select_window::CMD_PREVIOUS_WINDOW_ENTRY;
use cmd_select_window::CMD_SELECT_WINDOW_ENTRY;
use cmd_send_keys::CMD_SEND_KEYS_ENTRY;
use cmd_send_keys::CMD_SEND_PREFIX_ENTRY;
use cmd_server_access::CMD_SERVER_ACCESS_ENTRY;
use cmd_set_buffer::CMD_DELETE_BUFFER_ENTRY;
use cmd_set_buffer::CMD_SET_BUFFER_ENTRY;
use cmd_set_environment::CMD_SET_ENVIRONMENT_ENTRY;
use cmd_set_option::CMD_SET_HOOK_ENTRY;
use cmd_set_option::CMD_SET_OPTION_ENTRY;
use cmd_set_option::CMD_SET_WINDOW_OPTION_ENTRY;
use cmd_show_environment::CMD_SHOW_ENVIRONMENT_ENTRY;
use cmd_show_messages::CMD_SHOW_MESSAGES_ENTRY;
use cmd_show_options::CMD_SHOW_HOOKS_ENTRY;
use cmd_show_options::CMD_SHOW_OPTIONS_ENTRY;
use cmd_show_options::CMD_SHOW_WINDOW_OPTIONS_ENTRY;
use cmd_show_prompt_history::{CMD_CLEAR_PROMPT_HISTORY_ENTRY, CMD_SHOW_PROMPT_HISTORY_ENTRY};
use cmd_source_file::CMD_SOURCE_FILE_ENTRY;
use cmd_split_window::{CMD_NEW_PANE_ENTRY, CMD_SPLIT_WINDOW_ENTRY};
use cmd_swap_pane::CMD_SWAP_PANE_ENTRY;
use cmd_swap_window::CMD_SWAP_WINDOW_ENTRY;
use cmd_switch_client::CMD_SWITCH_CLIENT_ENTRY;
use cmd_unbind_key::CMD_UNBIND_KEY_ENTRY;
use cmd_wait_for::CMD_WAIT_FOR_ENTRY;

pub static CMD_TABLE: [&cmd_entry; 91] = [
    &CMD_ATTACH_SESSION_ENTRY,
    &CMD_BIND_KEY_ENTRY,
    &CMD_BREAK_PANE_ENTRY,
    &CMD_CAPTURE_PANE_ENTRY,
    &CMD_CHOOSE_BUFFER_ENTRY,
    &CMD_CHOOSE_CLIENT_ENTRY,
    &CMD_CHOOSE_TREE_ENTRY,
    &CMD_CLEAR_HISTORY_ENTRY,
    &CMD_CLEAR_PROMPT_HISTORY_ENTRY,
    &CMD_CLOCK_MODE_ENTRY,
    &CMD_COMMAND_PROMPT_ENTRY,
    &CMD_CONFIRM_BEFORE_ENTRY,
    &CMD_COPY_MODE_ENTRY,
    &CMD_CUSTOMIZE_MODE_ENTRY,
    &CMD_DELETE_BUFFER_ENTRY,
    &CMD_DETACH_CLIENT_ENTRY,
    &CMD_DISPLAY_MENU_ENTRY,
    &CMD_DISPLAY_MESSAGE_ENTRY,
    &CMD_DISPLAY_POPUP_ENTRY,
    &CMD_DISPLAY_PANES_ENTRY,
    &CMD_FIND_WINDOW_ENTRY,
    &CMD_HAS_SESSION_ENTRY,
    &CMD_IF_SHELL_ENTRY,
    &CMD_JOIN_PANE_ENTRY,
    &CMD_KILL_PANE_ENTRY,
    &CMD_KILL_SERVER_ENTRY,
    &CMD_KILL_SESSION_ENTRY,
    &CMD_KILL_WINDOW_ENTRY,
    &CMD_LAST_PANE_ENTRY,
    &CMD_LAST_WINDOW_ENTRY,
    &CMD_LINK_WINDOW_ENTRY,
    &CMD_LIST_BUFFERS_ENTRY,
    &CMD_LIST_CLIENTS_ENTRY,
    &CMD_LIST_COMMANDS_ENTRY,
    &CMD_LIST_KEYS_ENTRY,
    &CMD_LIST_PANES_ENTRY,
    &CMD_LIST_SESSIONS_ENTRY,
    &CMD_LIST_WINDOWS_ENTRY,
    &CMD_LOAD_BUFFER_ENTRY,
    &CMD_LOCK_CLIENT_ENTRY,
    &CMD_LOCK_SERVER_ENTRY,
    &CMD_LOCK_SESSION_ENTRY,
    &CMD_MOVE_PANE_ENTRY,
    &CMD_MOVE_WINDOW_ENTRY,
    &CMD_NEW_PANE_ENTRY,
    &CMD_NEW_SESSION_ENTRY,
    &CMD_NEW_WINDOW_ENTRY,
    &CMD_NEXT_LAYOUT_ENTRY,
    &CMD_NEXT_WINDOW_ENTRY,
    &CMD_PASTE_BUFFER_ENTRY,
    &CMD_PIPE_PANE_ENTRY,
    &CMD_PREVIOUS_LAYOUT_ENTRY,
    &CMD_PREVIOUS_WINDOW_ENTRY,
    &CMD_REFRESH_CLIENT_ENTRY,
    &CMD_RENAME_SESSION_ENTRY,
    &CMD_RENAME_WINDOW_ENTRY,
    &CMD_RESIZE_PANE_ENTRY,
    &CMD_RESIZE_WINDOW_ENTRY,
    &CMD_RESPAWN_PANE_ENTRY,
    &CMD_RESPAWN_WINDOW_ENTRY,
    &CMD_ROTATE_WINDOW_ENTRY,
    &CMD_RUN_SHELL_ENTRY,
    &CMD_SAVE_BUFFER_ENTRY,
    &CMD_SELECT_LAYOUT_ENTRY,
    &CMD_SELECT_PANE_ENTRY,
    &CMD_SELECT_WINDOW_ENTRY,
    &CMD_SEND_KEYS_ENTRY,
    &CMD_SEND_PREFIX_ENTRY,
    &CMD_SERVER_ACCESS_ENTRY,
    &CMD_SET_BUFFER_ENTRY,
    &CMD_SET_ENVIRONMENT_ENTRY,
    &CMD_SET_HOOK_ENTRY,
    &CMD_SET_OPTION_ENTRY,
    &CMD_SET_WINDOW_OPTION_ENTRY,
    &CMD_SHOW_BUFFER_ENTRY,
    &CMD_SHOW_ENVIRONMENT_ENTRY,
    &CMD_SHOW_HOOKS_ENTRY,
    &CMD_SHOW_MESSAGES_ENTRY,
    &CMD_SHOW_OPTIONS_ENTRY,
    &CMD_SHOW_PROMPT_HISTORY_ENTRY,
    &CMD_SHOW_WINDOW_OPTIONS_ENTRY,
    &CMD_SOURCE_FILE_ENTRY,
    &CMD_SPLIT_WINDOW_ENTRY,
    &CMD_START_SERVER_ENTRY,
    &CMD_SUSPEND_CLIENT_ENTRY,
    &CMD_SWAP_PANE_ENTRY,
    &CMD_SWAP_WINDOW_ENTRY,
    &CMD_SWITCH_CLIENT_ENTRY,
    &CMD_UNBIND_KEY_ENTRY,
    &CMD_UNLINK_WINDOW_ENTRY,
    &CMD_WAIT_FOR_ENTRY,
];

// Instance of a command.
#[repr(C)]
pub struct cmd {
    pub entry: &'static cmd_entry,
    pub args: *mut args,
    pub group: u32,
    pub file: *mut u8,
    pub line: u32,

    pub qentry: tailq_entry<cmd>,
}
pub type cmds = tailq_head<cmd>;

pub struct qentry;
impl Entry<cmd, qentry> for cmd {
    unsafe fn entry(this: *mut Self) -> *mut tailq_entry<cmd> {
        unsafe { &raw mut (*this).qentry }
    }
}

/// Next group number for new command list.
static CMD_LIST_NEXT_GROUP: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

macro_rules! cmd_log_argv {
   ($argc:expr, $argv:expr, $fmt:literal $(, $args:expr)* $(,)?) => {
        crate::cmd_::cmd_log_argv_($argc, $argv, format_args!($fmt $(, $args)*))
    };
}
pub(crate) use cmd_log_argv;

// Log an argument vector.
pub unsafe fn cmd_log_argv_(argc: i32, argv: *mut *mut u8, args: std::fmt::Arguments) {
    unsafe {
        let prefix = args.to_string();
        for i in 0..argc {
            log_debug!("{}: argv[{}]{}", prefix, i, _s(*argv.add(i as usize)));
        }
    }
}

/// C `vendor/tmux/cmd.c:272`: `void cmd_append_argv(int *argc, char ***argv, const char *arg)`
pub unsafe fn cmd_append_argv(argc: *mut c_int, argv: *mut *mut *mut u8, arg: *const u8) {
    unsafe {
        *argv = xreallocarray_::<*mut u8>(*argv, (*argc) as usize + 1).as_ptr();
        *(*argv).add((*argc) as usize) = xstrdup(arg).as_ptr();
        *argc += 1;
    }
}

/// C `vendor/tmux/cmd.c:280`: `int cmd_pack_argv(int argc, char **argv, char *buf, size_t len)`
pub unsafe fn cmd_pack_argv(
    argc: c_int,
    argv: *mut *mut u8,
    mut buf: *mut u8,
    mut len: usize,
) -> c_int {
    unsafe {
        //
        if argc == 0 {
            return 0;
        }
        cmd_log_argv!(argc, argv, "cmd_pack_argv");

        *buf = b'\0';
        for i in 0..argc {
            if strlcpy(buf, *argv.add(i as usize), len) >= len {
                return -1;
            }
            let arglen = strlen(*argv.add(i as usize)) + 1;
            buf = buf.add(arglen);
            len -= arglen;
        }

        0
    }
}

/// C `vendor/tmux/cmd.c:303`: `int cmd_unpack_argv(char *buf, size_t len, int argc, char ***argv)`
pub unsafe fn cmd_unpack_argv(
    mut buf: *mut u8,
    mut len: usize,
    argc: c_int,
    argv: *mut *mut *mut u8,
) -> c_int {
    unsafe {
        if argc == 0 {
            return 0;
        }
        *argv = xcalloc_::<*mut u8>(argc as usize).as_ptr();

        *buf.add(len - 1) = b'\0';
        for i in 0..argc {
            if len == 0 {
                cmd_free_argv(argc, *argv);
                return -1;
            }

            let arglen = strlen(buf) + 1;
            *(*argv).add(i as usize) = xstrdup(buf).as_ptr();

            buf = buf.add(arglen);
            len -= arglen;
        }
        cmd_log_argv!(argc, *argv, "cmd_unpack_argv");

        0
    }
}

/// C `vendor/tmux/cmd.c:334`: `char **cmd_copy_argv(int argc, char **argv)`
pub unsafe fn cmd_copy_argv(argc: c_int, argv: *const *mut u8) -> *mut *mut u8 {
    unsafe {
        if argc == 0 {
            return null_mut();
        }
        let new_argv: *mut *mut u8 = xcalloc(argc as usize + 1, size_of::<*mut u8>())
            .cast()
            .as_ptr();
        for i in 0..argc {
            if !(*argv.add(i as usize)).is_null() {
                *new_argv.add(i as usize) = xstrdup(*argv.add(i as usize)).as_ptr();
            }
        }
        new_argv
    }
}

/// C `vendor/tmux/cmd.c:351`: `void cmd_free_argv(int argc, char **argv)`
pub unsafe fn cmd_free_argv(argc: c_int, argv: *mut *mut u8) {
    unsafe {
        if argc == 0 {
            return;
        }
        for i in 0..argc {
            free(*argv.add(i as usize) as _);
        }
        free(argv as _);
    }
}

/// C `vendor/tmux/cmd.c:364`: `char *cmd_stringify_argv(int argc, char **argv)`
pub unsafe fn cmd_stringify_argv(argc: c_int, argv: *mut *mut u8) -> String {
    unsafe {
        if argc == 0 {
            return String::new();
        }

        let mut buf = String::new();
        for i in 0..argc {
            let s = args_escape(*argv.add(i as usize));
            log_debug!(
                "{}: {} {} = {}",
                "cmd_stringify_argv",
                i,
                _s(*argv.add(i as usize)),
                _s(s)
            );

            if i != 0 {
                buf.push(' ');
            }
            buf.push_str(cstr_to_str(s));

            free(s as _);
        }
        buf
    }
}

/// C `vendor/tmux/cmd.c:393`: `const struct cmd_entry *cmd_get_entry(struct cmd *cmd)`
pub unsafe fn cmd_get_entry(cmd: *const cmd) -> &'static cmd_entry {
    unsafe { (*cmd).entry }
}

/// C `vendor/tmux/cmd.c:400`: `struct args *cmd_get_args(struct cmd *cmd)`
pub unsafe fn cmd_get_args(cmd: *mut cmd) -> *mut args {
    unsafe { (*cmd).args }
}

/// C `vendor/tmux/cmd.c:407`: `u_int cmd_get_group(struct cmd *cmd)`
pub unsafe fn cmd_get_group(cmd: *const cmd) -> c_uint {
    unsafe { (*cmd).group }
}

/// C `vendor/tmux/cmd.c:414`: `void cmd_get_source(struct cmd *cmd, const char **file, u_int *line)`
pub unsafe fn cmd_get_source(cmd: *mut cmd, file: *mut *const u8, line: &AtomicU32) {
    unsafe {
        if !file.is_null() {
            *file = (*cmd).file;
        }
        line.store((*cmd).line, std::sync::atomic::Ordering::SeqCst);
    }
}

/// C `vendor/tmux/cmd.c:431`: `char *cmd_get_alias(const char *name)`
pub unsafe fn cmd_get_alias(name: *const u8) -> *mut u8 {
    unsafe {
        let o = options_get_only(GLOBAL_OPTIONS, "command-alias");
        if !o.is_null() {
            let wanted = strlen(name);

            let mut a = options_array_first(o);
            while !a.is_null() {
                let ov = options_array_item_value(a);

                let equals = strchr((*ov).string, b'=' as i32);
                if !equals.is_null() {
                    let n = equals.addr() - (*ov).string.addr();
                    if n == wanted && strncmp(name, (*ov).string, n) == 0 {
                        return xstrdup(equals.add(1)).as_ptr();
                    }
                }

                a = options_array_next(a);
            }
        }

        // ztmux: the original extensions (dashboard, switch, ...) are client
        // subcommands, not server commands - so `: dashboard` / a key binding
        // would otherwise be "unknown command". Make them invocable the same
        // three ways as any tmux command by expanding an unknown extension name
        // into a display-popup that launches `ztmux <name>` against THIS server's
        // own socket. (Real commands and user aliases are matched first above.)
        let name_str = cstr_to_str(name);
        if crate::extensions::EXTENSION_COMMANDS.contains(&name_str) && cmd_find(name_str).is_err() {
            // Point the nested ztmux at THIS server. display-popup runs the
            // command through /bin/sh with $TMUX set, so `${TMUX%%,*}` (the
            // socket path, up to the first comma) is the exact socket string the
            // current client connected with. Use that rather than the server's
            // stored SOCKET_PATH: the two can differ under path canonicalisation
            // (e.g. macOS /tmp vs /private/tmp), which makes the baked absolute
            // path read as "no server". This mirrors the ztmux extension key
            // bindings, so `:name`, a bound key, and `ztmux name` all behave the
            // same. (display-popup does not format-expand its command, so a
            // `#{socket_path}` cannot be used here.)
            let expanded = format!(
                "display-popup -E -w 90% -h 90% 'ztmux -S \"${{TMUX%%,*}}\" {name_str}'\0"
            );
            return xstrdup(expanded.as_bytes().as_ptr()).as_ptr();
        }

        null_mut()
    }
}

/// C `vendor/tmux/cmd.c:462`: `const struct cmd_entry *cmd_find(const char *name, char **cause)`
pub fn cmd_find(name: &str) -> Result<&'static cmd_entry, String> {
    let mut found = None;
    let mut ambiguous: bool = false;

    for entry in CMD_TABLE {
        if entry.alias.is_some_and(|alias| alias == name) {
            ambiguous = false;
            found = Some(entry);
            break;
        }

        if entry.name.starts_with(name) {
            if found.is_some() {
                ambiguous = true;
            }
            found = Some(entry);

            if entry.name == name {
                break;
            }
        }
    }

    if !ambiguous {
        match found {
            Some(value) => {
                log_debug!("cmd_find: {name} found");
                Ok(value)
            }
            None => Err(format!("unknown command: {name}")),
        }
    } else {
        let mut msg = format!("ambiguous command: {name}, could be: ");

        // TODO, once https://github.com/rust-lang/rust/issues/79524 is stabilized rewrite
        for entry in CMD_TABLE {
            if entry.name.starts_with(name) {
                msg.push_str(entry.name);
                msg.push_str(", ");
            }
        }

        // remove last ", "
        msg.truncate(msg.len() - 2);

        Err(msg)
    }
}

/// C `vendor/tmux/cmd.c:512`: `struct cmd *cmd_parse(struct args_value *values, u_int count, const char *file, u_int line, int parse_flags, char **cause)`
pub unsafe fn cmd_parse(
    values: *mut args_value,
    count: c_uint,
    file: Option<&str>,
    line: c_uint,
) -> Result<*mut cmd, String> {
    unsafe {
        let mut error: *mut u8 = null_mut();

        if count == 0 || (*values).type_ != args_type::ARGS_STRING {
            return Err("no command".to_string());
        }
        let entry = cmd_find(cstr_to_str((*values).union_.string))?;

        let args = args_parse(&entry.args, values, count, &raw mut error);
        if args.is_null() && error.is_null() {
            return Err(format!("usage: {} {}", entry.name, entry.usage));
        }
        if args.is_null() {
            let cause = format!("command {}: {}", entry.name, _s(error));
            free(error as _);
            return Err(cause);
        }

        let cmd: *mut cmd = Box::leak(Box::new(cmd {
            entry,
            args,
            group: 0,
            file: null_mut(),
            line: 0,
            qentry: tailq_entry {
                tqe_next: null_mut(),
                tqe_prev: null_mut(),
            },
        }));

        if let Some(file) = file {
            let mut file = file.to_string();
            file.push('\0');
            (*cmd).file = file.leak().as_mut_ptr().cast();
        }
        (*cmd).line = line;

        Ok(cmd)
    }
}

/// C `vendor/tmux/cmd.c:553`: `void cmd_free(struct cmd *cmd)`
pub unsafe fn cmd_free(cmd: *mut cmd) {
    unsafe {
        free((*cmd).file as _);

        args_free((*cmd).args);
        free(cmd as _);
    }
}

/// C `vendor/tmux/cmd.c:563`: `struct cmd *cmd_copy(struct cmd *cmd, int argc, char **argv)`
pub unsafe fn cmd_copy(cmd: *mut cmd, argc: c_int, argv: *mut *mut u8) -> *mut cmd {
    unsafe {
        let new_cmd: *mut cmd = Box::leak(Box::new(cmd {
            entry: (*cmd).entry,
            args: args_copy((*cmd).args, argc, argv),
            group: 0,
            file: null_mut(),
            line: 0,
            qentry: tailq_entry {
                tqe_next: null_mut(),
                tqe_prev: null_mut(),
            },
        }));

        if !(*cmd).file.is_null() {
            (*new_cmd).file = xstrdup((*cmd).file).as_ptr();
        }
        (*new_cmd).line = (*cmd).line;

        new_cmd
    }
}

/// C `vendor/tmux/cmd.c:580`: `char *cmd_print(struct cmd *cmd)`
pub unsafe fn cmd_print(cmd: *mut cmd) -> *mut u8 {
    unsafe {
        let s = args_print((*cmd).args);
        let out = if *s != b'\0' {
            format_nul!("{} {}", (*cmd).entry.name, _s(s))
        } else {
            xstrdup__((*cmd).entry.name)
        };
        free(s as _);

        out
    }
}

/// C `vendor/tmux/cmd.c:596`: `struct cmd_list *cmd_list_new(void)`
pub fn cmd_list_new<'a>() -> &'a mut cmd_list {
    let group = CMD_LIST_NEXT_GROUP.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    let list = Box::leak(Box::new(tailq_head {
        tqh_first: null_mut(),
        tqh_last: null_mut(),
    }));
    tailq_init_(list);

    Box::leak(Box::new(cmd_list {
        references: 1,
        group,
        list: list as _,
    }))
}

/// C `vendor/tmux/cmd.c:610`: `void cmd_list_append(struct cmd_list *cmdlist, struct cmd *cmd)`
pub unsafe fn cmd_list_append(cmdlist: *mut cmd_list, cmd: *mut cmd) {
    unsafe {
        (*cmd).group = (*cmdlist).group;
        tailq_insert_tail::<_, qentry>((*cmdlist).list, cmd);
    }
}

/// C `vendor/tmux/cmd.c:618`: `void cmd_list_append_all(struct cmd_list *cmdlist, struct cmd_list *from)`
pub unsafe fn cmd_list_append_all(cmdlist: *mut cmd_list, from: *mut cmd_list) {
    unsafe {
        for cmd in tailq_foreach::<_, qentry>((*from).list).map(NonNull::as_ptr) {
            (*cmd).group = (*cmdlist).group;
        }
        tailq_concat::<_, qentry>((*cmdlist).list, (*from).list);
    }
}

/// C `vendor/tmux/cmd.c:629`: `void cmd_list_move(struct cmd_list *cmdlist, struct cmd_list *from)`
pub unsafe fn cmd_list_move(cmdlist: *mut cmd_list, from: *mut cmd_list) {
    unsafe {
        tailq_concat::<_, qentry>((*cmdlist).list, (*from).list);
        (*cmdlist).group = CMD_LIST_NEXT_GROUP.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// C `vendor/tmux/cmd.c:637`: `void cmd_list_free(struct cmd_list *cmdlist)`
pub unsafe fn cmd_list_free(cmdlist: *mut cmd_list) {
    unsafe {
        (*cmdlist).references -= 1;
        if (*cmdlist).references != 0 {
            return;
        }

        for cmd in tailq_foreach::<_, qentry>((*cmdlist).list).map(NonNull::as_ptr) {
            tailq_remove::<_, qentry>((*cmdlist).list, cmd);
            cmd_free(cmd);
        }
        free_((*cmdlist).list);
        free_(cmdlist);
    }
}

/// C `vendor/tmux/cmd.c:654`: `struct cmd_list *cmd_list_copy(const struct cmd_list *cmdlist, int argc, char **argv)`
pub unsafe fn cmd_list_copy(
    cmdlist: &cmd_list,
    argc: c_int,
    argv: *mut *mut u8,
) -> *mut cmd_list {
    unsafe {
        let mut group: u32 = cmdlist.group;
        let s = cmd_list_print(cmdlist, 0);
        log_debug!("{}: {}", "cmd_list_copy", _s(s));
        free(s as _);

        let new_cmdlist = cmd_list_new();
        for cmd in tailq_foreach_const(cmdlist.list).map(NonNull::as_ptr) {
            if (*cmd).group != group {
                new_cmdlist.group =
                    CMD_LIST_NEXT_GROUP.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                group = (*cmd).group;
            }
            let new_cmd = cmd_copy(cmd, argc, argv);
            cmd_list_append(new_cmdlist, new_cmd);
        }

        let s = cmd_list_print(new_cmdlist, 0);
        log_debug!("{}: {}", "cmd_list_copy", _s(s));
        free(s as _);

        new_cmdlist
    }
}

/// C `vendor/tmux/cmd.c:685`: `char *cmd_list_print(const struct cmd_list *cmdlist, int flags)`
pub fn cmd_list_print(cmdlist: &cmd_list, escaped: c_int) -> *mut u8 {
    unsafe {
        let mut len = 1;
        let mut buf: *mut u8 = xcalloc(1, len).cast().as_ptr();

        let single_separator = if escaped != 0 { c!(" \\; ") } else { c!(" ; ") };
        let double_separator = if escaped != 0 {
            c!(" \\;\\; ")
        } else {
            c!(" ;; ")
        };

        for cmd in tailq_foreach_const::<_, qentry>(cmdlist.list).map(NonNull::as_ptr) {
            let this = cmd_print(cmd);

            len += strlen(this) + 6;
            buf = xrealloc_(buf, len).as_ptr();

            strlcat(buf, this, len);

            let next = tailq_next::<_, _, qentry>(cmd);
            if !next.is_null() {
                let separator = if (*cmd).group != (*next).group {
                    double_separator
                } else {
                    single_separator
                };
                strlcat(buf, separator, len);
            }

            free_(this);
        }

        buf
    }
}

/// C `vendor/tmux/cmd.c:724`: `struct cmd *cmd_list_first(struct cmd_list *cmdlist)`
pub unsafe fn cmd_list_first(cmdlist: *mut cmd_list) -> *mut cmd {
    unsafe { tailq_first((*cmdlist).list) }
}

/// C `vendor/tmux/cmd.c:731`: `struct cmd *cmd_list_next(struct cmd *cmd)`
pub unsafe fn cmd_list_next(cmd: *mut cmd) -> *mut cmd {
    unsafe { tailq_next::<_, _, qentry>(cmd) }
}

/// C `vendor/tmux/cmd.c:738`: `int cmd_list_all_have(struct cmd_list *cmdlist, int flag)`
pub unsafe fn cmd_list_all_have(cmdlist: *mut cmd_list, flag: cmd_flag) -> bool {
    unsafe {
        tailq_foreach((*cmdlist).list).all(|cmd| (*cmd.as_ptr()).entry.flags.intersects(flag))
    }
}

/// C `vendor/tmux/cmd.c:751`: `int cmd_list_any_have(struct cmd_list *cmdlist, int flag)`
pub unsafe fn cmd_list_any_have(cmdlist: *mut cmd_list, flag: cmd_flag) -> bool {
    unsafe {
        tailq_foreach((*cmdlist).list).any(|cmd| (*cmd.as_ptr()).entry.flags.intersects(flag))
    }
}

/// C `vendor/tmux/cmd.c:764`: `int cmd_mouse_at(struct window_pane *wp, struct mouse_event *m, u_int *xp, u_int *yp, int last)`
pub unsafe fn cmd_mouse_at(
    wp: *mut window_pane,
    m: *mut mouse_event,
    xp: *mut c_uint,
    yp: *mut c_uint,
    last: c_int,
) -> c_int {
    unsafe {
        let x: u32;
        let mut y: u32;

        if last != 0 {
            x = (*m).lx + (*m).ox;
            y = (*m).ly + (*m).oy;
        } else {
            x = (*m).x + (*m).ox;
            y = (*m).y + (*m).oy;
        }
        log_debug!(
            "{}: x={}, y={}{}",
            "cmd_mouse_at",
            x,
            y,
            if last != 0 { " (last)" } else { "" }
        );

        if (*m).statusat == 0 && y >= (*m).statuslines {
            y -= (*m).statuslines;
        }

        if x < (*wp).xoff || x >= (*wp).xoff + (*wp).sx {
            return -1;
        }

        if y < (*wp).yoff || y >= (*wp).yoff + (*wp).sy {
            return -1;
        }

        if !xp.is_null() {
            *xp = x - (*wp).xoff;
        }
        if !yp.is_null() {
            *yp = y - (*wp).yoff;
        }
        0
    }
}

/// C `vendor/tmux/cmd.c:795`: `struct winlink *cmd_mouse_window(struct mouse_event *m, struct session **sp)`
pub unsafe fn cmd_mouse_window(
    m: *mut mouse_event,
    sp: *mut *mut session,
) -> Option<NonNull<winlink>> {
    unsafe {
        let s: *mut session;

        if !(*m).valid {
            return None;
        }
        if (*m).s == -1
            || ({
                s = transmute_ptr(session_find_by_id((*m).s as u32));
                s.is_null()
            })
        {
            return None;
        }
        let wl = if (*m).w == -1 {
            NonNull::new((*s).curw)
        } else {
            let w = window_find_by_id((*m).w as u32);
            if w.is_null() {
                return None;
            }
            winlink_find_by_window(&raw mut (*s).windows, w)
        };
        if !sp.is_null() {
            *sp = s;
        }
        wl
    }
}

/// C `vendor/tmux/cmd.c:819`: `struct window_pane *cmd_mouse_pane(struct mouse_event *m, struct session **sp, struct winlink **wlp)`
pub unsafe fn cmd_mouse_pane(
    m: *mut mouse_event,
    sp: *mut *mut session,
    wlp: *mut *mut winlink,
) -> Option<NonNull<window_pane>> {
    unsafe {
        let wl = cmd_mouse_window(m, sp)?;
        let wp;

        if (*m).wp == -1 {
            wp = NonNull::new((*(*wl.as_ptr()).window).active);
        } else {
            wp = Some(NonNull::new(window_pane_find_by_id((*m).wp as u32))?);
            if !window_has_pane((*wl.as_ptr()).window, wp.unwrap().as_ptr()) {
                return None;
            }
        }

        if !wlp.is_null() {
            *wlp = wl.as_ptr();
        }
        wp
    }
}

/// Replace the first %% or %idx in template by s.
/// C `vendor/tmux/cmd.c:843`: `char *cmd_template_replace(const char *template, const char *s, int idx)`
pub unsafe fn cmd_template_replace(template: *const u8, s: Option<&str>, idx: c_int) -> *mut u8 {
    unsafe {
        let quote = c!("\"\\$;~");

        if strchr(template, b'%' as i32).is_null() {
            return xstrdup(template).as_ptr();
        }

        let mut buf: *mut u8 = xcalloc1::<u8>();
        let mut len = 0;
        let mut replaced = 0;

        let mut ptr = template;
        'outer: while *ptr != b'\0' {
            let ch = *ptr;
            ptr = ptr.add(1);
            'switch: {
                if matches!(ch, b'%') {
                    if *ptr < b'1' || *ptr > b'9' || *ptr as i32 - b'0' as i32 != idx {
                        if *ptr != b'%' || replaced != 0 {
                            break 'switch;
                        }
                        replaced = 1;
                    }
                    ptr = ptr.add(1);

                    let quoted = *ptr == b'%';
                    if quoted {
                        ptr = ptr.add(1);
                    }

                    buf = xrealloc_(buf, len + (s.map(str::len).unwrap_or_default() * 3) + 1)
                        .as_ptr();
                    for c in s.unwrap_or_default().chars() {
                        if quoted && !strchr(quote, c as i32).is_null() {
                            *buf.add(len) = b'\\';
                            len += 1;
                        }
                        *buf.add(len) = c as u8;
                        len += 1;
                    }
                    *buf.add(len) = b'\0';
                    continue 'outer;
                }
            } // 'switch
            buf = xrealloc_(buf, len + 2).as_ptr();
            *buf.add(len) = ch;
            len += 1;
            *buf.add(len) = b'\0';
        }

        buf
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_template_replace() {
        unsafe {
            let out = cmd_template_replace(c"%1".as_ptr().cast(), Some("resize-pane -D 3"), 1);

            let m = libc::strlen(b"resize-pane -D 3\0junk".as_ptr().cast());

            // note the real test is that the return value is properly nul terminated
            let n = libc::strlen(out);

            free_(out);

            assert_eq!(n, m);
        }
    }

    // Build an `argc`-length, heap-allocated argv from the given strings, each
    // entry `xstrdup`'d so it can be released with `cmd_free_argv`. There is no
    // trailing NULL (matching how argv is used by pack/stringify in cmd.c).
    unsafe fn build_argv(args: &[&str]) -> *mut *mut u8 {
        unsafe {
            let argc = args.len();
            let v: *mut *mut u8 = xcalloc(argc, size_of::<*mut u8>()).cast().as_ptr();
            for (i, a) in args.iter().enumerate() {
                let c = CString::new(*a).unwrap();
                *v.add(i) = xstrdup(c.as_ptr().cast()).as_ptr();
            }
            v
        }
    }

    // Read argv entry `i` back as an owned String (must be non-NULL).
    unsafe fn argv_at(argv: *mut *mut u8, i: usize) -> String {
        unsafe { cstr_to_str(*argv.add(i)).to_owned() }
    }

    // cmd.c:364 cmd_stringify_argv: argc == 0 yields an empty (xstrdup("")) string.
    #[test]
    fn stringify_argv_empty() {
        unsafe {
            assert_eq!(cmd_stringify_argv(0, null_mut()), "");
        }
    }

    // cmd.c:364 cmd_stringify_argv joins escaped args with a single space; plain
    // words (no chars in the double/single quote sets) escape to themselves per
    // arguments.c:args_escape.
    #[test]
    fn stringify_argv_plain_words() {
        unsafe {
            let argv = build_argv(&["foo", "bar", "baz"]);
            assert_eq!(cmd_stringify_argv(3, argv), "foo bar baz");
            cmd_free_argv(3, argv);
        }
    }

    // cmd.c:364: an empty argument escapes to '' (arguments.c:args_escape) and the
    // separator is only inserted between (not before) elements.
    #[test]
    fn stringify_argv_empty_and_quoted() {
        unsafe {
            // "" -> '' ; "a b" contains a space so it is wrapped in double quotes.
            let argv = build_argv(&["", "a b"]);
            assert_eq!(cmd_stringify_argv(2, argv), "'' \"a b\"");
            cmd_free_argv(2, argv);
        }
    }

    // cmd.c:334 cmd_copy_argv: argc == 0 returns NULL.
    #[test]
    fn copy_argv_zero_is_null() {
        unsafe {
            assert!(cmd_copy_argv(0, null_mut()).is_null());
        }
    }

    // cmd.c:334 cmd_copy_argv duplicates each string (deep copy) and NUL-terminates
    // the vector (allocates argc + 1). Round-trip contents and free both.
    #[test]
    fn copy_argv_roundtrip() {
        unsafe {
            let argv = build_argv(&["one", "two", "three"]);
            let copy = cmd_copy_argv(3, argv.cast_const());
            assert!(!copy.is_null());
            // Values equal but pointers distinct (deep copy).
            for i in 0..3 {
                assert_eq!(argv_at(copy, i), argv_at(argv, i));
                assert_ne!(*copy.add(i), *argv.add(i));
            }
            // Terminating NULL slot from the argc + 1 allocation.
            assert!((*copy.add(3)).is_null());

            cmd_free_argv(3, argv);
            cmd_free_argv(3, copy);
        }
    }

    // cmd.c:334 cmd_copy_argv leaves NULL entries as NULL (calloc-zeroed slot).
    #[test]
    fn copy_argv_preserves_null_entry() {
        unsafe {
            let argv: *mut *mut u8 = xcalloc(3, size_of::<*mut u8>()).cast().as_ptr();
            let a = CString::new("a").unwrap();
            let c = CString::new("c").unwrap();
            *argv.add(0) = xstrdup(a.as_ptr().cast()).as_ptr();
            *argv.add(1) = null_mut();
            *argv.add(2) = xstrdup(c.as_ptr().cast()).as_ptr();

            let copy = cmd_copy_argv(3, argv.cast_const());
            assert_eq!(argv_at(copy, 0), "a");
            assert!((*copy.add(1)).is_null());
            assert_eq!(argv_at(copy, 2), "c");

            cmd_free_argv(3, argv);
            cmd_free_argv(3, copy);
        }
    }

    // cmd.c:280 cmd_pack_argv + cmd.c:303 cmd_unpack_argv: a packed buffer of
    // NUL-separated args unpacks back to an identical vector.
    #[test]
    fn pack_unpack_roundtrip() {
        unsafe {
            let argv = build_argv(&["alpha", "beta", "gamma"]);
            let mut buf = [0u8; 256];
            assert_eq!(cmd_pack_argv(3, argv, buf.as_mut_ptr(), buf.len()), 0);

            let mut out: *mut *mut u8 = null_mut();
            assert_eq!(
                cmd_unpack_argv(buf.as_mut_ptr(), buf.len(), 3, &raw mut out),
                0
            );
            for i in 0..3 {
                assert_eq!(argv_at(out, i), argv_at(argv, i));
            }
            cmd_free_argv(3, argv);
            cmd_free_argv(3, out);
        }
    }

    // cmd.c:280 / cmd.c:303: argc == 0 is a no-op returning 0 (buffer/argv untouched).
    #[test]
    fn pack_unpack_empty() {
        unsafe {
            let mut buf = [0u8; 8];
            assert_eq!(cmd_pack_argv(0, null_mut(), buf.as_mut_ptr(), buf.len()), 0);

            let mut out: *mut *mut u8 = null_mut();
            assert_eq!(cmd_unpack_argv(buf.as_mut_ptr(), buf.len(), 0, &raw mut out), 0);
            // argc == 0 leaves *argv unmodified (stays NULL here).
            assert!(out.is_null());
        }
    }

    // cmd.c:303 cmd_unpack_argv unpacks a single argument correctly.
    #[test]
    fn pack_unpack_single() {
        unsafe {
            let argv = build_argv(&["solo"]);
            let mut buf = [0u8; 64];
            assert_eq!(cmd_pack_argv(1, argv, buf.as_mut_ptr(), buf.len()), 0);

            let mut out: *mut *mut u8 = null_mut();
            assert_eq!(cmd_unpack_argv(buf.as_mut_ptr(), buf.len(), 1, &raw mut out), 0);
            assert_eq!(argv_at(out, 0), "solo");

            cmd_free_argv(1, argv);
            cmd_free_argv(1, out);
        }
    }

    // cmd.c:280 cmd_pack_argv returns -1 when an argument does not fit in the
    // remaining buffer (strlcpy return >= len).
    #[test]
    fn pack_argv_buffer_too_small() {
        unsafe {
            let argv = build_argv(&["foobar"]);
            let mut buf = [0u8; 3];
            assert_eq!(cmd_pack_argv(1, argv, buf.as_mut_ptr(), buf.len()), -1);
            cmd_free_argv(1, argv);
        }
    }

    // cmd.c:303 cmd_unpack_argv returns -1 when the buffer runs out before all
    // args are consumed (the `len == 0` guard inside the loop).
    #[test]
    fn unpack_argv_truncated_buffer() {
        unsafe {
            // Pack two args into a large buffer, then unpack with a len that only
            // covers the first arg ("foo\0" is 4 bytes) so the 2nd iteration hits
            // len == 0 and returns -1.
            let argv = build_argv(&["foo", "bar"]);
            let mut buf = [0u8; 64];
            assert_eq!(cmd_pack_argv(2, argv, buf.as_mut_ptr(), buf.len()), 0);

            let mut out: *mut *mut u8 = null_mut();
            assert_eq!(cmd_unpack_argv(buf.as_mut_ptr(), 4, 2, &raw mut out), -1);
            cmd_free_argv(2, argv);
        }
    }

    // cmd.c:272 cmd_append_argv appends to the tail, growing argc; building a
    // vector from empty (NULL) yields the args in order.
    #[test]
    fn append_argv_builds_vector() {
        unsafe {
            let mut argv: *mut *mut u8 = null_mut();
            let mut argc: c_int = 0;

            let a = CString::new("first").unwrap();
            let b = CString::new("second").unwrap();
            cmd_append_argv(&raw mut argc, &raw mut argv, a.as_ptr().cast());
            cmd_append_argv(&raw mut argc, &raw mut argv, b.as_ptr().cast());

            assert_eq!(argc, 2);
            assert_eq!(argv_at(argv, 0), "first");
            assert_eq!(argv_at(argv, 1), "second");

            cmd_free_argv(argc, argv);
        }
    }

    // cmd.c:272 cmd_append_argv output survives a pack/unpack round-trip,
    // exercising the append path end-to-end.
    #[test]
    fn append_then_pack_unpack() {
        unsafe {
            let mut argv: *mut *mut u8 = null_mut();
            let mut argc: c_int = 0;
            for w in ["red", "green", "blue"] {
                let c = CString::new(w).unwrap();
                cmd_append_argv(&raw mut argc, &raw mut argv, c.as_ptr().cast());
            }

            let mut buf = [0u8; 128];
            assert_eq!(cmd_pack_argv(argc, argv, buf.as_mut_ptr(), buf.len()), 0);

            let mut out: *mut *mut u8 = null_mut();
            assert_eq!(cmd_unpack_argv(buf.as_mut_ptr(), buf.len(), argc, &raw mut out), 0);
            assert_eq!(argv_at(out, 0), "red");
            assert_eq!(argv_at(out, 1), "green");
            assert_eq!(argv_at(out, 2), "blue");

            cmd_free_argv(argc, argv);
            cmd_free_argv(argc, out);
        }
    }

    // cmd.c:462 cmd_find: an exact name match returns that entry. "new-window" is
    // a full name in cmd_table.
    #[test]
    fn cmd_find_exact_name() {
        let e = cmd_find("new-window").unwrap();
        assert_eq!(e.name, "new-window");
    }

    // cmd.c:462 cmd_find: a unique prefix of a single command name resolves to it
    // (no other name starts with "attach-s").
    #[test]
    fn cmd_find_unique_prefix() {
        let e = cmd_find("attach-s").unwrap();
        assert_eq!(e.name, "attach-session");
    }

    // cmd.c:462 cmd_find: an exact name that is also a prefix of nothing longer
    // still resolves (the `strcmp == 0` break in the C loop).
    #[test]
    fn cmd_find_exact_kill_window() {
        assert_eq!(cmd_find("kill-window").unwrap().name, "kill-window");
    }

    // cmd.c:462 cmd_find: the alias branch (`entry->alias != NULL && strcmp == 0`)
    // matches exactly and short-circuits. "neww" is the alias for new-window,
    // "splitw" for split-window, "lsw" for list-windows.
    #[test]
    fn cmd_find_alias_match() {
        assert_eq!(cmd_find("neww").unwrap().name, "new-window");
        assert_eq!(cmd_find("splitw").unwrap().name, "split-window");
        assert_eq!(cmd_find("lsw").unwrap().name, "list-windows");
    }

    // cmd.c:462 cmd_find: "send" is the alias for send-keys; the alias check wins
    // even though both send-keys and send-prefix share the "send" name prefix.
    #[test]
    fn cmd_find_alias_beats_prefix() {
        assert_eq!(cmd_find("send").unwrap().name, "send-keys");
    }

    // cmd.c:462 cmd_find: an unknown name yields "unknown command: <name>".
    #[test]
    fn cmd_find_unknown() {
        let err = cmd_find("definitely-not-real").err().unwrap();
        assert_eq!(err, "unknown command: definitely-not-real");
    }

    // cmd.c:462 cmd_find: an ambiguous prefix (several names start with "list-")
    // yields an "ambiguous command" error listing the candidates.
    #[test]
    fn cmd_find_ambiguous_prefix() {
        let err = cmd_find("list-").err().unwrap();
        assert!(
            err.starts_with("ambiguous command: list-, could be: "),
            "unexpected: {err:?}"
        );
        // Candidates include at least list-buffers and list-windows.
        assert!(err.contains("list-buffers"));
        assert!(err.contains("list-windows"));
        // The trailing ", " is trimmed (cmd.c:505: s[strlen(s) - 2] = '\0').
        assert!(!err.ends_with(", "));
    }

    // cmd.c:462 cmd_find: the empty string is a prefix of every command name, so
    // it matches the first entry, then the second sets ambiguous. Empty input is
    // therefore ambiguous, not "unknown".
    #[test]
    fn cmd_find_empty_is_ambiguous() {
        let err = cmd_find("").err().unwrap();
        assert!(err.starts_with("ambiguous command: "), "unexpected: {err:?}");
    }

    // cmd.c:843 cmd_template_replace: no '%' in the template returns a copy of the
    // template unchanged (the strchr NULL fast path).
    #[test]
    fn template_replace_no_percent() {
        unsafe {
            let out = cmd_template_replace(c"no markers here".as_ptr().cast(), Some("X"), 1);
            assert_eq!(cstr_to_str(out), "no markers here");
            free_(out);
        }
    }

    // cmd.c:843 cmd_template_replace: %1 is replaced by s when idx == 1; a %2 with
    // idx == 1 does not match (`*ptr - '0' != idx`) and stays literal.
    #[test]
    fn template_replace_indexed() {
        unsafe {
            let out = cmd_template_replace(c"%1 and %2".as_ptr().cast(), Some("VAL"), 1);
            assert_eq!(cstr_to_str(out), "VAL and %2");
            free_(out);
        }
    }

    // cmd.c:843 cmd_template_replace: a %-digit that does not match idx and is not
    // %% is emitted literally (the `break` falls through to the default case).
    #[test]
    fn template_replace_nonmatching_digit_literal() {
        unsafe {
            let out = cmd_template_replace(c"%3".as_ptr().cast(), Some("VAL"), 1);
            assert_eq!(cstr_to_str(out), "%3");
            free_(out);
        }
    }

    // cmd.c:843 cmd_template_replace: %% is replaced by s exactly once (replaced
    // flag); a second %% stays literal.
    #[test]
    fn template_replace_double_percent_once() {
        unsafe {
            let out = cmd_template_replace(c"%% %%".as_ptr().cast(), Some("Z"), 1);
            assert_eq!(cstr_to_str(out), "Z %%");
            free_(out);
        }
    }

    // cmd.c:843 cmd_template_replace: without the trailing '%' quote marker, the
    // substituted text is inserted verbatim (no backslash escaping).
    #[test]
    fn template_replace_unquoted_no_escape() {
        unsafe {
            let out = cmd_template_replace(c"%1".as_ptr().cast(), Some("a;b"), 1);
            assert_eq!(cstr_to_str(out), "a;b");
            free_(out);
        }
    }

    // cmd.c:843 cmd_template_replace: a '%' immediately after the index digit is
    // the quote marker; then chars in the quote set ("\"\\$;~") are backslash
    // escaped in the substitution. ';' is in the set, so "a;b" -> "a\;b".
    #[test]
    fn template_replace_quoted_escapes() {
        unsafe {
            let out = cmd_template_replace(c"%1%".as_ptr().cast(), Some("a;b"), 1);
            assert_eq!(cstr_to_str(out), "a\\;b");
            free_(out);
        }
    }

    // cmd.c:364 cmd_stringify_argv: a single argument is returned with no
    // separator.
    #[test]
    fn stringify_argv_single() {
        unsafe {
            let argv = build_argv(&["solo"]);
            assert_eq!(cmd_stringify_argv(1, argv), "solo");
            cmd_free_argv(1, argv);
        }
    }

    // cmd.c:280 cmd_pack_argv: an argument that exactly fills the buffer (its
    // bytes plus the NUL) still packs (strlcpy return == len - 1 < len).
    #[test]
    fn pack_argv_exact_fit() {
        unsafe {
            let argv = build_argv(&["abc"]);
            // "abc" needs 4 bytes (3 + NUL); a 4-byte buffer is the exact fit.
            let mut buf = [0u8; 4];
            assert_eq!(cmd_pack_argv(1, argv, buf.as_mut_ptr(), buf.len()), 0);
            let mut out: *mut *mut u8 = null_mut();
            assert_eq!(cmd_unpack_argv(buf.as_mut_ptr(), buf.len(), 1, &raw mut out), 0);
            assert_eq!(argv_at(out, 0), "abc");
            cmd_free_argv(1, argv);
            cmd_free_argv(1, out);
        }
    }

    // cmd.c:334 cmd_copy_argv of a single-element vector produces an independent
    // deep copy that still round-trips through pack/unpack.
    #[test]
    fn copy_argv_single_deep() {
        unsafe {
            let argv = build_argv(&["only"]);
            let copy = cmd_copy_argv(1, argv.cast_const());
            assert_eq!(argv_at(copy, 0), "only");
            assert_ne!(*copy, *argv);
            assert!((*copy.add(1)).is_null());
            cmd_free_argv(1, argv);
            cmd_free_argv(1, copy);
        }
    }

    // cmd.c:843 cmd_template_replace: an index other than 1 matches %<idx>. %2 with
    // idx == 2 is substituted.
    #[test]
    fn template_replace_index_two() {
        unsafe {
            let out = cmd_template_replace(c"%2".as_ptr().cast(), Some("X"), 2);
            assert_eq!(cstr_to_str(out), "X");
            free_(out);
        }
    }

    // cmd.c:843 cmd_template_replace: literal text surrounding the marker is copied
    // verbatim around the substitution.
    #[test]
    fn template_replace_surrounding_text() {
        unsafe {
            let out = cmd_template_replace(c"x%1y".as_ptr().cast(), Some("Z"), 1);
            assert_eq!(cstr_to_str(out), "xZy");
            free_(out);
        }
    }

    // cmd.c:843 cmd_template_replace: %0 is not a valid marker ('0' < '1'), and the
    // following char is not '%', so both characters are emitted literally.
    #[test]
    fn template_replace_percent_zero_literal() {
        unsafe {
            let out = cmd_template_replace(c"%0".as_ptr().cast(), Some("Z"), 1);
            assert_eq!(cstr_to_str(out), "%0");
            free_(out);
        }
    }

    // cmd.c:843 cmd_template_replace: a trailing '%' with nothing after it is not a
    // marker (the following byte is NUL, not a digit or '%'), so it stays literal.
    #[test]
    fn template_replace_trailing_percent_literal() {
        unsafe {
            let out = cmd_template_replace(c"50%".as_ptr().cast(), Some("Z"), 1);
            assert_eq!(cstr_to_str(out), "50%");
            free_(out);
        }
    }

    // cmd.c:843 cmd_template_replace: with the '%' quote marker, a '$' (in the quote
    // set "\"\\$;~") is backslash-escaped in the substitution: "a$b" -> "a\$b".
    #[test]
    fn template_replace_quoted_dollar() {
        unsafe {
            let out = cmd_template_replace(c"%1%".as_ptr().cast(), Some("a$b"), 1);
            assert_eq!(cstr_to_str(out), "a\\$b");
            free_(out);
        }
    }

    // cmd.c:364 cmd_stringify_argv: a value containing a bare double quote (no
    // double-quote-set char) is single-quoted by args_escape (arguments.c:606).
    #[test]
    fn stringify_argv_single_quoted() {
        unsafe {
            let argv = build_argv(&["a\"b"]);
            assert_eq!(cmd_stringify_argv(1, argv), "'a\"b'");
            cmd_free_argv(1, argv);
        }
    }

    // cmd.c:462 cmd_find: "kill-s" is a prefix of both kill-server and kill-session,
    // so it is ambiguous and the error lists both.
    #[test]
    fn cmd_find_ambiguous_kill_s() {
        let err = cmd_find("kill-s").err().unwrap();
        assert!(
            err.starts_with("ambiguous command: kill-s, could be: "),
            "unexpected: {err:?}"
        );
        assert!(err.contains("kill-server"));
        assert!(err.contains("kill-session"));
    }

    // cmd.c:462 cmd_find: "kill-ser" narrows the kill-server/kill-session ambiguity
    // to a single command.
    #[test]
    fn cmd_find_unique_kill_server() {
        assert_eq!(cmd_find("kill-ser").unwrap().name, "kill-server");
    }

    // cmd.c:462 cmd_find: "rename-" is a prefix of rename-session and rename-window,
    // so it is ambiguous.
    #[test]
    fn cmd_find_ambiguous_rename() {
        let err = cmd_find("rename-").err().unwrap();
        assert!(err.starts_with("ambiguous command: rename-"), "unexpected: {err:?}");
        assert!(err.contains("rename-session"));
        assert!(err.contains("rename-window"));
    }

    // cmd.c:462 cmd_find: "new-s" matches only new-session (new-window does not
    // share the "new-s" prefix), so it resolves uniquely.
    #[test]
    fn cmd_find_unique_new_session() {
        assert_eq!(cmd_find("new-s").unwrap().name, "new-session");
    }
}
