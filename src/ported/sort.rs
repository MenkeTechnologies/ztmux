// Copyright (c) 2026 Dane Jensen <dhcjensen@gmail.com>
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
//
// Faithful port of vendor/tmux/sort.c. The C file caches its results in a
// `static` reallocated array and returns the count through a `u_int *n`
// out-parameter; the Rust collectors return an owned `Vec<*mut T>` instead
// (its `.len()` is the count), which is equivalent for every caller.
//
// The C comparators read the criteria from a file-static `sort_criteria`
// pointer so they can be passed to `qsort`. The Rust ports take the criteria
// by value and are driven through `Vec::sort_by`, so no shared static is
// needed.
use crate::*;

/// C `vendor/tmux/tmux.h:2469`: `enum sort_order`.
#[derive(Clone, Copy, PartialEq)]
pub enum sort_order {
    SORT_ACTIVITY,
    SORT_CREATION,
    SORT_INDEX,
    SORT_MODIFIER,
    SORT_NAME,
    SORT_ORDER,
    SORT_SIZE,
    SORT_Z,
    SORT_END,
}

/// C `vendor/tmux/tmux.h:2482`: `struct sort_criteria`.
#[derive(Clone, Copy)]
pub struct sort_criteria {
    pub order: sort_order,
    pub reversed: bool,
    /// C: `enum sort_order *order_seq` — the null(`SORT_END`)-terminated list of
    /// available orders cycled through by `sort_next_order`. `null` when the
    /// caller does not cycle (e.g. the format `#{S:} #{W:} #{P:} #{L:}` loops).
    pub order_seq: *mut sort_order,
}

/// C `vendor/tmux/sort.c:28`: `static void sort_qsort(void *l, u_int len, u_int size, int (*cmp)(const void *, const void *), struct sort_criteria *sort_crit)`
unsafe fn sort_qsort<T>(
    l: &mut [*mut T],
    cmp: unsafe fn(*mut T, *mut T, sort_criteria) -> i32,
    sc: sort_criteria,
) {
    match sc.order {
        sort_order::SORT_END => (),
        sort_order::SORT_ORDER => {
            if sc.reversed {
                l.reverse();
            }
        }
        _ => l.sort_by(|&a, &b| unsafe { cmp(a, b, sc) }.cmp(&0)),
    }
}

/// C `vendor/tmux/sort.c:53`: `static int sort_buffer_cmp(const void *a0, const void *b0)`
unsafe fn sort_buffer_cmp(a: *mut paste_buffer, b: *mut paste_buffer, sc: sort_criteria) -> i32 {
    unsafe {
        let na = (*a).name.as_ref();
        let nb = (*b).name.as_ref();
        let ncmp = na.cmp(nb) as i32;
        let mut result = match sc.order {
            sort_order::SORT_NAME => ncmp,
            sort_order::SORT_CREATION => (*b).order.cmp(&(*a).order) as i32,
            sort_order::SORT_SIZE => (*a).size.wrapping_sub((*b).size) as i32,
            _ => 0,
        };
        if result == 0 {
            result = ncmp;
        }
        if sc.reversed {
            result = -result;
        }
        result
    }
}

/// C `vendor/tmux/sort.c:95`: `static int sort_client_cmp(const void *a0, const void *b0)`
unsafe fn sort_client_cmp(a: *mut client, b: *mut client, sc: sort_criteria) -> i32 {
    unsafe {
        let ncmp = strcmp((*a).name, (*b).name);
        let mut result = match sc.order {
            sort_order::SORT_NAME => ncmp,
            sort_order::SORT_SIZE => {
                let mut r = (*a).tty.sx.wrapping_sub((*b).tty.sx) as i32;
                if r == 0 {
                    r = (*a).tty.sy.wrapping_sub((*b).tty.sy) as i32;
                }
                r
            }
            sort_order::SORT_CREATION => {
                let at = ((*a).creation_time.tv_sec, (*a).creation_time.tv_usec);
                let bt = ((*b).creation_time.tv_sec, (*b).creation_time.tv_usec);
                at.cmp(&bt) as i32
            }
            sort_order::SORT_ACTIVITY => {
                let at = ((*a).activity_time.tv_sec, (*a).activity_time.tv_usec);
                let bt = ((*b).activity_time.tv_sec, (*b).activity_time.tv_usec);
                bt.cmp(&at) as i32
            }
            _ => 0,
        };
        if result == 0 {
            result = ncmp;
        }
        if sc.reversed {
            result = -result;
        }
        result
    }
}

