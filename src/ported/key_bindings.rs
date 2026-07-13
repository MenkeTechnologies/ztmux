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

macro_rules! DEFAULT_SESSION_MENU {
    () => {
        concat!(
            " 'Next' 'n' {switch-client -n}",
            " 'Previous' 'p' {switch-client -p}",
            " ''",
            " 'Renumber' 'N' {move-window -r}",
            " 'Rename' 'n' {command-prompt -I \"#S\" {rename-session -- '%%'}}",
            " ''",
            " 'New Session' 's' {new-session}",
            " 'New Window' 'w' {new-window}"
        )
    };
}

macro_rules! DEFAULT_WINDOW_MENU {
    () => {
        concat!(
            " '#{?#{>:#{session_windows},1},,-}Swap Left' 'l' {swap-window -t:-1}",
            " '#{?#{>:#{session_windows},1},,-}Swap Right' 'r' {swap-window -t:+1}",
            " '#{?pane_marked_set,,-}Swap Marked' 's' {swap-window}",
            " ''",
            " 'Kill' 'X' {kill-window}",
            " 'Respawn' 'R' {respawn-window -k}",
            " '#{?pane_marked,Unmark,Mark}' 'm' {select-pane -m}",
            " 'Rename' 'n' {command-prompt -FI \"#W\" {rename-window -t '#{window_id}' -- '%%'}}",
            " ''",
            " 'New After' 'w' {new-window -a}",
            " 'New At End' 'W' {new-window}"
        )
    };
}

macro_rules! DEFAULT_PANE_MENU {
    () => {
        concat!(
            " '#{?#{m/r:(copy|view)-mode,#{pane_mode}},Go To Top,}' '<' {send -X history-top}",
            " '#{?#{m/r:(copy|view)-mode,#{pane_mode}},Go To Bottom,}' '>' {send -X history-bottom}",
            " ''",
            " '#{?mouse_word,Search For #[underscore]#{=/9/...:mouse_word},}' 'C-r' {if -F '#{?#{m/r:(copy|view)-mode,#{pane_mode}},0,1}' 'copy-mode -t='; send -Xt= search-backward \"#{q:mouse_word}\"}",
            " '#{?mouse_word,Type #[underscore]#{=/9/...:mouse_word},}' 'C-y' {copy-mode -q; send-keys -l -- \"#{q:mouse_word}\"}",
            " '#{?mouse_word,Copy #[underscore]#{=/9/...:mouse_word},}' 'c' {copy-mode -q; set-buffer -- \"#{q:mouse_word}\"}",
            " '#{?mouse_line,Copy Line,}' 'l' {copy-mode -q; set-buffer -- \"#{q:mouse_line}\"}",
            " ''",
            " '#{?mouse_hyperlink,Type #[underscore]#{=/9/...:mouse_hyperlink},}' 'C-h' {copy-mode -q; send-keys -l -- \"#{q:mouse_hyperlink}\"}",
            " '#{?mouse_hyperlink,Copy #[underscore]#{=/9/...:mouse_hyperlink},}' 'h' {copy-mode -q; set-buffer -- \"#{q:mouse_hyperlink}\"}",
            " ''",
            " 'Horizontal Split' 'h' {split-window -h}",
            " 'Vertical Split' 'v' {split-window -v}",
            " ''",
            " '#{?#{>:#{window_panes},1},,-}Swap Up' 'u' {swap-pane -U}",
            " '#{?#{>:#{window_panes},1},,-}Swap Down' 'd' {swap-pane -D}",
            " '#{?pane_marked_set,,-}Swap Marked' 's' {swap-pane}",
            " ''",
            " 'Kill' 'X' {kill-pane}",
            " 'Respawn' 'R' {respawn-pane -k}",
            " '#{?pane_marked,Unmark,Mark}' 'm' {select-pane -m}",
            " '#{?#{>:#{window_panes},1},,-}#{?window_zoomed_flag,Unzoom,Zoom}' 'z' {resize-pane -Z}",
            // ztmux originals: edit this pane's scrollback in $EDITOR, and the
            // zellij-style multi-pane sync marks (mark panes, then sync the set).
            // Reachable from the menu regardless of how the user's config has
            // rebound the `e`/`m`/`M` keys.
            " ''",
            " 'Edit Scrollback in $EDITOR' 'e' {capture-pane -S - -b ztmux-scrollback ; save-buffer -b ztmux-scrollback /tmp/ztmux-scrollback.txt ; delete-buffer -b ztmux-scrollback ; display-popup -E -w 90% -h 90% 'exec ${EDITOR:-${VISUAL:-vi}} /tmp/ztmux-scrollback.txt'}",
            " '#{?@ztmux_sel,Deselect This Pane,Select This Pane for Sync}' 'g' {set -pF @ztmux_sel '#{?@ztmux_sel,,1}'}",
            " 'Sync Selected Panes' 'y' {run-shell \"ztmux -S #{socket_path} pick sync\"}",
            " '#{?synchronize-panes,,-}Unsync All Panes' 'U' {run-shell \"ztmux -S #{socket_path} pick clear\"}",
            " ''",
            " '#{?@ztmux-stacked,Unstack Panes,Stack Panes (zellij)}' 'k' {if -F '#{@ztmux-stacked}' {set -uw @ztmux-stacked ; select-layout even-vertical} {set -w @ztmux-stacked 1 ; select-layout even-vertical ; resize-pane -y 999}}",
            " '#{?@ztmux-tab-bar,Hide Tab Bar,Tab Bar (zellij)}' 'T' {run-shell \"ztmux -S #{socket_path} tabs toggle\"}",
            " 'Floating Pane (zellij)' 'f' {if -F '#{==:#{session_name},_ztmux_float}' {detach-client} {display-popup -E -w 80% -h 70% -T ' floating pane (prefix C-f to close) ' 'ztmux -S \"${TMUX%%,*}\" new-session -A -s _ztmux_float'}}",
            " 'Open URL / Path from Pane' 'o' {display-popup -E -w 80% -h 60% 'ztmux -S \"${TMUX%%,*}\" open'}",
        )
    };
}

RB_GENERATE!(
    key_bindings,
    key_binding,
    entry,
    discr_entry,
    key_bindings_cmp
);
RB_GENERATE!(key_tables, key_table, entry, discr_entry, key_table_cmp);
static mut KEY_TABLES: key_tables = rb_initializer();

/// C `vendor/tmux/key-bindings.c:81`: `static int key_table_cmp(struct key_table *table1, struct key_table *table2)`
pub fn key_table_cmp(table1: &key_table, table2: &key_table) -> cmp::Ordering {
    unsafe { i32_to_ordering(strcmp(table1.name_ptr(), table2.name_ptr())) }
}

impl key_table {
    /// Borrowed `char *` to the (always present) table name.
    #[inline]
    pub(crate) fn name_ptr(&self) -> *const u8 {
        self.name.as_ptr().cast()
    }
}

/// C `vendor/tmux/key-bindings.c:87`: `static int key_bindings_cmp(struct key_binding *bd1, struct key_binding *bd2)`
pub fn key_bindings_cmp(bd1: &key_binding, bd2: &key_binding) -> cmp::Ordering {
    bd1.key.cmp(&bd2.key)
}

impl key_binding {
    /// Borrowed `char *` to the note, or NULL when unset (C `bd->note == NULL`).
    #[inline]
    pub(crate) fn note_ptr(&self) -> *const u8 {
        match &self.note {
            Some(n) => n.as_ptr().cast(),
            None => std::ptr::null(),
        }
    }
}

/// C `vendor/tmux/key-bindings.c:97`: `static void key_bindings_free(struct key_binding *bd)`
pub unsafe fn key_bindings_free(bd: *mut key_binding) {
    unsafe {
        cmd_list_free((*bd).cmdlist);
        // Reclaim the boxed binding; its owned `note` CString drops with it.
        drop(Box::from_raw(bd));
    }
}

/// C `vendor/tmux/key-bindings.c:105`: `struct key_table *key_bindings_get_table(const char *name, int create)`
pub unsafe fn key_bindings_get_table(name: *const u8, create: bool) -> *mut key_table {
    unsafe {
        // Look up by a borrowed key (no throwaway node): strcmp(name, node.name).
        let table = rb_find_by(&raw mut KEY_TABLES, |t: &key_table| {
            i32_to_ordering(strcmp(name, t.name_ptr()))
        });
        if !table.is_null() || !create {
            return table;
        }

        let table = Box::into_raw(Box::new(key_table {
            name: std::ffi::CStr::from_ptr(name.cast()).to_owned(),
            activity_time: timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            key_bindings: rb_initializer(),
            default_key_bindings: rb_initializer(),
            references: 1, /* one reference in key_tables */
            entry: rb_entry::default(),
        }));
        rb_insert(&raw mut KEY_TABLES, table);

        table
    }
}

