// Copyright (c) 2013 Nicholas Marriott <nicholas.marriott@gmail.com>
// Copyright (c) 2013 Thiago de Arruda <tpadilha84@gmail.com>
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
use std::cmp::Ordering;
use std::ffi::{CStr, CString};

use crate::compat::{
    queue::{tailq_empty, tailq_first, tailq_foreach, tailq_init, tailq_insert_tail, tailq_remove},
    tree::{rb_find_by, rb_foreach, rb_initializer, rb_insert, rb_remove},
};
use crate::*;

pub static CMD_WAIT_FOR_ENTRY: cmd_entry = cmd_entry {
    name: "wait-for",
    alias: Some("wait"),

    args: args_parse::new("LSU", 1, 1, None),
    usage: "[-L|-S|-U] channel",

    flags: cmd_flag::empty(),
    exec: cmd_wait_for_exec,
    source: cmd_entry_flag::zeroed(),
    target: cmd_entry_flag::zeroed(),
};

impl_tailq_entry!(wait_item, entry, tailq_entry<wait_item>);
#[repr(C)]
pub struct wait_item {
    item: *mut cmdq_item,
    // #[entry]
    entry: tailq_entry<wait_item>,
}

#[repr(C)]
pub struct wait_channel {
    /// Owned channel name (always set). Dropped with the struct in
    /// `cmd_wait_for_remove`; C freed `wc->name` separately. Read via `n()`.
    pub name: CString,
    pub locked: bool,
    pub woken: bool,

    pub waiters: tailq_head<wait_item>,
    pub lockers: tailq_head<wait_item>,

    pub entry: rb_entry<wait_channel>,
}

impl wait_channel {
    pub(crate) fn n(&self) -> *const u8 {
        self.name.as_ptr().cast()
    }
}

pub type wait_channels = rb_head<wait_channel>;

static mut WAIT_CHANNELS: wait_channels = rb_initializer();

RB_GENERATE!(
    wait_channels,
    wait_channel,
    entry,
    discr_entry,
    wait_channel_cmp
);

/// C `vendor/tmux/cmd-wait-for.c:66`: `static int wait_channel_cmp(struct wait_channel *wc1, struct wait_channel *wc2)`
pub fn wait_channel_cmp(wc1: &wait_channel, wc2: &wait_channel) -> Ordering {
    // strcmp orders by unsigned byte, and treats a prefix as less than the longer
    // string (the NUL compares low). Comparing the byte slices reproduces both.
    wc1.name.as_bytes().cmp(wc2.name.as_bytes())
}

/// C `vendor/tmux/cmd-wait-for.c:84`: `static struct wait_channel *cmd_wait_for_add(const char *name)`
pub unsafe fn cmd_wait_for_add(name: *const u8) -> *mut wait_channel {
    unsafe {
        let wc = Box::into_raw(Box::new(wait_channel {
            name: CStr::from_ptr(name.cast()).to_owned(),
            locked: false,
            woken: false,
            waiters: zeroed(),
            lockers: zeroed(),
            entry: zeroed(),
        }));

        tailq_init(&raw mut (*wc).waiters);
        tailq_init(&raw mut (*wc).lockers);

        rb_insert(&raw mut WAIT_CHANNELS, wc);

        log_debug!("add wait channel {}", _s((*wc).n()));

        wc
    }
}

/// C `vendor/tmux/cmd-wait-for.c:105`: `static void cmd_wait_for_remove(struct wait_channel *wc)`
pub unsafe fn cmd_wait_for_remove(wc: *mut wait_channel) {
    unsafe {
        if (*wc).locked {
            return;
        }
        if !tailq_empty(&raw mut (*wc).waiters) || !(*wc).woken {
            return;
        }

        log_debug!("remove wait channel {}", _s((*wc).n()));

        rb_remove(&raw mut WAIT_CHANNELS, wc);

        // Reclaim the Box from cmd_wait_for_add. Dropping it frees the owned name,
        // which C did by hand (`free(wc->name); free(wc);`).
        drop(Box::from_raw(wc));
    }
}

/// C `vendor/tmux/cmd-wait-for.c:121`: `static enum cmd_retval cmd_wait_for_exec(struct cmd *self, struct cmdq_item *item)`
pub unsafe fn cmd_wait_for_exec(self_: *mut cmd, item: *mut cmdq_item) -> cmd_retval {
    unsafe {
        let args = cmd_get_args(self_);
        let name = args_string(args, 0);

        // C builds a throwaway stack `struct wait_channel` as the RB_FIND key. With an
        // owned name that is unsound in Rust — a partially-initialized wait_channel
        // would drop garbage. Search by key instead; same descent, no key node.
        let key = CStr::from_ptr(name.cast());
        let wc = rb_find_by(&raw mut WAIT_CHANNELS, |wc| {
            key.to_bytes().cmp(wc.name.as_bytes())
        });

        if args_has(args, 'S') {
            return cmd_wait_for_signal(item, name, wc);
        }
        if args_has(args, 'L') {
            return cmd_wait_for_lock(item, name, wc);
        }
        if args_has(args, 'U') {
            return cmd_wait_for_unlock(item, name, wc);
        }

        cmd_wait_for_wait(item, name, wc)
    }
}