/// C `vendor/tmux/sort.c:142`: `static int sort_session_cmp(const void *a0, const void *b0)`
unsafe fn sort_session_cmp(a: *mut session, b: *mut session, sc: sort_criteria) -> i32 {
    unsafe {
        let na = (*a).name.as_ref();
        let nb = (*b).name.as_ref();
        let ncmp = na.cmp(nb) as i32;
        let mut result = match sc.order {
            sort_order::SORT_INDEX => (*a).id.wrapping_sub((*b).id) as i32,
            sort_order::SORT_CREATION => {
                let at = ((*a).creation_time.tv_sec, (*a).creation_time.tv_usec);
                let bt = ((*b).creation_time.tv_sec, (*b).creation_time.tv_usec);
                at.cmp(&bt) as i32
            }
            sort_order::SORT_ACTIVITY => {
                let at = ((*a).activity_time.tv_sec, (*a).activity_time.tv_usec);
                let bt = ((*b).activity_time.tv_sec, (*b).activity_time.tv_usec);
                bt.cmp(&at) as i32
            }
            sort_order::SORT_NAME => ncmp,
            _ => 0,
        };
        if result == 0 {
            result = ncmp;
        }
        if sc.reversed {
            result = -result;
        }
        result
    }
}

/// C `vendor/tmux/sort.c:195`: `static int sort_pane_cmp(const void *a0, const void *b0)`
unsafe fn sort_pane_cmp(a: *mut window_pane, b: *mut window_pane, sc: sort_criteria) -> i32 {
    unsafe {
        let title_cmp = || strcmp((*(*a).screen).title, (*(*b).screen).title);
        let mut result = match sc.order {
            sort_order::SORT_ACTIVITY => (*a).active_point.wrapping_sub((*b).active_point) as i32,
            sort_order::SORT_CREATION => (*a).id.wrapping_sub((*b).id) as i32,
            sort_order::SORT_SIZE => {
                (*a).sx
                    .wrapping_mul((*a).sy)
                    .wrapping_sub((*b).sx.wrapping_mul((*b).sy)) as i32
            }
            sort_order::SORT_INDEX => {
                let mut ai = 0;
                let mut bi = 0;
                window_pane_index(a, &raw mut ai);
                window_pane_index(b, &raw mut bi);
                ai.wrapping_sub(bi) as i32
            }
            sort_order::SORT_NAME => title_cmp(),
            sort_order::SORT_Z => {
                let mut ai = 0;
                let mut bi = 0;
                window_pane_zindex(a, &raw mut ai);
                window_pane_zindex(b, &raw mut bi);
                ai.wrapping_sub(bi) as i32
            }
            _ => 0,
        };
        if result == 0 {
            result = title_cmp();
        }
        if sc.reversed {
            result = -result;
        }
        result
    }
}

/// C `vendor/tmux/sort.c:241`: `static int sort_winlink_cmp(const void *a0, const void *b0)`
unsafe fn sort_winlink_cmp(a: *mut winlink, b: *mut winlink, sc: sort_criteria) -> i32 {
    unsafe {
        let wa = (*a).window;
        let wb = (*b).window;
        let ncmp = strcmp((*wa).name, (*wb).name);
        let mut result = match sc.order {
            sort_order::SORT_INDEX => (*a).idx.wrapping_sub((*b).idx),
            sort_order::SORT_CREATION => {
                let at = ((*wa).creation_time.tv_sec, (*wa).creation_time.tv_usec);
                let bt = ((*wb).creation_time.tv_sec, (*wb).creation_time.tv_usec);
                at.cmp(&bt) as i32
            }
            sort_order::SORT_ACTIVITY => {
                let at = ((*wa).activity_time.tv_sec, (*wa).activity_time.tv_usec);
                let bt = ((*wb).activity_time.tv_sec, (*wb).activity_time.tv_usec);
                bt.cmp(&at) as i32
            }
            sort_order::SORT_NAME => ncmp,
            sort_order::SORT_SIZE => (*wa)
                .sx
                .wrapping_mul((*wa).sy)
                .wrapping_sub((*wb).sx.wrapping_mul((*wb).sy))
                as i32,
            _ => 0,
        };
        if result == 0 {
            result = ncmp;
        }
        if sc.reversed {
            result = -result;
        }
        result
    }
}