/// C `vendor/tmux/key-bindings.c:126`: `struct key_table *key_bindings_first_table(void)`
pub unsafe fn key_bindings_first_table() -> *mut key_table {
    unsafe { rb_min(&raw mut KEY_TABLES) }
}

/// C `vendor/tmux/key-bindings.c:132`: `struct key_table *key_bindings_next_table(struct key_table *table)`
pub unsafe fn key_bindings_next_table(table: *mut key_table) -> *mut key_table {
    unsafe { rb_next(table) }
}

/// C `vendor/tmux/key-bindings.c:138`: `void key_bindings_unref_table(struct key_table *table)`
pub unsafe fn key_bindings_unref_table(table: *mut key_table) {
    unsafe {
        (*table).references -= 1;
        if (*table).references != 0 {
            return;
        }

        for bd in rb_foreach(&raw mut (*table).key_bindings).map(NonNull::as_ptr) {
            rb_remove(&raw mut (*table).key_bindings, bd);
            key_bindings_free(bd);
        }
        for bd in rb_foreach(&raw mut (*table).default_key_bindings).map(NonNull::as_ptr) {
            rb_remove(&raw mut (*table).default_key_bindings, bd);
            key_bindings_free(bd);
        }

        // Reclaim the boxed table; its owned `name` CString drops with it.
        drop(Box::from_raw(table));
    }
}

/// C `vendor/tmux/key-bindings.c:160`: `struct key_binding *key_bindings_get(struct key_table *table, key_code key)`
pub unsafe fn key_bindings_get(table: NonNull<key_table>, key: key_code) -> *mut key_binding {
    unsafe {
        // C fabricates a stack key_binding as the RB_FIND key; key_binding owns a note,
        // so garbage there is not a value it can hold. Search by key instead.
        rb_find_by(&raw mut (*table.as_ptr()).key_bindings, |bd| key.cmp(&bd.key))
    }
}

/// C `vendor/tmux/key-bindings.c:169`: `struct key_binding *key_bindings_get_default(struct key_table *table, key_code key)`
pub unsafe fn key_bindings_get_default(table: *mut key_table, key: key_code) -> *mut key_binding {
    unsafe {
        rb_find_by(&raw mut (*table).default_key_bindings, |bd| key.cmp(&bd.key))
    }
}

/// C `vendor/tmux/key-bindings.c:178`: `struct key_binding *key_bindings_first(struct key_table *table)`
pub unsafe fn key_bindings_first(table: *mut key_table) -> *mut key_binding {
    unsafe { rb_min(&raw mut (*table).key_bindings) }
}

/// C `vendor/tmux/key-bindings.c:184`: `struct key_binding *key_bindings_next(__unused struct key_table *table, struct key_binding *bd)`
pub unsafe fn key_bindings_next(_table: *mut key_table, bd: *mut key_binding) -> *mut key_binding {
    unsafe { rb_next(bd) }
}

/// C `vendor/tmux/key-bindings.c:190`: `void key_bindings_add(const char *name, key_code key, const char *note, int repeat, struct cmd_list *cmdlist)`
pub unsafe fn key_bindings_add(
    name: *const u8,
    key: key_code,
    note: *const u8,
    repeat: bool,
    cmdlist: *mut cmd_list,
) {
    unsafe {
        let table = key_bindings_get_table(name, true);

        let mut bd = key_bindings_get(NonNull::new(table).unwrap(), key & !KEYC_MASK_FLAGS);
        if cmdlist.is_null() {
            if !bd.is_null() {
                // C key-bindings.c:200-208: only replace the note when a new one
                // is given (leave the existing note otherwise), and honour repeat.
                if !note.is_null() {
                    // Assigning drops the old note CString — no manual free.
                    (*bd).note = Some(std::ffi::CStr::from_ptr(note.cast()).to_owned());
                }
                if repeat {
                    (*bd).flags |= KEY_BINDING_REPEAT;
                }
            }
            return;
        }
        if !bd.is_null() {
            rb_remove(&raw mut (*table).key_bindings, bd);
            key_bindings_free(bd);
        }

        bd = Box::into_raw(Box::new(key_binding {
            key: key & !KEYC_MASK_FLAGS,
            cmdlist: null_mut(),
            note: if note.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(note.cast()).to_owned())
            },
            flags: 0,
            entry: zeroed(),
        }));
        rb_insert(&raw mut (*table).key_bindings, bd);

        if repeat {
            (*bd).flags |= KEY_BINDING_REPEAT;
        }
        (*bd).cmdlist = cmdlist;

        let s = cmd_list_print(&*(*bd).cmdlist, 0);
        log_debug!(
            "{}: {:#x} {} = {}",
            "key_bindings_add",
            (*bd).key,
            _s(key_string_lookup_key((*bd).key, 1)),
            _s(s),
        );
        free_(s);
    }
}

/// C `vendor/tmux/key-bindings.c:234`: `void key_bindings_remove(const char *name, key_code key)`
pub unsafe fn key_bindings_remove(name: *const u8, key: key_code) {
    unsafe {
        let Some(table) = NonNull::new(key_bindings_get_table(name, false)) else {
            return;
        };

        let bd = key_bindings_get(table, key & !KEYC_MASK_FLAGS);
        if bd.is_null() {
            return;
        }

        log_debug!(
            "{}: {:#x} {}",
            "key_bindings_remove",
            (*bd).key,
            _s(key_string_lookup_key((*bd).key, 1)),
        );

        rb_remove(&raw mut (*table.as_ptr()).key_bindings, bd);
        key_bindings_free(bd);

        if rb_empty(&raw mut (*table.as_ptr()).key_bindings)
            && rb_empty(&raw mut (*table.as_ptr()).default_key_bindings)
        {
            rb_remove(&raw mut KEY_TABLES, table.as_ptr());
            key_bindings_unref_table(table.as_ptr());
        }
    }
}

/// C `vendor/tmux/key-bindings.c:261`: `void key_bindings_reset(const char *name, key_code key)`
pub unsafe fn key_bindings_reset(name: *const u8, key: key_code) {
    unsafe {
        let Some(table) = NonNull::new(key_bindings_get_table(name, false)) else {
            return;
        };

        let bd = key_bindings_get(table, key & !KEYC_MASK_FLAGS);
        if bd.is_null() {
            return;
        }

        let dd = key_bindings_get_default(table.as_ptr(), (*bd).key);
        if dd.is_null() {
            key_bindings_remove(name, (*bd).key);
            return;
        }

        cmd_list_free((*bd).cmdlist);
        (*bd).cmdlist = (*dd).cmdlist;
        (*(*bd).cmdlist).references += 1;

        // Clone dd's note (or None); assigning drops bd's old note — no free.
        (*bd).note.clone_from(&(*dd).note);
        (*bd).flags = (*dd).flags;
    }
}

/// C `vendor/tmux/key-bindings.c:293`: `void key_bindings_remove_table(const char *name)`
pub unsafe fn key_bindings_remove_table(name: *const u8) {
    unsafe {
        let table = key_bindings_get_table(name, false);
        if !table.is_null() {
            rb_remove(&raw mut KEY_TABLES, table);
            key_bindings_unref_table(table);
        }
        for c in crate::compat::queue::tailq_foreach(&raw mut CLIENTS).map(NonNull::as_ptr) {
            if (*c).keytable == table {
                server_client_set_key_table(c, null_mut());
            }
        }
    }
}

#[expect(dead_code)]
/// C `vendor/tmux/key-bindings.c:310`: `void key_bindings_reset_table(const char *name)`
unsafe fn key_bindings_reset_table(name: *const u8) {
    unsafe {
        let table = key_bindings_get_table(name, false);
        if table.is_null() {
            return;
        }
        if rb_empty(&raw mut (*table).default_key_bindings) {
            key_bindings_remove_table(name);
            return;
        }
        for bd in rb_foreach(&raw mut (*table).key_bindings).map(NonNull::as_ptr) {
            key_bindings_reset(name, (*bd).key);
        }
    }
}

/// C `vendor/tmux/key-bindings.c:327`: `static enum cmd_retval key_bindings_init_done(__unused struct cmdq_item *item, __unused void *data)`
unsafe fn key_bindings_init_done(_item: *mut cmdq_item, _data: *mut c_void) -> cmd_retval {
    unsafe {
        for table in rb_foreach(&raw mut KEY_TABLES).map(NonNull::as_ptr) {
            for bd in rb_foreach(&raw mut (*table).key_bindings).map(NonNull::as_ptr) {
                let new_bd = Box::into_raw(Box::new(key_binding {
                    key: (*bd).key,
                    cmdlist: (*bd).cmdlist,
                    note: (*bd).note.clone(),
                    flags: (*bd).flags,
                    entry: zeroed(),
                }));
                (*(*new_bd).cmdlist).references += 1;
                rb_insert(&raw mut (*table).default_key_bindings, new_bd);
            }
        }
    }

    cmd_retval::CMD_RETURN_NORMAL
}