/// C `vendor/tmux/cmd-wait-for.c:140`: `static enum cmd_retval cmd_wait_for_signal(__unused struct cmdq_item *item, const char *name, struct wait_channel *wc)`
pub unsafe fn cmd_wait_for_signal(
    _item: *const cmdq_item,
    name: *const u8,
    mut wc: *mut wait_channel,
) -> cmd_retval {
    unsafe {
        if wc.is_null() {
            wc = cmd_wait_for_add(name);
        }

        if tailq_empty(&raw mut (*wc).waiters) && !(*wc).woken {
            log_debug!("signal wait channel {}, no waiters", _s((*wc).n()));
            (*wc).woken = true;
            return cmd_retval::CMD_RETURN_NORMAL;
        }
        log_debug!("signal wait channel {}, with waiters", _s((*wc).n()));

        for wi in tailq_foreach::<_, ()>(&raw mut (*wc).waiters).map(NonNull::as_ptr) {
            cmdq_continue((*wi).item);

            tailq_remove::<_, ()>(&raw mut (*wc).waiters, wi);
            drop(Box::from_raw(wi));
        }

        cmd_wait_for_remove(wc);

        cmd_retval::CMD_RETURN_NORMAL
    }
}

/// C `vendor/tmux/cmd-wait-for.c:167`: `static enum cmd_retval cmd_wait_for_wait(struct cmdq_item *item, const char *name, struct wait_channel *wc)`
pub unsafe fn cmd_wait_for_wait(
    item: *mut cmdq_item,
    name: *const u8,
    mut wc: *mut wait_channel,
) -> cmd_retval {
    unsafe {
        let c = cmdq_get_client(item);

        if c.is_null() {
            cmdq_error!(item, "not able to wait");
            return cmd_retval::CMD_RETURN_ERROR;
        }

        if wc.is_null() {
            wc = cmd_wait_for_add(name);
        }

        if (*wc).woken {
            log_debug!("wait channel {} already woken ({:p})", _s((*wc).n()), c);
            cmd_wait_for_remove(wc);
            return cmd_retval::CMD_RETURN_NORMAL;
        }
        log_debug!("wait channel {} not woken ({:p})", _s((*wc).n()), c);

        let wi = Box::into_raw(Box::new(wait_item {
            item,
            entry: zeroed(),
        }));
        tailq_insert_tail(&raw mut (*wc).waiters, wi);
    }
    cmd_retval::CMD_RETURN_WAIT
}

/// C `vendor/tmux/cmd-wait-for.c:196`: `static enum cmd_retval cmd_wait_for_lock(struct cmdq_item *item, const char *name, struct wait_channel *wc)`
pub unsafe fn cmd_wait_for_lock(
    item: *mut cmdq_item,
    name: *const u8,
    mut wc: *mut wait_channel,
) -> cmd_retval {
    unsafe {
        if cmdq_get_client(item).is_null() {
            cmdq_error!(item, "not able to lock");
            return cmd_retval::CMD_RETURN_ERROR;
        }

        if wc.is_null() {
            wc = cmd_wait_for_add(name);
        }

        if (*wc).locked {
            let wi = Box::into_raw(Box::new(wait_item {
                item,
                entry: zeroed(),
            }));
            tailq_insert_tail(&raw mut (*wc).lockers, wi);
            return cmd_retval::CMD_RETURN_WAIT;
        }
        (*wc).locked = true;
    }
    cmd_retval::CMD_RETURN_NORMAL
}

/// C `vendor/tmux/cmd-wait-for.c:221`: `static enum cmd_retval cmd_wait_for_unlock(struct cmdq_item *item, const char *name, struct wait_channel *wc)`
pub unsafe fn cmd_wait_for_unlock(
    item: *mut cmdq_item,
    name: *const u8,
    wc: *mut wait_channel,
) -> cmd_retval {
    unsafe {
        if wc.is_null() || !(*wc).locked {
            cmdq_error!(item, "channel {} not locked", _s(name));
            return cmd_retval::CMD_RETURN_ERROR;
        }

        let wi = tailq_first(&raw mut (*wc).lockers);
        if !wi.is_null() {
            cmdq_continue((*wi).item);
            tailq_remove(&raw mut (*wc).lockers, wi);
            drop(Box::from_raw(wi));
        } else {
            (*wc).locked = false;
            cmd_wait_for_remove(wc);
        }
    }
    cmd_retval::CMD_RETURN_NORMAL
}

/// C `vendor/tmux/cmd-wait-for.c:244`: `void cmd_wait_for_flush(void)`
pub unsafe fn cmd_wait_for_flush() {
    unsafe {
        for wc in rb_foreach(&raw mut WAIT_CHANNELS).map(NonNull::as_ptr) {
            for wi in tailq_foreach(&raw mut (*wc).waiters).map(NonNull::as_ptr) {
                cmdq_continue((*wi).item);
                tailq_remove(&raw mut (*wc).waiters, wi);
                drop(Box::from_raw(wi));
            }
            (*wc).woken = true;
            for wi in tailq_foreach(&raw mut (*wc).lockers).map(NonNull::as_ptr) {
                cmdq_continue((*wi).item);
                tailq_remove(&raw mut (*wc).lockers, wi);
                drop(Box::from_raw(wi));
            }
            (*wc).locked = false;
            cmd_wait_for_remove(wc);
        }
    }
}