#[expect(
    dead_code,
    reason = "unified sort path (cmd-list-*/mode-tree), not yet wired"
)]
/// C `vendor/tmux/sort.c:334`: `void sort_next_order(struct sort_criteria *sort_crit)`
pub unsafe fn sort_next_order(sc: *mut sort_criteria) {
    unsafe {
        let seq = (*sc).order_seq;
        if seq.is_null() {
            return;
        }
        let mut i = 0usize;
        while *seq.add(i) != sort_order::SORT_END {
            if (*sc).order == *seq.add(i) {
                break;
            }
            i += 1;
        }

        if *seq.add(i) == sort_order::SORT_END {
            i = 0;
        } else {
            i += 1;
            if *seq.add(i) == sort_order::SORT_END {
                i = 0;
            }
        }
        (*sc).order = *seq.add(i);
    }
}

/// C `vendor/tmux/sort.c:356`: `enum sort_order sort_order_from_string(const char* order)`
pub unsafe fn sort_order_from_string(order: *const u8) -> sort_order {
    unsafe {
        if !order.is_null() {
            if strcaseeq_(order, "activity") {
                return sort_order::SORT_ACTIVITY;
            }
            if strcaseeq_(order, "creation") {
                return sort_order::SORT_CREATION;
            }
            if strcaseeq_(order, "index") || strcaseeq_(order, "key") {
                return sort_order::SORT_INDEX;
            }
            if strcaseeq_(order, "modifier") {
                return sort_order::SORT_MODIFIER;
            }
            if strcaseeq_(order, "name") || strcaseeq_(order, "title") {
                return sort_order::SORT_NAME;
            }
            if strcaseeq_(order, "order") {
                return sort_order::SORT_ORDER;
            }
            if strcaseeq_(order, "size") {
                return sort_order::SORT_SIZE;
            }
            if strcaseeq_(order, "z") {
                return sort_order::SORT_Z;
            }
        }
        sort_order::SORT_END
    }
}

#[expect(
    dead_code,
    reason = "unified sort path (cmd-list-*/mode-tree), not yet wired"
)]
/// C `vendor/tmux/sort.c:382`: `const char *sort_order_to_string(enum sort_order order)`
pub fn sort_order_to_string(order: sort_order) -> *const u8 {
    match order {
        sort_order::SORT_ACTIVITY => c!("activity"),
        sort_order::SORT_CREATION => c!("creation"),
        sort_order::SORT_INDEX => c!("index"),
        sort_order::SORT_MODIFIER => c!("modifier"),
        sort_order::SORT_NAME => c!("name"),
        sort_order::SORT_ORDER => c!("order"),
        sort_order::SORT_SIZE => c!("size"),
        sort_order::SORT_Z => c!("z"),
        sort_order::SORT_END => null(),
    }
}

#[expect(
    dead_code,
    reason = "unified sort path (cmd-list-*/mode-tree), not yet wired"
)]
/// C `vendor/tmux/sort.c:404`: `int sort_would_window_tree_swap(struct sort_criteria *sort_crit, struct winlink *wla, struct winlink *wlb)`
pub unsafe fn sort_would_window_tree_swap(
    sc: sort_criteria,
    wla: *mut winlink,
    wlb: *mut winlink,
) -> i32 {
    unsafe {
        if sc.order == sort_order::SORT_INDEX {
            return 0;
        }
        i32::from(sort_winlink_cmp(wla, wlb, sc) != 0)
    }
}