/// C `vendor/tmux/key-bindings.c:349`: `void key_bindings_init(void)`
pub unsafe fn key_bindings_init() {
    #[rustfmt::skip]
    static DEFAULTS: [&str; 263] = [
        // Prefix keys.
        "bind -N 'Send the prefix key' C-b { send-prefix }",
        "bind -N 'Rotate through the panes' C-o { rotate-window }",
        "bind -N 'Suspend the current client' C-z { suspend-client }",
        "bind -N 'Select next layout' Space { next-layout }",
        "bind -N 'Break pane to a new window' ! { break-pane }",
        "bind -N 'Split window vertically' '\"' { split-window }",
        "bind -N 'List all paste buffers' '#' { list-buffers }",
        "bind -N 'Rename current session' '$' { command-prompt -I'#S' { rename-session -- '%%' } }",
        "bind -N 'Split window horizontally' % { split-window -h }",
        "bind -N 'Kill current window' & { confirm-before -p\"kill-window #W? (y/n)\" kill-window }",
        "bind -N 'Prompt for window index to select' \"'\" { command-prompt -T window-target -pindex { select-window -t ':%%' } }",
        "bind -N 'Switch to previous client' ( { switch-client -p }",
        "bind -N 'Switch to next client' ) { switch-client -n }",
        "bind -N 'Rename current window' , { command-prompt -I'#W' { rename-window -- '%%' } }",
        "bind -N 'Delete the most recent paste buffer' - { delete-buffer }",
        "bind -N 'Move the current window' . { command-prompt -T target { move-window -t '%%' } }",
        "bind -N 'Describe key binding' '/' { command-prompt -kpkey  { list-keys -1N '%%' } }",
        "bind -N 'Select window 0' 0 { select-window -t:=0 }",
        "bind -N 'Select window 1' 1 { select-window -t:=1 }",
        "bind -N 'Select window 2' 2 { select-window -t:=2 }",
        "bind -N 'Select window 3' 3 { select-window -t:=3 }",
        "bind -N 'Select window 4' 4 { select-window -t:=4 }",
        "bind -N 'Select window 5' 5 { select-window -t:=5 }",
        "bind -N 'Select window 6' 6 { select-window -t:=6 }",
        "bind -N 'Select window 7' 7 { select-window -t:=7 }",
        "bind -N 'Select window 8' 8 { select-window -t:=8 }",
        "bind -N 'Select window 9' 9 { select-window -t:=9 }",
        "bind -N 'Prompt for a command' : { command-prompt }",
        "bind -N 'Move to the previously active pane' \\; { last-pane }",
        "bind -N 'Choose a paste buffer from a list' = { choose-buffer -Z }",
        "bind -N 'List key bindings' ? { list-keys -N }",
        "bind -N 'Choose and detach a client from a list' D { choose-client -Z }",
        "bind -N 'Spread panes out evenly' E { select-layout -E }",
        "bind -N 'Switch to the last client' L { switch-client -l }",
        "bind -N 'Clear the marked pane' M { select-pane -M }",
        "bind -N 'Enter copy mode' [ { copy-mode }",
        "bind -N 'Paste the most recent paste buffer' ] { paste-buffer -p }",
        "bind -N 'Create a new window' c { new-window }",
        "bind -N 'Detach the current client' d { detach-client }",
        "bind -N 'Search for a pane' f { command-prompt { find-window -Z -- '%%' } }",
        "bind -N 'Display window information' i { display-message }",
        "bind -N 'Select the previously current window' l { last-window }",
        "bind -N 'Toggle the marked pane' m { select-pane -m }",
        "bind -N 'Select the next window' n { next-window }",
        "bind -N 'Select the next pane' o { select-pane -t:.+ }",
        "bind -N 'Customize options' C { customize-mode -Z }",
        "bind -N 'Select the previous window' p { previous-window }",
        "bind -N 'Display pane numbers' q { display-panes }",
        "bind -N 'Redraw the current client' r { refresh-client }",
        "bind -N 'Choose a session from a list' s { choose-tree -Zs }",
        "bind -N 'Show a clock' t { clock-mode }",
        "bind -N 'Choose a window from a list' w { choose-tree -Zw }",
        "bind -N 'Kill the active pane' x { confirm-before -p\"kill-pane #P? (y/n)\" kill-pane }",
        "bind -N 'Zoom the active pane' z { resize-pane -Z }",
        "bind -N 'Swap the active pane with the pane above' '{' { swap-pane -U }",
        "bind -N 'Swap the active pane with the pane below' '}' { swap-pane -D }",
        "bind -N 'Show messages' '~' { show-messages }",
        "bind -N 'Enter copy mode and scroll up' PPage { copy-mode -u }",
        "bind -N 'Select the pane above the active pane' -r Up { select-pane -U }",
        "bind -N 'Select the pane below the active pane' -r Down { select-pane -D }",
        "bind -N 'Select the pane to the left of the active pane' -r Left { select-pane -L }",
        "bind -N 'Select the pane to the right of the active pane' -r Right { select-pane -R }",
        "bind -N 'Set the even-horizontal layout' M-1 { select-layout even-horizontal }",
        "bind -N 'Set the even-vertical layout' M-2 { select-layout even-vertical }",
        "bind -N 'Set the main-horizontal layout' M-3 { select-layout main-horizontal }",
        "bind -N 'Set the main-vertical layout' M-4 { select-layout main-vertical }",
        "bind -N 'Select the tiled layout' M-5 { select-layout tiled }",
        "bind -N 'Set the main-horizontal-mirrored layout' M-6 { select-layout main-horizontal-mirrored }",
        "bind -N 'Set the main-vertical-mirrored layout' M-7 { select-layout main-vertical-mirrored }",
        "bind -N 'Select the next window with an alert' M-n { next-window -a }",
        "bind -N 'Rotate through the panes in reverse' M-o { rotate-window -D }",
        "bind -N 'Select the previous window with an alert' M-p { previous-window -a }",
        "bind -N 'Move the visible part of the window up' -r S-Up { refresh-client -U 10 }",
        "bind -N 'Move the visible part of the window down' -r S-Down { refresh-client -D 10 }",
        "bind -N 'Move the visible part of the window left' -r S-Left { refresh-client -L 10 }",
        "bind -N 'Move the visible part of the window right' -r S-Right { refresh-client -R 10 }",
        "bind -N 'Reset so the visible part of the window follows the cursor' -r DC { refresh-client -c }",
        "bind -N 'Resize the pane up by 5' -r M-Up { resize-pane -U 5 }",
        "bind -N 'Resize the pane down by 5' -r M-Down { resize-pane -D 5 }",
        "bind -N 'Resize the pane left by 5' -r M-Left { resize-pane -L 5 }",
        "bind -N 'Resize the pane right by 5' -r M-Right { resize-pane -R 5 }",
        "bind -N 'Resize the pane up' -r C-Up { resize-pane -U }",
        "bind -N 'Resize the pane down' -r C-Down { resize-pane -D }",
        "bind -N 'Resize the pane left' -r C-Left { resize-pane -L }",
        "bind -N 'Resize the pane right' -r C-Right { resize-pane -R }",
        /* Menu keys */
        concat!( "bind < { display-menu -xW -yW -T '#[align=centre]#{window_index}:#{window_name}' ", DEFAULT_WINDOW_MENU!(), " }"),
        concat!( "bind > { display-menu -xP -yP -T '#[align=centre]#{pane_index} ", "(#{pane_id})' ", DEFAULT_PANE_MENU!(), " }"),
        // Mouse button 1 down on pane.
        "bind -n MouseDown1Pane { select-pane -t=; send -M }",
        /* Mouse button 1 drag on pane. */
        "bind -n MouseDrag1Pane { if -F '#{||:#{pane_in_mode},#{mouse_any_flag}}' { send -M } { copy-mode -M } }",
        /* Mouse wheel up on pane. */
        "bind -n WheelUpPane { if -F '#{||:#{pane_in_mode},#{mouse_any_flag}}' { send -M } { copy-mode -e } }",
        /* Mouse button 2 down on pane. */
        "bind -n MouseDown2Pane { select-pane -t=; if -F '#{||:#{pane_in_mode},#{mouse_any_flag}}' { send -M } { paste -p } }",
        /* Mouse button 1 double click on pane. */
        "bind -n DoubleClick1Pane { select-pane -t=; if -F '#{||:#{pane_in_mode},#{mouse_any_flag}}' { send -M } { copy-mode -H; send -X select-word; run -d0.3; send -X copy-pipe-and-cancel } }",
        /* Mouse button 1 triple click on pane. */
        "bind -n TripleClick1Pane { select-pane -t=; if -F '#{||:#{pane_in_mode},#{mouse_any_flag}}' { send -M } { copy-mode -H; send -X select-line; run -d0.3; send -X copy-pipe-and-cancel } }",
        /* Mouse button 1 drag on border. */
        "bind -n MouseDrag1Border { resize-pane -M }",
        /* ztmux: right-click a pane BORDER for the pane context menu. Bound to
         * the border (not the pane body) so a TUI running inside the pane keeps
         * all its own right-click/mouse events - the menu never shadows it. `-O`
         * keeps the menu open on release (click-to-select, no hold-and-drag). */
        concat!("bind -n MouseDown3Border { display-menu -O -t= -xM -yM -T '#[align=centre]#{pane_index} (#{pane_id})' ", DEFAULT_PANE_MENU!(), " }"),
        /* Mouse button 1 down on status line. */
        "bind -n MouseDown1Status { select-window -t= }",
        /* Mouse wheel down on status line. */
        "bind -n WheelDownStatus { next-window }",
        /* Mouse wheel up on status line. */
        "bind -n WheelUpStatus { previous-window }",
        /* Mouse button 3 down on status left. */
        concat!("bind -n MouseDown3StatusLeft { display-menu -O -t= -xM -yW -T '#[align=centre]#{session_name}' ", DEFAULT_SESSION_MENU!(), " }"),
        concat!("bind -n M-MouseDown3StatusLeft { display-menu -O -t= -xM -yW -T '#[align=centre]#{session_name}' ", DEFAULT_SESSION_MENU!(), " }"),
        /* Mouse button 3 down on status line. */
        concat!( "bind -n MouseDown3Status { display-menu -O -t= -xW -yW -T '#[align=centre]#{window_index}:#{window_name}' ", DEFAULT_WINDOW_MENU!(), "}"),
        concat!( "bind -n M-MouseDown3Status { display-menu -O -t= -xW -yW -T '#[align=centre]#{window_index}:#{window_name}' ", DEFAULT_WINDOW_MENU!(), "}"),
        /* Mouse button 3 down on pane. */
        concat!( "bind -n MouseDown3Pane { if -Ft= '#{||:#{mouse_any_flag},#{&&:#{pane_in_mode},#{?#{m/r:(copy|view)-mode,#{pane_mode}},0,1}}}' { select-pane -t=; send -M } { display-menu -t= -xM -yM -T '#[align=centre]#{pane_index} (#{pane_id})' ", DEFAULT_PANE_MENU!(), " } }"),
        concat!( "bind -n M-MouseDown3Pane { display-menu -t= -xM -yM -T '#[align=centre]#{pane_index} (#{pane_id})' ", DEFAULT_PANE_MENU!(), " }"),
        /* Copy mode (emacs) keys. */
        "bind -Tcopy-mode C-Space { send -X begin-selection }",
        "bind -Tcopy-mode C-a { send -X start-of-line }",
        "bind -Tcopy-mode C-c { send -X cancel }",
        "bind -Tcopy-mode C-e { send -X end-of-line }",
        "bind -Tcopy-mode C-f { send -X cursor-right }",
        "bind -Tcopy-mode C-b { send -X cursor-left }",
        "bind -Tcopy-mode C-g { send -X clear-selection }",
        "bind -Tcopy-mode C-k { send -X copy-pipe-end-of-line-and-cancel }",
        "bind -Tcopy-mode C-n { send -X cursor-down }",
        "bind -Tcopy-mode C-p { send -X cursor-up }",
        "bind -Tcopy-mode C-r { command-prompt -T search -ip'(search up)' -I'#{pane_search_string}' { send -X search-backward-incremental '%%' } }",
        "bind -Tcopy-mode C-s { command-prompt -T search -ip'(search down)' -I'#{pane_search_string}' { send -X search-forward-incremental '%%' } }",
        "bind -Tcopy-mode C-v { send -X page-down }",
        "bind -Tcopy-mode C-w { send -X copy-pipe-and-cancel }",
        "bind -Tcopy-mode Escape { send -X cancel }",
        "bind -Tcopy-mode Space { send -X page-down }",
        "bind -Tcopy-mode , { send -X jump-reverse }",
        "bind -Tcopy-mode \\; { send -X jump-again }",
        "bind -Tcopy-mode F { command-prompt -1p'(jump backward)' { send -X jump-backward '%%' } }",
        "bind -Tcopy-mode N { send -X search-reverse }",
        "bind -Tcopy-mode P { send -X toggle-position }",
        "bind -Tcopy-mode R { send -X rectangle-toggle }",
        "bind -Tcopy-mode T { command-prompt -1p'(jump to backward)' { send -X jump-to-backward '%%' } }",
        "bind -Tcopy-mode X { send -X set-mark }",
        "bind -Tcopy-mode f { command-prompt -1p'(jump forward)' { send -X jump-forward '%%' } }",
        "bind -Tcopy-mode g { command-prompt -p'(goto line)' { send -X goto-line '%%' } }",
        "bind -Tcopy-mode n { send -X search-again }",
        "bind -Tcopy-mode q { send -X cancel }",
        "bind -Tcopy-mode r { send -X refresh-from-pane }",
        "bind -Tcopy-mode t { command-prompt -1p'(jump to forward)' { send -X jump-to-forward '%%' } }",
        "bind -Tcopy-mode Home { send -X start-of-line }",
        "bind -Tcopy-mode End { send -X end-of-line }",
        "bind -Tcopy-mode MouseDown1Pane select-pane",
        "bind -Tcopy-mode MouseDrag1Pane { select-pane; send -X begin-selection }",
        "bind -Tcopy-mode MouseDragEnd1Pane { send -X copy-pipe-and-cancel }",
        "bind -Tcopy-mode WheelUpPane { select-pane; send -N5 -X scroll-up }",
        "bind -Tcopy-mode WheelDownPane { select-pane; send -N5 -X scroll-down }",
        "bind -Tcopy-mode DoubleClick1Pane { select-pane; send -X select-word; run -d0.3; send -X copy-pipe-and-cancel }",
        "bind -Tcopy-mode TripleClick1Pane { select-pane; send -X select-line; run -d0.3; send -X copy-pipe-and-cancel }",
        "bind -Tcopy-mode NPage { send -X page-down }",
        "bind -Tcopy-mode PPage { send -X page-up }",
        "bind -Tcopy-mode Up { send -X cursor-up }",
        "bind -Tcopy-mode Down { send -X cursor-down }",
        "bind -Tcopy-mode Left { send -X cursor-left }",
        "bind -Tcopy-mode Right { send -X cursor-right }",
        "bind -Tcopy-mode M-1 { command-prompt -Np'(repeat)' -I1 { send -N '%%' } }",
        "bind -Tcopy-mode M-2 { command-prompt -Np'(repeat)' -I2 { send -N '%%' } }",
        "bind -Tcopy-mode M-3 { command-prompt -Np'(repeat)' -I3 { send -N '%%' } }",
        "bind -Tcopy-mode M-4 { command-prompt -Np'(repeat)' -I4 { send -N '%%' } }",
        "bind -Tcopy-mode M-5 { command-prompt -Np'(repeat)' -I5 { send -N '%%' } }",
        "bind -Tcopy-mode M-6 { command-prompt -Np'(repeat)' -I6 { send -N '%%' } }",
        "bind -Tcopy-mode M-7 { command-prompt -Np'(repeat)' -I7 { send -N '%%' } }",
        "bind -Tcopy-mode M-8 { command-prompt -Np'(repeat)' -I8 { send -N '%%' } }",
        "bind -Tcopy-mode M-9 { command-prompt -Np'(repeat)' -I9 { send -N '%%' } }",
        "bind -Tcopy-mode M-< { send -X history-top }",
        "bind -Tcopy-mode M-> { send -X history-bottom }",
        "bind -Tcopy-mode M-R { send -X top-line }",
        "bind -Tcopy-mode M-b { send -X previous-word }",
        "bind -Tcopy-mode C-M-b { send -X previous-matching-bracket }",
        "bind -Tcopy-mode M-f { send -X next-word-end }",
        "bind -Tcopy-mode C-M-f { send -X next-matching-bracket }",
        "bind -Tcopy-mode M-m { send -X back-to-indentation }",
        "bind -Tcopy-mode M-r { send -X middle-line }",
        "bind -Tcopy-mode M-v { send -X page-up }",
        "bind -Tcopy-mode M-w { send -X copy-pipe-and-cancel }",
        "bind -Tcopy-mode M-x { send -X jump-to-mark }",
        "bind -Tcopy-mode 'M-{' { send -X previous-paragraph }",
        "bind -Tcopy-mode 'M-}' { send -X next-paragraph }",
        "bind -Tcopy-mode M-Up { send -X halfpage-up }",
        "bind -Tcopy-mode M-Down { send -X halfpage-down }",
        "bind -Tcopy-mode C-Up { send -X scroll-up }",
        "bind -Tcopy-mode C-Down { send -X scroll-down }",
        /* Copy mode (vi) keys. */
        "bind -Tcopy-mode-vi '#' { send -FX search-backward '#{copy_cursor_word}' }",
        "bind -Tcopy-mode-vi * { send -FX search-forward '#{copy_cursor_word}' }",
        "bind -Tcopy-mode-vi C-c { send -X cancel }",
        "bind -Tcopy-mode-vi C-d { send -X halfpage-down }",
        "bind -Tcopy-mode-vi C-e { send -X scroll-down }",
        "bind -Tcopy-mode-vi C-b { send -X page-up }",
        "bind -Tcopy-mode-vi C-f { send -X page-down }",
        "bind -Tcopy-mode-vi C-h { send -X cursor-left }",
        "bind -Tcopy-mode-vi C-j { send -X copy-pipe-and-cancel }",
        "bind -Tcopy-mode-vi Enter { send -X copy-pipe-and-cancel }",
        "bind -Tcopy-mode-vi C-u { send -X halfpage-up }",
        "bind -Tcopy-mode-vi C-v { send -X rectangle-toggle }",
        "bind -Tcopy-mode-vi C-y { send -X scroll-up }",
        "bind -Tcopy-mode-vi Escape { send -X clear-selection }",
        "bind -Tcopy-mode-vi Space { send -X begin-selection }",
        "bind -Tcopy-mode-vi '$' { send -X end-of-line }",
        "bind -Tcopy-mode-vi , { send -X jump-reverse }",
        "bind -Tcopy-mode-vi / { command-prompt -T search -p'(search down)' { send -X search-forward '%%' } }",
        "bind -Tcopy-mode-vi 0 { send -X start-of-line }",
        "bind -Tcopy-mode-vi 1 { command-prompt -Np'(repeat)' -I1 { send -N '%%' } }",
        "bind -Tcopy-mode-vi 2 { command-prompt -Np'(repeat)' -I2 { send -N '%%' } }",
        "bind -Tcopy-mode-vi 3 { command-prompt -Np'(repeat)' -I3 { send -N '%%' } }",
        "bind -Tcopy-mode-vi 4 { command-prompt -Np'(repeat)' -I4 { send -N '%%' } }",
        "bind -Tcopy-mode-vi 5 { command-prompt -Np'(repeat)' -I5 { send -N '%%' } }",
        "bind -Tcopy-mode-vi 6 { command-prompt -Np'(repeat)' -I6 { send -N '%%' } }",
        "bind -Tcopy-mode-vi 7 { command-prompt -Np'(repeat)' -I7 { send -N '%%' } }",
        "bind -Tcopy-mode-vi 8 { command-prompt -Np'(repeat)' -I8 { send -N '%%' } }",
        "bind -Tcopy-mode-vi 9 { command-prompt -Np'(repeat)' -I9 { send -N '%%' } }",
        "bind -Tcopy-mode-vi : { command-prompt -p'(goto line)' { send -X goto-line '%%' } }",
        "bind -Tcopy-mode-vi \\; { send -X jump-again }",
        "bind -Tcopy-mode-vi ? { command-prompt -T search -p'(search up)' { send -X search-backward '%%' } }",
        "bind -Tcopy-mode-vi A { send -X append-selection-and-cancel }",
        "bind -Tcopy-mode-vi B { send -X previous-space }",
        "bind -Tcopy-mode-vi D { send -X copy-pipe-end-of-line-and-cancel }",
        "bind -Tcopy-mode-vi E { send -X next-space-end }",
        "bind -Tcopy-mode-vi F { command-prompt -1p'(jump backward)' { send -X jump-backward '%%' } }",
        "bind -Tcopy-mode-vi G { send -X history-bottom }",
        "bind -Tcopy-mode-vi H { send -X top-line }",
        "bind -Tcopy-mode-vi J { send -X scroll-down }",
        "bind -Tcopy-mode-vi K { send -X scroll-up }",
        "bind -Tcopy-mode-vi L { send -X bottom-line }",
        "bind -Tcopy-mode-vi M { send -X middle-line }",
        "bind -Tcopy-mode-vi N { send -X search-reverse }",
        "bind -Tcopy-mode-vi P { send -X toggle-position }",
        "bind -Tcopy-mode-vi T { command-prompt -1p'(jump to backward)' { send -X jump-to-backward '%%' } }",
        "bind -Tcopy-mode-vi V { send -X select-line }",
        "bind -Tcopy-mode-vi W { send -X next-space }",
        "bind -Tcopy-mode-vi X { send -X set-mark }",
        "bind -Tcopy-mode-vi ^ { send -X back-to-indentation }",
        "bind -Tcopy-mode-vi b { send -X previous-word }",
        "bind -Tcopy-mode-vi e { send -X next-word-end }",
        "bind -Tcopy-mode-vi f { command-prompt -1p'(jump forward)' { send -X jump-forward '%%' } }",
        "bind -Tcopy-mode-vi g { send -X history-top }",
        "bind -Tcopy-mode-vi h { send -X cursor-left }",
        "bind -Tcopy-mode-vi j { send -X cursor-down }",
        "bind -Tcopy-mode-vi k { send -X cursor-up }",
        "bind -Tcopy-mode-vi z { send -X scroll-middle }",
        "bind -Tcopy-mode-vi l { send -X cursor-right }",
        "bind -Tcopy-mode-vi n { send -X search-again }",
        "bind -Tcopy-mode-vi o { send -X other-end }",
        "bind -Tcopy-mode-vi q { send -X cancel }",
        "bind -Tcopy-mode-vi r { send -X refresh-from-pane }",
        "bind -Tcopy-mode-vi t { command-prompt -1p'(jump to forward)' { send -X jump-to-forward '%%' } }",
        "bind -Tcopy-mode-vi v { send -X rectangle-toggle }",
        "bind -Tcopy-mode-vi w { send -X next-word }",
        "bind -Tcopy-mode-vi '{' { send -X previous-paragraph }",
        "bind -Tcopy-mode-vi '}' { send -X next-paragraph }",
        "bind -Tcopy-mode-vi % { send -X next-matching-bracket }",
        "bind -Tcopy-mode-vi Home { send -X start-of-line }",
        "bind -Tcopy-mode-vi End { send -X end-of-line }",
        "bind -Tcopy-mode-vi MouseDown1Pane { select-pane }",
        "bind -Tcopy-mode-vi MouseDrag1Pane { select-pane; send -X begin-selection }",
        "bind -Tcopy-mode-vi MouseDragEnd1Pane { send -X copy-pipe-and-cancel }",
        "bind -Tcopy-mode-vi WheelUpPane { select-pane; send -N5 -X scroll-up }",
        "bind -Tcopy-mode-vi WheelDownPane { select-pane; send -N5 -X scroll-down }",
        "bind -Tcopy-mode-vi DoubleClick1Pane { select-pane; send -X select-word; run -d0.3; send -X copy-pipe-and-cancel }",
        "bind -Tcopy-mode-vi TripleClick1Pane { select-pane; send -X select-line; run -d0.3; send -X copy-pipe-and-cancel }",
        "bind -Tcopy-mode-vi BSpace { send -X cursor-left }",
        "bind -Tcopy-mode-vi NPage { send -X page-down }",
        "bind -Tcopy-mode-vi PPage { send -X page-up }",
        "bind -Tcopy-mode-vi Up { send -X cursor-up }",
        "bind -Tcopy-mode-vi Down { send -X cursor-down }",
        "bind -Tcopy-mode-vi Left { send -X cursor-left }",
        "bind -Tcopy-mode-vi Right { send -X cursor-right }",
        "bind -Tcopy-mode-vi M-x { send -X jump-to-mark }",
        "bind -Tcopy-mode-vi C-Up { send -X scroll-up }",
        "bind -Tcopy-mode-vi C-Down { send -X scroll-down }",
    ];

    // ztmux extension bindings — NOT part of the tmux C port. The `DEFAULTS`
    // table above stays byte-identical to `vendor/tmux/key-bindings.c`; these
    // extra prefix keys launch the original client subcommands in src/extensions
    // via display-popup (which gives them the TTY a ratatui/one-shot tool needs).
    // Keys chosen from those left unbound by the default prefix table:
    //   C-d dashboard · S switch · T tree · H doctor (health) · W watch ·
    //   I stats (info) · G graph. The streaming `events` extension runs forever
    //   and is pipe-oriented, so it is intentionally left unbound.
    //
    // The popup command is run through `/bin/sh -c` (see job_run), so we point
    // the nested ztmux at the *same* server via `-S "${TMUX%%,*}"` — the socket
    // path is the part of $TMUX before the first comma. That is robust for
    // non-default sockets (a bare `ztmux` would otherwise use the default
    // socket); display-popup does not format-expand its command, so `#{...}`
    // cannot be used here. If $TMUX is unset the empty -S falls back to default.
    // A few of these are not extension launchers but native ztmux bindings
    // (scrollback-to-$EDITOR, the multi-pane selection). They may use `#{...}`
    // formats freely; only the `display-popup` *command* above cannot.
    #[rustfmt::skip]
    static ZTMUX_EXTENSION_BINDINGS: [&str; 15] = [
        "bind -N 'ztmux: live server dashboard' C-d { display-popup -E -w 90% -h 90% 'ztmux -S \"${TMUX%%,*}\" dashboard' }",
        // Zellij-style floating pane: a persistent pane that floats above the
        // tiled layout in a popup. It lives in a hidden `_ztmux_float` holding
        // session (state kept between toggles); `new-session -A` attaches it or
        // creates it on first use. Pressing the key again *inside* the float
        // (its session name matches) detaches, closing the popup. So `prefix C-f`
        // both opens and closes it.
        "bind -N 'ztmux: toggle floating pane' C-f { if -F '#{==:#{session_name},_ztmux_float}' { detach-client } { display-popup -E -w 80% -h 70% -T ' floating pane (prefix C-f to close) ' 'ztmux -S \"${TMUX%%,*}\" new-session -A -s _ztmux_float' } }",
        "bind -N 'ztmux: session/window/pane picker' S { display-popup -E -w 80% -h 70% 'ztmux -S \"${TMUX%%,*}\" switcher' }",
        "bind -N 'ztmux: server tree' T { display-popup -E -w 80% -h 80% 'ztmux -S \"${TMUX%%,*}\" tree | less -R' }",
        "bind -N 'ztmux: environment/server health check' H { display-popup -E -w 80% -h 80% 'ztmux -S \"${TMUX%%,*}\" doctor | less -R' }",
        "bind -N 'ztmux: live process monitor' W { display-popup -E -w 90% -h 90% 'ztmux -S \"${TMUX%%,*}\" watch' }",
        "bind -N 'ztmux: server stats' I { display-popup -E -w 80% -h 80% 'ztmux -S \"${TMUX%%,*}\" stats | less -R' }",
        "bind -N 'ztmux: server graph' G { display-popup -E -w 80% -h 80% 'ztmux -S \"${TMUX%%,*}\" graph | less -R' }",
        // Edit this pane's full scrollback in $EDITOR. Captured at bind time (so
        // the active pane is the source, before the popup opens), dumped to a
        // file, then opened in $EDITOR inside a popup.
        "bind -N 'ztmux: edit this pane scrollback in $EDITOR' e { capture-pane -S - -b ztmux-scrollback ; save-buffer -b ztmux-scrollback /tmp/ztmux-scrollback.txt ; delete-buffer -b ztmux-scrollback ; display-popup -E -w 90% -h 90% 'exec ${EDITOR:-${VISUAL:-vi}} /tmp/ztmux-scrollback.txt' }",
        // Multi-pane sync selection. Kept OFF the native `m`/`M` marked-pane
        // bindings (those stay tmux's select-pane -m/-M for swap): our select
        // lives on `C-s`, and `M` syncs the whole selection.
        //   prefix C-s -> select/deselect THIS pane (selections persist)
        //   prefix M   -> sync all selected panes (then the selection clears)
        // The pane border menu also exposes select / sync / clear.
        "bind -N 'ztmux: select/deselect this pane for sync' C-s { set -pF @ztmux_sel '#{?@ztmux_sel,,1}' ; display-message 'pane #{pane_index} #{?@ztmux_sel,\u{2713} selected for sync,deselected}' }",
        "bind -N 'ztmux: sync all selected panes' M { run-shell 'ztmux -S \"${TMUX%%,*}\" pick sync' ; display-message 'synced all selected panes' }",
        // Inline trigger wizard: chain four command-prompts (name, pane glob,
        // match regex, action) straight into `triggers add` - no JSON editing.
        "bind -N 'ztmux: add a content-trigger (inline wizard)' R { command-prompt -p 'trigger name:,pane glob (*):,match regex:,action:' { run-shell \"ztmux -S '#{socket_path}' triggers add '%1' '%2' '%3' '%4'\" } }",
        // Zellij-style pane stacks (the reference port lives in
        // src/extensions/stack.rs, reachable as `ztmux stack` / `:stack`). The
        // geometry is realised in pure tmux here — equalise the column then grow
        // the active pane to full height (resize-pane -y 999), which squeezes the
        // rest to 1-row title bars, matching zellij. It is done inline rather than
        // shelling out to `ztmux stack` because these run from a hook / key
        // binding: a run-shell subprocess would connect back into the server
        // mid-command (reentrant) and its nested queries fail. `prefix +` toggles
        // the stack; the window-pane-changed hook re-collapses on focus change so
        // navigating expands the newly-focused pane. Both guard on @ztmux-stacked
        // so unstacked windows are untouched.
        "set-hook -ga window-pane-changed { if -F '#{@ztmux-stacked}' 'select-layout even-vertical ; resize-pane -y 999' }",
        "bind -N 'ztmux: toggle zellij pane stack' + { if -F '#{@ztmux-stacked}' { set -uw @ztmux-stacked ; select-layout even-vertical } { set -w @ztmux-stacked 1 ; select-layout even-vertical ; resize-pane -y 999 } }",
        // Continuum-style session persistence: when `@ztmux-resurrect-auto on`,
        // the first client to attach spawns a detached background daemon that
        // re-saves the whole server every 15 minutes (`ztmux resurrect autosave`,
        // pidfile-guarded so re-attaching never starts a second). If
        // `@ztmux-resurrect-restore on` too, that daemon also restores the last
        // snapshot once on start. No-op unless the option is set.
        "set-hook -ga client-attached { if -F '#{@ztmux-resurrect-auto}' 'run-shell -b \"ztmux -S #{socket_path} resurrect autosave\"' }",
    ];

    unsafe {
        for &default in DEFAULTS.iter().chain(ZTMUX_EXTENSION_BINDINGS.iter()) {
            match cmd_parse_from_string(default, None) {
                Err(error) => {
                    log_debug!("{}", _s(error));
                    fatalx_!("bad default key: {}", default);
                }
                Ok(cmdlist) => {
                    cmdq_append(null_mut(), cmdq_get_command(cmdlist, null_mut()));
                    cmd_list_free(cmdlist);
                }
            }
        }
        cmdq_append(
            null_mut(),
            cmdq_get_callback!(key_bindings_init_done, null_mut()).as_ptr(),
        );
    }
}