#[expect(
    dead_code,
    reason = "unified sort path (cmd-list-*/mode-tree), not yet wired"
)]
/// C `vendor/tmux/sort.c:414`: `struct paste_buffer **sort_get_buffers(u_int *n, struct sort_criteria *sort_crit)`
pub unsafe fn sort_get_buffers(sc: sort_criteria) -> Vec<*mut paste_buffer> {
    unsafe {
        let mut l: Vec<*mut paste_buffer> = Vec::new();
        let mut pb: *mut paste_buffer = null_mut();
        loop {
            pb = paste_walk(pb);
            if pb.is_null() {
                break;
            }
            l.push(pb);
        }
        sort_qsort(&mut l, sort_buffer_cmp, sc);
        l
    }
}

/// C `vendor/tmux/sort.c:437`: `struct client **sort_get_clients(u_int *n, struct sort_criteria *sort_crit)`
pub unsafe fn sort_get_clients(sc: sort_criteria) -> Vec<*mut client> {
    unsafe {
        let mut l: Vec<*mut client> = tailq_foreach(&raw mut CLIENTS)
            .map(NonNull::as_ptr)
            .filter(|&c| {
                !(*c).flags.intersects(CLIENT_UNATTACHEDFLAGS)
                    && (*c).flags.intersects(client_flag::ATTACHED)
            })
            .collect();
        sort_qsort(&mut l, sort_client_cmp, sc);
        l
    }
}

/// C `vendor/tmux/sort.c:464`: `struct session **sort_get_sessions(u_int *n, struct sort_criteria *sort_crit)`
pub unsafe fn sort_get_sessions(sc: sort_criteria) -> Vec<*mut session> {
    unsafe {
        let mut l: Vec<*mut session> = rb_foreach(&raw mut SESSIONS).map(NonNull::as_ptr).collect();
        sort_qsort(&mut l, sort_session_cmp, sc);
        l
    }
}

#[expect(
    dead_code,
    reason = "unified sort path (cmd-list-*/mode-tree), not yet wired"
)]
/// C `vendor/tmux/sort.c:487`: `struct window_pane **sort_get_panes(u_int *n, struct sort_criteria *sort_crit)`
pub unsafe fn sort_get_panes(sc: sort_criteria) -> Vec<*mut window_pane> {
    unsafe {
        let mut l: Vec<*mut window_pane> = Vec::new();
        for s in rb_foreach(&raw mut SESSIONS).map(NonNull::as_ptr) {
            for wl in rb_foreach(&raw mut (*s).windows).map(NonNull::as_ptr) {
                let w = (*wl).window;
                for wp in tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).map(NonNull::as_ptr)
                {
                    l.push(wp);
                }
            }
        }
        sort_qsort(&mut l, sort_pane_cmp, sc);
        l
    }
}

#[expect(
    dead_code,
    reason = "unified sort path (cmd-list-*/mode-tree), not yet wired"
)]
/// C `vendor/tmux/sort.c:518`: `struct window_pane **sort_get_panes_session(struct session *s, u_int *n, struct sort_criteria *sort_crit)`
pub unsafe fn sort_get_panes_session(s: *mut session, sc: sort_criteria) -> Vec<*mut window_pane> {
    unsafe {
        let mut l: Vec<*mut window_pane> = Vec::new();
        for wl in rb_foreach(&raw mut (*s).windows).map(NonNull::as_ptr) {
            let w = (*wl).window;
            for wp in tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).map(NonNull::as_ptr) {
                l.push(wp);
            }
        }
        sort_qsort(&mut l, sort_pane_cmp, sc);
        l
    }
}

/// C `vendor/tmux/sort.c:547`: `struct window_pane **sort_get_panes_window(struct window *w, u_int *n, struct sort_criteria *sort_crit)`
pub unsafe fn sort_get_panes_window(w: *mut window, sc: sort_criteria) -> Vec<*mut window_pane> {
    unsafe {
        let mut l: Vec<*mut window_pane> = tailq_foreach::<_, discr_entry>(&raw mut (*w).panes)
            .map(NonNull::as_ptr)
            .collect();
        sort_qsort(&mut l, sort_pane_cmp, sc);
        l
    }
}