/// C `vendor/tmux/key-bindings.c:691`: `static enum cmd_retval key_bindings_read_only(struct cmdq_item *item, __unused void *data)`
pub unsafe fn key_bindings_read_only(item: *mut cmdq_item, _data: *mut c_void) -> cmd_retval {
    unsafe {
        cmdq_error!(item, "client is read-only");
    }
    cmd_retval::CMD_RETURN_ERROR
}

/// C `vendor/tmux/key-bindings.c:698`: `struct cmdq_item *key_bindings_dispatch(struct key_binding *bd, struct cmdq_item *item, struct client *c, struct key_event *event, struct cmd_find_state *fs)`
pub unsafe fn key_bindings_dispatch(
    bd: *mut key_binding,
    item: *mut cmdq_item,
    c: *mut client,
    event: *mut key_event,
    fs: *mut cmd_find_state,
) -> *mut cmdq_item {
    unsafe {
        let mut flags = cmdq_state_flags::empty();

        let readonly = if c.is_null() || !(*c).flags.intersects(client_flag::READONLY) {
            true
        } else {
            cmd_list_all_have((*bd).cmdlist, cmd_flag::CMD_READONLY)
        };

        let mut new_item;
        if !readonly {
            new_item = cmdq_get_callback!(key_bindings_read_only, null_mut()).as_ptr();
        } else {
            if (*bd).flags & KEY_BINDING_REPEAT != 0 {
                flags |= cmdq_state_flags::CMDQ_STATE_REPEAT;
            }
            let new_state = cmdq_new_state(fs, event, flags);
            new_item = cmdq_get_command((*bd).cmdlist, new_state);
            cmdq_free_state(new_state);
        }
        if !item.is_null() {
            new_item = cmdq_insert_after(item, new_item);
        } else {
            new_item = cmdq_append(c, new_item);
        }
        new_item
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // key_bindings_get_table / add / remove mutate the process-global KEY_TABLES
    // static; cargo runs tests in parallel threads, so serialize every test that
    // touches it. See HARD RULE 6.
    static KB_LOCK: Mutex<()> = Mutex::new(());

    fn kb_lock() -> std::sync::MutexGuard<'static, ()> {
        KB_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    // Universal cleanup: drop the whole table (frees every binding + the name).
    // key_bindings_remove_table iterates the global CLIENTS tailq, which is
    // zero-initialized (empty) in the test binary, so the loop body never runs.
    unsafe fn drop_table(name: *const u8) {
        unsafe { key_bindings_remove_table(name) };
    }

    // Zeroed key_binding is enough for key_bindings_cmp, which only reads `.key`.
    fn kb_with_key(key: key_code) -> key_binding {
        let mut bd: key_binding = unsafe { std::mem::zeroed() };
        bd.key = key;
        bd
    }

    // key_table with only `.name` meaningful; key_table_cmp only reads `.name`.
    fn kt_with_name(name: *const u8) -> key_table {
        unsafe {
            key_table {
                name: std::ffi::CStr::from_ptr(name.cast()).to_owned(),
                activity_time: std::mem::zeroed(),
                key_bindings: rb_initializer(),
                default_key_bindings: rb_initializer(),
                references: 0,
                entry: rb_entry::default(),
            }
        }
    }

    // vendor/tmux/key-bindings.c:87 key_bindings_cmp: -1 if key1<key2, 1 if >, 0 if ==.
    #[test]
    fn cmp_orders_by_key() {
        let a = kb_with_key(1);
        let b = kb_with_key(2);
        assert_eq!(key_bindings_cmp(&a, &b), cmp::Ordering::Less);
        assert_eq!(key_bindings_cmp(&b, &a), cmp::Ordering::Greater);
        assert_eq!(key_bindings_cmp(&a, &a), cmp::Ordering::Equal);
    }

    #[test]
    fn cmp_handles_full_key_code_range() {
        // key_code is 64-bit; ordering must not truncate.
        let lo = kb_with_key(0x0000_0000_0000_0001);
        let hi = kb_with_key(0x0100_0000_0000_0000);
        assert_eq!(key_bindings_cmp(&lo, &hi), cmp::Ordering::Less);
        assert_eq!(key_bindings_cmp(&hi, &lo), cmp::Ordering::Greater);
    }

    // vendor/tmux/key-bindings.c:81 key_table_cmp: strcmp(name1, name2).
    #[test]
    fn table_cmp_orders_by_name() {
        let a = kt_with_name(c!("aaa"));
        let b = kt_with_name(c!("bbb"));
        assert_eq!(key_table_cmp(&a, &b), cmp::Ordering::Less);
        assert_eq!(key_table_cmp(&b, &a), cmp::Ordering::Greater);
        assert_eq!(key_table_cmp(&a, &a), cmp::Ordering::Equal);
    }

    #[test]
    fn table_cmp_prefix_is_less() {
        // strcmp("copy", "copy-mode") < 0 (shorter string is a prefix).
        let a = kt_with_name(c!("copy"));
        let b = kt_with_name(c!("copy-mode"));
        assert_eq!(key_table_cmp(&a, &b), cmp::Ordering::Less);
    }

    // vendor/tmux/key-bindings.c:105 key_bindings_get_table: create=0 returns NULL
    // when missing; create=1 inserts and returns the same table on re-lookup.
    #[test]
    fn get_table_create_then_find() {
        let _g = kb_lock();
        let name = c!("ztmux-ut-get-table");
        unsafe {
            // Not present initially.
            assert!(key_bindings_get_table(name, false).is_null());

            let t = key_bindings_get_table(name, true);
            assert!(!t.is_null());
            // Name is a heap dup that compares equal.
            assert_eq!(strcmp((*t).name_ptr(), name), 0);
            assert_eq!((*t).references, 1);

            // Re-lookup (no create) returns the identical pointer.
            assert_eq!(key_bindings_get_table(name, false), t);
            // Create again also returns the identical pointer (found, not new).
            assert_eq!(key_bindings_get_table(name, true), t);

            drop_table(name);
            assert!(key_bindings_get_table(name, false).is_null());
        }
    }

    // vendor/tmux/key-bindings.c:190 key_bindings_add + :160 key_bindings_get +
    // :234 key_bindings_remove round-trip.
    #[test]
    fn add_get_remove_roundtrip() {
        let _g = kb_lock();
        let name = c!("ztmux-ut-roundtrip");
        let key: key_code = b'a' as key_code;
        unsafe {
            let cmdlist = cmd_list_new();
            key_bindings_add(name, key, c!("my note"), false, cmdlist);

            let table = NonNull::new(key_bindings_get_table(name, false)).unwrap();
            let bd = key_bindings_get(table, key);
            assert!(!bd.is_null());
            assert_eq!((*bd).key, key);
            assert_eq!((*bd).cmdlist, cmdlist as *mut _);
            assert_eq!((*bd).flags & KEY_BINDING_REPEAT, 0);
            assert_eq!(strcmp((*bd).note_ptr(), c!("my note")), 0);

            // Remove: binding gone, and since the table is now empty it is
            // removed from KEY_TABLES too (key-bindings.c:253).
            key_bindings_remove(name, key);
            assert!(key_bindings_get_table(name, false).is_null());
        }
    }

    // vendor/tmux/key-bindings.c:223 repeat flag is set when repeat != 0.
    #[test]
    fn add_sets_repeat_flag() {
        let _g = kb_lock();
        let name = c!("ztmux-ut-repeat");
        let key: key_code = b'r' as key_code;
        unsafe {
            key_bindings_add(name, key, null_mut(), true, cmd_list_new());
            let table = NonNull::new(key_bindings_get_table(name, false)).unwrap();
            let bd = key_bindings_get(table, key);
            assert!(!bd.is_null());
            assert_ne!((*bd).flags & KEY_BINDING_REPEAT, 0);
            // note was NULL, so it stays NULL (key-bindings.c:219).
            assert!((*bd).note.is_none());
            drop_table(name);
        }
    }

    // vendor/tmux/key-bindings.c:211 re-adding an existing key frees the old
    // binding and replaces it (still exactly one binding for that key).
    #[test]
    fn add_replaces_existing() {
        let _g = kb_lock();
        let name = c!("ztmux-ut-replace");
        let key: key_code = b'x' as key_code;
        unsafe {
            key_bindings_add(name, key, null_mut(), false, cmd_list_new());
            let cmdlist2 = cmd_list_new();
            key_bindings_add(name, key, null_mut(), false, cmdlist2);

            let table = key_bindings_get_table(name, false);
            let table_nn = NonNull::new(table).unwrap();
            let bd = key_bindings_get(table_nn, key);
            assert!(!bd.is_null());
            assert_eq!((*bd).cmdlist, cmdlist2 as *mut _);

            // Exactly one binding in the table.
            let first = key_bindings_first(table);
            assert!(!first.is_null());
            assert!(key_bindings_next(table, first).is_null());

            drop_table(name);
        }
    }

    // vendor/tmux/key-bindings.c:199/217 the KEYC_MASK_FLAGS bits are stripped
    // from the key before storing, and key_bindings_get strips them on lookup.
    #[test]
    fn add_strips_key_flags() {
        let _g = kb_lock();
        let name = c!("ztmux-ut-mask");
        let base: key_code = b'q' as key_code;
        unsafe {
            key_bindings_add(name, base | KEYC_MASK_FLAGS, null_mut(), false, cmd_list_new());

            let table = NonNull::new(key_bindings_get_table(name, false)).unwrap();
            // Stored key has the KEYC_MASK_FLAGS bits stripped off. Note that
            // key_bindings_get itself does NOT mask (vendor/tmux/key-bindings.c:160),
            // so we look it up with the already-masked base key.
            let bd = key_bindings_get(table, base);
            assert!(!bd.is_null());
            assert_eq!((*bd).key, base);

            drop_table(name);
        }
    }

    // vendor/tmux/key-bindings.c:178/184 key_bindings_first/next walk the tree in
    // ascending key order (RB min + next), regardless of insertion order.
    #[test]
    fn first_next_iterate_sorted() {
        let _g = kb_lock();
        let name = c!("ztmux-ut-sorted");
        let insert_order: [key_code; 5] = [b'm' as _, b'a' as _, b'z' as _, b'c' as _, b'b' as _];
        unsafe {
            for &k in &insert_order {
                key_bindings_add(name, k, null_mut(), false, cmd_list_new());
            }
            let table = key_bindings_get_table(name, false);

            let mut got = Vec::new();
            let mut bd = key_bindings_first(table);
            while !bd.is_null() {
                got.push((*bd).key);
                bd = key_bindings_next(table, bd);
            }
            let mut want = insert_order.to_vec();
            want.sort_unstable();
            assert_eq!(got, want);

            drop_table(name);
        }
    }

    // vendor/tmux/key-bindings.c:234 removing an absent key is a no-op; removing a
    // present key from a table that still has others leaves the table alive.
    #[test]
    fn remove_absent_and_partial() {
        let _g = kb_lock();
        let name = c!("ztmux-ut-remove");
        let k1: key_code = b'1' as key_code;
        let k2: key_code = b'2' as key_code;
        unsafe {
            key_bindings_add(name, k1, null_mut(), false, cmd_list_new());
            key_bindings_add(name, k2, null_mut(), false, cmd_list_new());

            // Removing a key that was never bound does nothing.
            key_bindings_remove(name, b'9' as key_code);
            let table = NonNull::new(key_bindings_get_table(name, false)).unwrap();
            assert!(!key_bindings_get(table, k1).is_null());
            assert!(!key_bindings_get(table, k2).is_null());

            // Remove one; table survives because k2 remains.
            key_bindings_remove(name, k1);
            let table = NonNull::new(key_bindings_get_table(name, false)).unwrap();
            assert!(key_bindings_get(table, k1).is_null());
            assert!(!key_bindings_get(table, k2).is_null());

            drop_table(name);
        }
    }

    // vendor/tmux/key-bindings.c:169 key_bindings_get_default: no defaults were
    // inserted, so it always returns NULL for this table.
    #[test]
    fn get_default_is_null_without_defaults() {
        let _g = kb_lock();
        let name = c!("ztmux-ut-default");
        let key: key_code = b'k' as key_code;
        unsafe {
            key_bindings_add(name, key, null_mut(), false, cmd_list_new());
            let table = key_bindings_get_table(name, false);
            assert!(key_bindings_get_default(table, key).is_null());
            drop_table(name);
        }
    }

    // vendor/tmux/key-bindings.c:234/261 removing/resetting a nonexistent table is
    // a silent no-op (get_table with create=0 returns NULL and we bail).
    #[test]
    fn remove_and_reset_missing_table_noop() {
        let _g = kb_lock();
        let name = c!("ztmux-ut-missing");
        unsafe {
            assert!(key_bindings_get_table(name, false).is_null());
            // Must not panic / must not create the table.
            key_bindings_remove(name, b'z' as key_code);
            key_bindings_reset(name, b'z' as key_code);
            assert!(key_bindings_get_table(name, false).is_null());
        }
    }

    // vendor/tmux/key-bindings.c:200-208: when cmdlist==NULL and the binding
    // exists, C updates the note only if `note != NULL` and sets the repeat flag
    // when `repeat`. The ztmux port (key_bindings.rs:212-222) drops the repeat
    // handling entirely (and unconditionally nulls the note when note==NULL), so
    // a note-only update never sets KEY_BINDING_REPEAT. Asserting the C behavior.
    #[test]
    fn note_only_update_sets_repeat() {
        let _g = kb_lock();
        let name = c!("ztmux-ut-noteonly");
        let key: key_code = b'n' as key_code;
        unsafe {
            key_bindings_add(name, key, c!("orig"), false, cmd_list_new());
            // cmdlist == NULL, note != NULL, repeat == true.
            key_bindings_add(name, key, c!("updated"), true, null_mut());

            let table = NonNull::new(key_bindings_get_table(name, false)).unwrap();
            let bd = key_bindings_get(table, key);
            assert!(!bd.is_null());
            assert_eq!(strcmp((*bd).note_ptr(), c!("updated")), 0);
            // C sets the repeat flag here; the port does not.
            assert_ne!((*bd).flags & KEY_BINDING_REPEAT, 0);

            drop_table(name);
        }
    }

    // vendor/tmux/key-bindings.c:126/132 key_bindings_first_table / next_table
    // walk KEY_TABLES in ascending name order (RB min + next), independent of
    // insertion order. The lock guarantees KEY_TABLES is otherwise empty here.
    #[test]
    fn tables_iterate_in_name_order() {
        let _g = kb_lock();
        let names = [c!("kbt-mmm"), c!("kbt-aaa"), c!("kbt-zzz")];
        let key: key_code = b'x' as key_code;
        unsafe {
            for &n in &names {
                key_bindings_add(n, key, null_mut(), false, cmd_list_new());
            }

            let mut got: Vec<String> = Vec::new();
            let mut t = key_bindings_first_table();
            while !t.is_null() {
                got.push((*t).name.to_str().unwrap().to_string());
                t = key_bindings_next_table(t);
            }
            assert_eq!(got, vec!["kbt-aaa", "kbt-mmm", "kbt-zzz"]);

            for &n in &names {
                drop_table(n);
            }
            assert!(key_bindings_first_table().is_null());
        }
    }

    // vendor/tmux/key-bindings.c:261 key_bindings_reset: with no default binding
    // for the key it falls through to key_bindings_remove, deleting the binding
    // (and the now-empty table).
    #[test]
    fn reset_without_default_removes_binding() {
        let _g = kb_lock();
        let name = c!("ztmux-ut-reset-nodef");
        let key: key_code = b'g' as key_code;
        unsafe {
            key_bindings_add(name, key, null_mut(), false, cmd_list_new());
            assert!(!key_bindings_get_table(name, false).is_null());

            key_bindings_reset(name, key);
            // No default existed, so the binding was removed; table now gone.
            assert!(key_bindings_get_table(name, false).is_null());
        }
    }

    // vendor/tmux/key-bindings.c:293 key_bindings_remove_table drops the whole
    // table and every binding in it in one call.
    #[test]
    fn remove_table_drops_all_bindings() {
        let _g = kb_lock();
        let name = c!("ztmux-ut-remove-table");
        unsafe {
            for k in [b'a', b'b', b'c'] {
                key_bindings_add(name, k as key_code, null_mut(), false, cmd_list_new());
            }
            assert!(!key_bindings_get_table(name, false).is_null());

            key_bindings_remove_table(name);
            assert!(key_bindings_get_table(name, false).is_null());
        }
    }

    // vendor/tmux/key-bindings.c:190-232 re-adding with a real cmdlist takes the
    // replacement path (frees the old bd, allocates a new one). With note==NULL
    // the new binding's note is NULL even though the old one had a note.
    #[test]
    fn replace_with_null_note_clears_note() {
        let _g = kb_lock();
        let name = c!("ztmux-ut-replace-note");
        let key: key_code = b'p' as key_code;
        unsafe {
            key_bindings_add(name, key, c!("first"), false, cmd_list_new());
            // Replacement path: cmdlist != NULL, note == NULL.
            key_bindings_add(name, key, null_mut(), false, cmd_list_new());

            let table = NonNull::new(key_bindings_get_table(name, false)).unwrap();
            let bd = key_bindings_get(table, key);
            assert!(!bd.is_null());
            assert!((*bd).note.is_none(), "replacement with NULL note must clear it");

            drop_table(name);
        }
    }

    // Two independent tables: a key bound in one is invisible in the other, and
    // removing one leaves the other intact.
    #[test]
    fn two_tables_are_independent() {
        let _g = kb_lock();
        let n1 = c!("ztmux-ut-tbl-1");
        let n2 = c!("ztmux-ut-tbl-2");
        let key: key_code = b'k' as key_code;
        unsafe {
            key_bindings_add(n1, key, null_mut(), false, cmd_list_new());

            // n2 was never created.
            assert!(key_bindings_get_table(n2, false).is_null());

            key_bindings_add(n2, key, null_mut(), false, cmd_list_new());
            let t1 = NonNull::new(key_bindings_get_table(n1, false)).unwrap();
            let t2 = NonNull::new(key_bindings_get_table(n2, false)).unwrap();
            assert_ne!(t1.as_ptr(), t2.as_ptr());
            assert!(!key_bindings_get(t1, key).is_null());
            assert!(!key_bindings_get(t2, key).is_null());

            // Removing n1's only binding drops n1 but not n2.
            key_bindings_remove(n1, key);
            assert!(key_bindings_get_table(n1, false).is_null());
            assert!(!key_bindings_get_table(n2, false).is_null());

            drop_table(n2);
        }
    }
}