/// C `vendor/tmux/sort.c:571`: `struct winlink **sort_get_winlinks(u_int *n, struct sort_criteria *sort_crit)`
pub unsafe fn sort_get_winlinks(sc: sort_criteria) -> Vec<*mut winlink> {
    unsafe {
        let mut l: Vec<*mut winlink> = Vec::new();
        for s in rb_foreach(&raw mut SESSIONS).map(NonNull::as_ptr) {
            for wl in rb_foreach(&raw mut (*s).windows).map(NonNull::as_ptr) {
                l.push(wl);
            }
        }
        sort_qsort(&mut l, sort_winlink_cmp, sc);
        l
    }
}

/// C `vendor/tmux/sort.c:597`: `struct winlink **sort_get_winlinks_session(struct session *s, u_int *n, struct sort_criteria *sort_crit)`
pub unsafe fn sort_get_winlinks_session(s: *mut session, sc: sort_criteria) -> Vec<*mut winlink> {
    unsafe {
        let mut l: Vec<*mut winlink> = rb_foreach(&raw mut (*s).windows)
            .map(NonNull::as_ptr)
            .collect();
        sort_qsort(&mut l, sort_winlink_cmp, sc);
        l
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A plain-old-data stand-in for the tmux structs the real comparators walk,
    // so `sort_qsort`'s dispatch can be exercised without a live server.
    struct Item {
        key: i32,
    }

    unsafe fn item_cmp(a: *mut Item, b: *mut Item, sc: sort_criteria) -> i32 {
        unsafe {
            let mut result = match sc.order {
                sort_order::SORT_INDEX => (*a).key - (*b).key,
                _ => 0,
            };
            if sc.reversed {
                result = -result;
            }
            result
        }
    }

    fn crit(order: sort_order, reversed: bool) -> sort_criteria {
        sort_criteria {
            order,
            reversed,
            order_seq: core::ptr::null_mut(),
        }
    }

    fn keys(l: &[*mut Item]) -> Vec<i32> {
        unsafe { l.iter().map(|&p| (*p).key).collect() }
    }

    // `sort_qsort` faithfully mirrors sort.c:28 — SORT_END is a no-op, SORT_ORDER
    // keeps insertion order (reversing it only when `reversed`), and any other
    // order runs the comparator. These are the exact branches that decide the
    // element order of every `#{S:} #{W:} #{P:} #{L:}` format loop.
    #[test]
    fn sort_qsort_dispatch() {
        unsafe {
            let mut items = [Item { key: 3 }, Item { key: 1 }, Item { key: 2 }];
            let (p0, p1, p2) = (&raw mut items[0], &raw mut items[1], &raw mut items[2]);

            // SORT_INDEX runs the comparator: ascending by key.
            let mut l = vec![p0, p1, p2];
            sort_qsort(&mut l, item_cmp, crit(sort_order::SORT_INDEX, false));
            assert_eq!(keys(&l), [1, 2, 3]);

            // Reversed comparator: descending by key.
            let mut l = vec![p0, p1, p2];
            sort_qsort(&mut l, item_cmp, crit(sort_order::SORT_INDEX, true));
            assert_eq!(keys(&l), [3, 2, 1]);

            // SORT_ORDER keeps insertion order untouched.
            let mut l = vec![p0, p1, p2];
            sort_qsort(&mut l, item_cmp, crit(sort_order::SORT_ORDER, false));
            assert_eq!(keys(&l), [3, 1, 2]);

            // SORT_ORDER reversed flips insertion order in place (no comparator).
            let mut l = vec![p0, p1, p2];
            sort_qsort(&mut l, item_cmp, crit(sort_order::SORT_ORDER, true));
            assert_eq!(keys(&l), [2, 1, 3]);

            // SORT_END is a no-op even when reversed.
            let mut l = vec![p0, p1, p2];
            sort_qsort(&mut l, item_cmp, crit(sort_order::SORT_END, true));
            assert_eq!(keys(&l), [3, 1, 2]);
        }
    }
}
