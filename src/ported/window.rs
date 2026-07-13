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
use crate::compat::HOST_NAME_MAX;
use crate::libc::{
    FIONREAD, FNM_CASEFOLD, TIOCSWINSZ, close, fnmatch, free, gethostname, gettimeofday, ioctl,
    isspace, memset, regcomp, regex_t, regexec, regfree, strlen, winsize,
};
#[cfg(feature = "utempter")]
use crate::utempter::utempter_remove_record;
use crate::*;
use crate::options_::{options_create, options_free, options_get_number___, options_get_string_};

/// Default pixel cell sizes.
pub const DEFAULT_XPIXEL: u32 = 16;
pub const DEFAULT_YPIXEL: u32 = 32;

pub static mut WINDOWS: windows = unsafe { std::mem::zeroed() };

pub static mut ALL_WINDOW_PANES: window_pane_tree = unsafe { std::mem::zeroed() };

#[repr(C)]
#[derive(Copy, Clone)]
pub struct window_pane_input_data {
    item: *mut cmdq_item,
    wp: u32,
    file: *mut client_file,
}

RB_GENERATE!(windows, window, entry, discr_entry, window_cmp);
RB_GENERATE!(winlinks, winlink, entry, discr_entry, winlink_cmp);
RB_GENERATE!(
    window_pane_tree,
    window_pane,
    tree_entry,
    discr_tree_entry,
    window_pane_cmp
);

/// C `vendor/tmux/window.c:91`: `int window_cmp(struct window *w1, struct window *w2)`
pub fn window_cmp(w1: &window, w2: &window) -> cmp::Ordering {
    w1.id.cmp(&w2.id)
}

/// C `vendor/tmux/window.c:97`: `int winlink_cmp(struct winlink *wl1, struct winlink *wl2)`
pub fn winlink_cmp(wl1: &winlink, wl2: &winlink) -> cmp::Ordering {
    wl1.idx.cmp(&wl2.idx)
}

/// C `vendor/tmux/window.c:103`: `int window_pane_cmp(struct window_pane *wp1, struct window_pane *wp2)`
pub fn window_pane_cmp(wp1: &window_pane, wp2: &window_pane) -> cmp::Ordering {
    wp1.id.cmp(&wp2.id)
}

/// C `vendor/tmux/window.c:109`: `struct winlink *winlink_find_by_window(struct winlinks *wwl, struct window *w)`
pub unsafe fn winlink_find_by_window(
    wwl: *mut winlinks,
    w: *mut window,
) -> Option<NonNull<winlink>> {
    unsafe {
        for wl in rb_foreach(wwl) {
            if (*wl.as_ptr()).window == w {
                return Some(wl);
            }
        }
        None
    }
}

/// C `vendor/tmux/window.c:122`: `struct winlink *winlink_find_by_index(struct winlinks *wwl, int idx)`
pub unsafe fn winlink_find_by_index(wwl: *mut winlinks, idx: i32) -> *mut winlink {
    unsafe {
        if idx < 0 {
            fatalx("bad index");
        }

        let mut wl: winlink = std::mem::zeroed();
        wl.idx = idx;

        rb_find(wwl, &raw mut wl)
    }
}

/// C `vendor/tmux/window.c:134`: `struct winlink *winlink_find_by_window_id(struct winlinks *wwl, u_int id)`
pub unsafe fn winlink_find_by_window_id(wwl: *mut winlinks, id: u32) -> *mut winlink {
    unsafe {
        for wl in rb_foreach(wwl).map(NonNull::as_ptr) {
            if (*(*wl).window).id == id {
                return wl;
            }
        }

        null_mut()
    }
}

/// C `vendor/tmux/window.c:146`: `static int winlink_next_index(struct winlinks *wwl, int idx)`
unsafe fn winlink_next_index(wwl: *mut winlinks, idx: i32) -> i32 {
    let mut i = idx;

    loop {
        if unsafe { winlink_find_by_index(wwl, i).is_null() } {
            return i;
        }

        if i == i32::MAX {
            i = 0;
        } else {
            i += 1;
        }

        if i == idx {
            break;
        }
    }

    -1
}

/// C `vendor/tmux/window.c:163`: `u_int winlink_count(struct winlinks *wwl)`
pub unsafe fn winlink_count(wwl: *mut winlinks) -> u32 {
    unsafe { rb_foreach(wwl).count() as u32 }
}

/// C `vendor/tmux/window.c:176`: `struct winlink *winlink_add(struct winlinks *wwl, int idx)`
pub unsafe fn winlink_add(wwl: *mut winlinks, mut idx: i32) -> *mut winlink {
    unsafe {
        if idx < 0 {
            idx = winlink_next_index(wwl, -idx - 1);
            if idx == -1 {
                return null_mut();
            }
        } else if !winlink_find_by_index(wwl, idx).is_null() {
            return null_mut();
        }

        let wl: *mut winlink = xcalloc_::<winlink>(1).as_ptr();
        (*wl).idx = idx;
        rb_insert(wwl, wl);

        wl
    }
}

/// C `vendor/tmux/window.c:194`: `void winlink_set_window(struct winlink *wl, struct window *w)`
pub unsafe fn winlink_set_window(wl: *mut winlink, w: *mut window) {
    unsafe {
        if !(*wl).window.is_null() {
            tailq_remove::<_, discr_wentry>(&raw mut (*(*wl).window).winlinks, wl);
            window_remove_ref((*wl).window, c!("winlink_set_window"));
        }
        tailq_insert_tail::<_, discr_wentry>(&raw mut (*w).winlinks, wl);
        (*wl).window = w;
        window_add_ref(w, c!("winlink_set_window"));
    }
}

/// C `vendor/tmux/window.c:206`: `void winlink_remove(struct winlinks *wwl, struct winlink *wl)`
pub unsafe fn winlink_remove(wwl: *mut winlinks, wl: *mut winlink) {
    unsafe {
        let w = (*wl).window;

        if !w.is_null() {
            tailq_remove::<_, discr_wentry>(&raw mut (*w).winlinks, wl);
            window_remove_ref(w, c!("winlink_remove"));
        }

        rb_remove(wwl, wl);
        free(wl as _);
    }
}

/// C `vendor/tmux/window.c:220`: `struct winlink *winlink_next(struct winlink *wl)`
pub unsafe fn winlink_next(wl: *mut winlink) -> *mut winlink {
    unsafe { rb_next(wl) }
}

/// C `vendor/tmux/window.c:226`: `struct winlink *winlink_previous(struct winlink *wl)`
pub unsafe fn winlink_previous(wl: *mut winlink) -> *mut winlink {
    unsafe { rb_prev(wl) }
}

/// C `vendor/tmux/window.c:232`: `struct winlink *winlink_next_by_number(struct winlink *wl, struct session *s, int n)`
pub unsafe fn winlink_next_by_number(
    mut wl: *mut winlink,
    s: *mut session,
    n: i32,
) -> *mut winlink {
    unsafe {
        for _ in 0..n {
            wl = rb_next(wl);
            if wl.is_null() {
                wl = rb_min(&raw mut (*s).windows);
            }
        }
    }

    wl
}

/// C `vendor/tmux/window.c:243`: `struct winlink *winlink_previous_by_number(struct winlink *wl, struct session *s, int n)`
pub unsafe fn winlink_previous_by_number(
    mut wl: *mut winlink,
    s: *mut session,
    n: i32,
) -> *mut winlink {
    unsafe {
        for _ in 0..n {
            wl = rb_prev(wl);
            if wl.is_null() {
                wl = rb_min(&raw mut (*s).windows);
            }
        }
    }

    wl
}

/// C `vendor/tmux/window.c:254`: `void winlink_stack_push(struct winlink_stack *stack, struct winlink *wl)`
pub unsafe fn winlink_stack_push(stack: *mut winlink_stack, wl: *mut winlink) {
    if wl.is_null() {
        return;
    }

    unsafe {
        winlink_stack_remove(stack, wl);
        tailq_insert_head::<_, discr_sentry>(stack, wl);
        (*wl).flags |= winlink_flags::WINLINK_VISITED;
    }
}

/// C `vendor/tmux/window.c:265`: `void winlink_stack_remove(struct winlink_stack *stack, struct winlink *wl)`
pub unsafe fn winlink_stack_remove(stack: *mut winlink_stack, wl: *mut winlink) {
    unsafe {
        if !wl.is_null() && (*wl).flags.intersects(winlink_flags::WINLINK_VISITED) {
            tailq_remove::<_, discr_sentry>(stack, wl);
            (*wl).flags &= !winlink_flags::WINLINK_VISITED;
        }
    }
}

/// C `vendor/tmux/window.c:274`: `struct window *window_find_by_id_str(const char *s)`
pub unsafe fn window_find_by_id_str(s: &str) -> *mut window {
    unsafe {
        if !s.starts_with('@') {
            return null_mut();
        }

        let Ok(id) = strtonum_(&s[1..], 0, u32::MAX) else {
            return null_mut();
        };

        window_find_by_id(id)
    }
}

/// C `vendor/tmux/window.c:289`: `struct window *window_find_by_id(u_int id)`
pub unsafe fn window_find_by_id(id: u32) -> *mut window {
    unsafe {
        let mut w: window = std::mem::zeroed();

        w.id = id;
        rb_find(&raw mut WINDOWS, &raw mut w)
    }
}

/// C `vendor/tmux/window.c:298`: `void window_update_activity(struct window *w)`
pub unsafe fn window_update_activity(w: NonNull<window>) {
    unsafe {
        gettimeofday(&raw mut (*w.as_ptr()).activity_time, null_mut());
        alerts_queue(w, window_flag::ACTIVITY);
    }
}

/// C `vendor/tmux/window.c:305`: `struct window *window_create(u_int sx, u_int sy, u_int xpixel, u_int ypixel)`
impl window {
    /// Borrowed `char *` to the window name, or NULL if transiently unset.
    #[inline]
    pub(crate) fn name_ptr(&self) -> *const u8 {
        match &self.name {
            Some(c) => c.as_ptr().cast(),
            None => std::ptr::null(),
        }
    }
}

/// C `vendor/tmux/window.c:158`: `struct window *window_create(u_int sx, u_int sy, u_int xpixel, u_int ypixel)`
pub unsafe fn window_create(sx: u32, sy: u32, mut xpixel: u32, mut ypixel: u32) -> *mut window {
    static NEXT_WINDOW_ID: AtomicU32 = AtomicU32::new(0);

    if xpixel == 0 {
        xpixel = DEFAULT_XPIXEL;
    }
    if ypixel == 0 {
        ypixel = DEFAULT_YPIXEL;
    }
    unsafe {
        let w: *mut window = xcalloc_::<window>(1).as_ptr();
        (*w).name = Some(c"".to_owned());
        (*w).flags = window_flag::empty();

        tailq_init(&raw mut (*w).panes);
        tailq_init(&raw mut (*w).z_index);
        tailq_init(&raw mut (*w).last_panes);
        (*w).active = null_mut();

        (*w).lastlayout = -1;
        (*w).layout_root = null_mut();

        (*w).sx = sx;
        (*w).sy = sy;
        (*w).manual_sx = sx;
        (*w).manual_sy = sy;
        (*w).xpixel = xpixel;
        (*w).ypixel = ypixel;

        (*w).options = options_create(GLOBAL_W_OPTIONS);

        (*w).references = 0;
        tailq_init(&raw mut (*w).winlinks);

        (*w).id = NEXT_WINDOW_ID.fetch_add(1, atomic::Ordering::Relaxed);
        rb_insert(&raw mut WINDOWS, w);

        window_set_fill_character(NonNull::new_unchecked(w));

        if gettimeofday(&raw mut (*w).creation_time, null_mut()) != 0 {
            fatal("gettimeofday failed");
        }
        window_update_activity(NonNull::new_unchecked(w));

        log_debug!(
            "{}: @{} create {}x{} ({}x{})",
            "window_create",
            (*w).id,
            sx,
            sy,
            (*w).xpixel,
            (*w).ypixel,
        );
        w
    }
}

/// C `vendor/tmux/window.c:353`: `static void window_destroy(struct window *w)`
unsafe fn window_destroy(w: *mut window) {
    unsafe {
        log_debug!(
            "window @{} destroyed ({} references)",
            (*w).id,
            (*w).references
        );

        window_unzoom(w, 0);
        rb_remove(&raw mut WINDOWS, w);

        if !(*w).layout_root.is_null() {
            layout_free_cell((*w).layout_root, 0);
        }
        if !(*w).saved_layout_root.is_null() {
            layout_free_cell((*w).saved_layout_root, 0);
        }
        (*w).old_layout = None;

        window_destroy_panes(w);

        if event_initialized(&raw mut (*w).name_event) != 0 {
            event_del(&raw mut (*w).name_event);
        }

        if event_initialized(&raw mut (*w).alerts_timer) != 0 {
            event_del(&raw mut (*w).alerts_timer);
        }
        if event_initialized(&raw mut (*w).offset_timer) != 0 {
            event_del(&raw mut (*w).offset_timer);
        }

        options_free((*w).options);
        free((*w).fill_character as _);

        (*w).name = None;
        free(w as _);
    }
}

/// C `vendor/tmux/window.c:382`: `int window_pane_destroy_ready(struct window_pane *wp)`
pub unsafe fn window_pane_destroy_ready(wp: *mut window_pane) -> bool {
    let mut n = 0;
    unsafe {
        if (*wp).pipe_fd != -1 {
            if EVBUFFER_LENGTH((*(*wp).pipe_event).output) != 0 {
                return false;
            }
            if ioctl((*wp).fd, FIONREAD, &raw mut n) != -1 && n > 0 {
                return false;
            }
        }

        if !(*wp).flags.intersects(window_pane_flags::PANE_EXITED) {
            return false;
        }
    }

    true
}

/// C `vendor/tmux/window.c:406`: `void window_add_ref(struct window *w, const char *from)`
pub unsafe fn window_add_ref(w: *mut window, from: *const u8) {
    unsafe {
        (*w).references += 1;
        log_debug!(
            "{}: @{} {}, now {}",
            "window_add_ref",
            (*w).id,
            _s(from),
            (*w).references,
        );
    }
}

/// C `vendor/tmux/window.c:413`: `void window_remove_ref(struct window *w, const char *from)`
pub unsafe fn window_remove_ref(w: *mut window, from: *const u8) {
    unsafe {
        (*w).references -= 1;
        log_debug!(
            "{}: @{} {}, now {}",
            "window_remove_ref",
            (*w).id,
            _s(from),
            (*w).references,
        );

        if (*w).references == 0 {
            window_destroy(w);
        }
    }
}

/// C `vendor/tmux/window.c:423`: `void window_set_name(struct window *w, const char *new_name, int untrusted)`
pub unsafe fn window_set_name(w: *mut window, new_name: *const u8) {
    unsafe {
        // utf8_stravis writes a freshly-allocated char* to the out-param;
        // capture it and adopt into the owned field (dropping the old name).
        let mut namebuf: *mut u8 = null_mut();
        utf8_stravis(
            &raw mut namebuf,
            new_name,
            vis_flags::VIS_OCTAL | vis_flags::VIS_CSTYLE | vis_flags::VIS_TAB | vis_flags::VIS_NL,
        );
        (*w).name = Some(std::ffi::CString::from_raw(namebuf.cast()));
        notify_window(c"window-renamed", w);
    }
}

/// C `vendor/tmux/window.c:436`: `void window_resize(struct window *w, u_int sx, u_int sy, int xpixel, int ypixel)`
pub unsafe fn window_resize(w: *mut window, sx: u32, sy: u32, mut xpixel: i32, mut ypixel: i32) {
    if xpixel == 0 {
        xpixel = DEFAULT_XPIXEL as i32;
    }
    if ypixel == 0 {
        ypixel = DEFAULT_YPIXEL as i32;
    }

    unsafe {
        log_debug!(
            "{}: @{} resize {}x{} ({}x{})",
            "window_resize",
            (*w).id,
            sx,
            sy,
            if xpixel == -1 {
                (*w).xpixel
            } else {
                xpixel as u32
            },
            if ypixel == -1 {
                (*w).ypixel
            } else {
                ypixel as u32
            },
        );

        (*w).sx = sx;
        (*w).sy = sy;
        if xpixel != -1 {
            (*w).xpixel = xpixel as u32;
        }
        if ypixel != -1 {
            (*w).ypixel = ypixel as u32;
        }
    }
}

/// C `vendor/tmux/window.c:455`: `void window_pane_send_resize(struct window_pane *wp, u_int sx, u_int sy)`
pub unsafe fn window_pane_send_resize(wp: *mut window_pane, sx: u32, sy: u32) {
    unsafe {
        let w = (*wp).window;
        let mut ws: winsize = core::mem::zeroed();

        if (*wp).fd == -1 {
            return;
        }

        log_debug!(
            "{}: %%{} resize to {},{}",
            "window_pane_send_resize",
            (*wp).id,
            sx,
            sy,
        );

        memset(&raw mut ws as _, 0, size_of::<winsize>());

        ws.ws_col = sx as u16;
        ws.ws_row = sy as u16;
        ws.ws_xpixel = (*w).xpixel as u16 * ws.ws_col;
        ws.ws_ypixel = (*w).ypixel as u16 * ws.ws_row;

        // TODO sun ifdef

        if ioctl((*wp).fd, TIOCSWINSZ, &ws) == -1 {
            fatal("ioctl failed");
        }
    }
}

/// C `vendor/tmux/window.c:496`: `int window_has_pane(struct window *w, struct window_pane *wp)`
pub unsafe fn window_has_pane(w: *mut window, wp: *mut window_pane) -> bool {
    unsafe { tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).any(|wp1| wp1.as_ptr() == wp) }
}

/// C `vendor/tmux/window.c:508`: `void window_update_focus(struct window *w)`
pub unsafe fn window_update_focus(w: *mut window) {
    unsafe {
        if !w.is_null() {
            log_debug!("{}: @{}", "window_update_focus", (*w).id);
            window_pane_update_focus((*w).active);
        }
    }
}

/// C `vendor/tmux/window.c:517`: `void window_pane_update_focus(struct window_pane *wp)`
pub unsafe fn window_pane_update_focus(wp: *mut window_pane) {
    unsafe {
        let mut focused = false;

        if !wp.is_null() && !(*wp).flags.intersects(window_pane_flags::PANE_EXITED) {
            if wp != (*(*wp).window).active {
                focused = false;
            } else {
                for c in tailq_foreach(&raw mut CLIENTS).map(NonNull::as_ptr) {
                    if !(*c).session.is_null()
                        && (*(*c).session).attached != 0
                        && (*c).flags.intersects(client_flag::FOCUSED)
                        && (*(*(*c).session).curw).window == (*wp).window
                    {
                        focused = true;
                        break;
                    }
                }
            }
            if !focused && (*wp).flags.intersects(window_pane_flags::PANE_FOCUSED) {
                log_debug!("{}: %%{} focus out", "window_pane_update_focus", (*wp).id);
                if (*wp).base.mode.intersects(mode_flag::MODE_FOCUSON) {
                    bufferevent_write((*wp).event, c!("\x1b[O") as _, 3);
                }
                notify_pane(c"pane-focus-out", wp);
                (*wp).flags &= !window_pane_flags::PANE_FOCUSED;
            } else if focused && !(*wp).flags.intersects(window_pane_flags::PANE_FOCUSED) {
                log_debug!("{}: %%{} focus in", "window_pane_update_focus", (*wp).id);
                if (*wp).base.mode.intersects(mode_flag::MODE_FOCUSON) {
                    bufferevent_write((*wp).event, c!("\x1b[I") as _, 3);
                }
                notify_pane(c"pane-focus-in", wp);
                (*wp).flags |= window_pane_flags::PANE_FOCUSED;
            } else {
                log_debug!(
                    "{}: %%{} focus unchanged",
                    "window_pane_update_focus",
                    (*wp).id,
                );
            }
        }
    }
}

/// C `vendor/tmux/window.c:555`: `int window_set_active_pane(struct window *w, struct window_pane *wp, int notify)`
pub unsafe fn window_set_active_pane(w: *mut window, wp: *mut window_pane, notify: i32) -> i32 {
    static NEXT_ACTIVE_POINT: AtomicU32 = AtomicU32::new(0);

    let lastwp: *mut window_pane;
    unsafe {
        log_debug!("{}: pane %%{}", "window_set_active_pane", (*wp).id);

        if wp == (*w).active {
            return 0;
        }
        lastwp = (*w).active;

        window_pane_stack_remove(&raw mut (*w).last_panes, wp);
        window_pane_stack_push(&raw mut (*w).last_panes, lastwp);

        (*w).active = wp;
        (*(*w).active).active_point = NEXT_ACTIVE_POINT.fetch_add(1, atomic::Ordering::Relaxed);
        (*(*w).active).flags |= window_pane_flags::PANE_CHANGED;

        if options_get_number___::<i64>(&*GLOBAL_OPTIONS, "focus-events") != 0 {
            window_pane_update_focus(lastwp);
            window_pane_update_focus((*w).active);
        }

        tty_update_window_offset(w);

        if notify != 0 {
            notify_window(c"window-pane-changed", w);
        }
    }
    1
}

/// C `vendor/tmux/window.c:588`: `static int window_pane_get_palette(struct window_pane *wp, int c)`
fn window_pane_get_palette(wp: Option<&window_pane>, c: i32) -> i32 {
    if let Some(wp) = wp {
        colour_palette_get(Some(&wp.palette), c)
    } else {
        -1
    }
}

/// C `vendor/tmux/window.c:596`: `void window_redraw_active_switch(struct window *w, struct window_pane *wp)`
pub unsafe fn window_redraw_active_switch(w: *mut window, mut wp: *mut window_pane) {
    unsafe {
        if wp == (*w).active {
            return;
        }

        loop {
            // If the active and inactive styles or palettes are different,
            // need to redraw the panes.
            let gc1 = &raw mut (*wp).cached_gc;
            let gc2 = &raw mut (*wp).cached_active_gc;
            if grid_cells_look_equal(gc1, gc2) == 0 {
                (*wp).flags |= window_pane_flags::PANE_REDRAW;
            } else {
                let mut c1 = window_pane_get_palette(ptr_to_ref(wp), (*gc1).fg);
                let mut c2 = window_pane_get_palette(ptr_to_ref(wp), (*gc2).fg);
                if c1 != c2 {
                    (*wp).flags |= window_pane_flags::PANE_REDRAW;
                } else {
                    c1 = window_pane_get_palette(ptr_to_ref(wp), (*gc1).bg);
                    c2 = window_pane_get_palette(ptr_to_ref(wp), (*gc2).bg);
                    if c1 != c2 {
                        (*wp).flags |= window_pane_flags::PANE_REDRAW;
                    }
                }
            }
            if wp == (*w).active {
                break;
            }
            wp = (*w).active;
        }
    }
}

/// C `vendor/tmux/window.c:645`: `struct window_pane *window_get_active_at(struct window *w, u_int x, u_int y)`
pub unsafe fn window_get_active_at(w: *mut window, x: u32, y: u32) -> *mut window_pane {
    unsafe {
        for wp in tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).map(NonNull::as_ptr) {
            if !window_pane_visible(wp) {
                continue;
            }
            if x < (*wp).xoff || x > (*wp).xoff + (*wp).sx {
                continue;
            }
            if y < (*wp).yoff || y > (*wp).yoff + (*wp).sy {
                continue;
            }
            return wp;
        }
        null_mut()
    }
}

/// C `vendor/tmux/window.c:710`: `struct window_pane *window_find_string(struct window *w, const char *s)`
pub unsafe fn window_find_string(w: *mut window, s: &str) -> *mut window_pane {
    unsafe {
        let mut top: u32 = 0;
        let mut bottom: u32 = (*w).sy - 1;

        let mut x = (*w).sx / 2;
        let mut y = (*w).sy / 2;

        let status: Result<pane_status, _> =
            options_get_number___::<i32>(&*(*w).options, "pane-border-status").try_into();
        match status {
            Ok(pane_status::PANE_STATUS_TOP) => top += 1,
            Ok(pane_status::PANE_STATUS_BOTTOM) => bottom -= 1,
            _ => (),
        }

        if s.eq_ignore_ascii_case("top") {
            y = top;
        } else if s.eq_ignore_ascii_case("bottom") {
            y = bottom;
        } else if s.eq_ignore_ascii_case("left") {
            x = 0;
        } else if s.eq_ignore_ascii_case("right") {
            x = (*w).sx - 1;
        } else if s.eq_ignore_ascii_case("top-left") {
            x = 0;
            y = top;
        } else if s.eq_ignore_ascii_case("top-right") {
            x = (*w).sx - 1;
            y = top;
        } else if s.eq_ignore_ascii_case("bottom-left") {
            x = 0;
            y = bottom;
        } else if s.eq_ignore_ascii_case("bottom-right") {
            x = (*w).sx - 1;
            y = bottom;
        } else {
            return null_mut();
        }

        window_get_active_at(w, x, y)
    }
}

/// C `vendor/tmux/window.c:751`: `int window_zoom(struct window_pane *wp)`
pub unsafe fn window_zoom(wp: *mut window_pane) -> i32 {
    unsafe {
        let w = (*wp).window;

        if (*w).flags.intersects(window_flag::ZOOMED) {
            return -1;
        }

        if window_count_panes(w) == 1 {
            return -1;
        }

        if (*w).active != wp {
            window_set_active_pane(w, wp, 1);
        }
        (*wp).flags |= window_pane_flags::PANE_ZOOMED;

        for wp1 in tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).map(NonNull::as_ptr) {
            (*wp1).saved_layout_cell = (*wp1).layout_cell;
            (*wp1).layout_cell = null_mut();
        }

        (*w).saved_layout_root = (*w).layout_root;
        layout_init(w, wp);
        (*w).flags |= window_flag::ZOOMED;
        notify_window(c"window-layout-changed", w);

        0
    }
}

/// C `vendor/tmux/window.c:780`: `int window_unzoom(struct window *w, int notify)`
pub unsafe fn window_unzoom(w: *mut window, notify: i32) -> i32 {
    unsafe {
        if !(*w).flags.intersects(window_flag::ZOOMED) {
            return -1;
        }

        (*w).flags &= !window_flag::ZOOMED;
        layout_free(w, 0);
        (*w).layout_root = (*w).saved_layout_root;
        (*w).saved_layout_root = null_mut();

        for wp in tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).map(NonNull::as_ptr) {
            (*wp).layout_cell = (*wp).saved_layout_cell;
            (*wp).saved_layout_cell = null_mut();
            (*wp).flags &= !window_pane_flags::PANE_ZOOMED;
        }
        layout_fix_panes(w, null_mut());

        if notify != 0 {
            notify_window(c"window-layout-changed", w);
        }

        0
    }
}

/// C `vendor/tmux/window.c:481`: `const char *window_pane_printable_flags(struct window_pane *wp)`
/// The floating-pane (`F`) branch is omitted because ztmux has no floating
/// panes, so no pane is ever floating; output is identical to C for every
/// reachable pane state.
pub unsafe fn window_pane_printable_flags(wp: *mut window_pane) -> String {
    unsafe {
        let w = (*wp).window;
        let mut flags = String::new();
        if wp == (*w).active {
            flags.push('*');
        }
        if wp == tailq_first(&raw mut (*w).last_panes) {
            flags.push('-');
        }
        if (*wp).flags.intersects(window_pane_flags::PANE_ZOOMED) {
            flags.push('Z');
        }
        flags
    }
}

/// C `vendor/tmux/window.c:807`: `int window_push_zoom(struct window *w, int always, int flag)`
pub unsafe fn window_push_zoom(w: *mut window, always: bool, flag: bool) -> bool {
    unsafe {
        log_debug!(
            "{}: @{} {}",
            "window_push_zoom",
            (*w).id,
            (flag && (*w).flags.intersects(window_flag::ZOOMED)) as i32,
        );
        if flag && (always || (*w).flags.intersects(window_flag::ZOOMED)) {
            (*w).flags |= window_flag::WASZOOMED;
        } else {
            (*w).flags &= !window_flag::WASZOOMED;
        }

        window_unzoom(w, 1) == 0
    }
}

/// C `vendor/tmux/window.c:819`: `int window_pop_zoom(struct window *w)`
pub unsafe fn window_pop_zoom(w: *mut window) -> bool {
    unsafe {
        log_debug!(
            "{}: @{} {}",
            "window_pop_zoom",
            (*w).id,
            (*w).flags.intersects(window_flag::WASZOOMED) as i32,
        );
        if (*w).flags.intersects(window_flag::WASZOOMED) {
            return window_zoom((*w).active) == 0;
        }
    }

    false
}

/// C `vendor/tmux/window.c:829`: `struct window_pane *window_add_pane(struct window *w, struct window_pane *other, u_int hlimit, int flags)`
pub unsafe fn window_add_pane(
    w: *mut window,
    mut other: *mut window_pane,
    hlimit: u32,
    flags: spawn_flags,
) -> *mut window_pane {
    let func = "window_add_pane";
    unsafe {
        if other.is_null() {
            other = (*w).active;
        }

        let wp = window_pane_create(w, (*w).sx, (*w).sy, hlimit);
        if tailq_empty(&raw mut (*w).panes) {
            log_debug!("{}: @{} at start", func, (*w).id);
            tailq_insert_head::<_, discr_entry>(&raw mut (*w).panes, wp);
        } else if flags.intersects(SPAWN_BEFORE) {
            log_debug!("{}: @{} before %%{}", func, (*w).id, (*wp).id);
            if flags.intersects(SPAWN_FULLSIZE) {
                tailq_insert_head::<_, discr_entry>(&raw mut (*w).panes, wp);
            } else {
                tailq_insert_before::<_, discr_entry>(other, wp);
            }
        } else {
            log_debug!("{}: @{} after %%{}", func, (*w).id, (*wp).id);
            if flags.intersects(SPAWN_FULLSIZE | SPAWN_FLOATING) {
                tailq_insert_tail::<_, discr_entry>(&raw mut (*w).panes, wp);
            } else {
                tailq_insert_after::<_, discr_entry>(&raw mut (*w).panes, other, wp);
            }
        }

        // Track every pane in the z-index list (floating panes go on top).
        if !flags.intersects(SPAWN_FLOATING) {
            tailq_insert_tail::<_, discr_zentry>(&raw mut (*w).z_index, wp);
        } else {
            tailq_insert_head::<_, discr_zentry>(&raw mut (*w).z_index, wp);
        }
        redraw_invalidate_scene(w);

        wp
    }
}

/// C `vendor/tmux/window.c:864`: `void window_lost_pane(struct window *w, struct window_pane *wp)`
pub unsafe fn window_lost_pane(w: *mut window, wp: *mut window_pane) {
    unsafe {
        log_debug!("{}: @{} pane %%{}", "window_lost_pane", (*w).id, (*wp).id);

        if wp == MARKED_PANE.wp {
            server_clear_marked();
        }

        window_pane_stack_remove(&raw mut (*w).last_panes, wp);
        if wp == (*w).active {
            (*w).active = tailq_first(&raw mut (*w).last_panes);
            if (*w).active.is_null() {
                (*w).active = tailq_prev::<_, _, discr_entry>(wp);
                if (*w).active.is_null() {
                    (*w).active = tailq_next::<_, _, discr_entry>(wp);
                }
            }
            if !(*w).active.is_null() {
                window_pane_stack_remove(&raw mut (*w).last_panes, (*w).active);
                (*(*w).active).flags |= window_pane_flags::PANE_CHANGED;
                notify_window(c"window-pane-changed", w);
                window_update_focus(w);
            }
        }
    }
}

/// C `vendor/tmux/window.c:890`: `void window_remove_pane(struct window *w, struct window_pane *wp)`
pub unsafe fn window_remove_pane(w: *mut window, wp: *mut window_pane) {
    unsafe {
        window_lost_pane(w, wp);

        tailq_remove::<_, discr_entry>(&raw mut (*w).panes, wp);
        tailq_remove::<_, discr_zentry>(&raw mut (*w).z_index, wp);
        redraw_invalidate_scene(w);
        window_pane_destroy(wp);
    }
}

// floating-pane API, consumed from the next phase (F1+)
/// C `vendor/tmux/window.c`: `int window_pane_is_floating(struct window_pane *wp)`
pub unsafe fn window_pane_is_floating(wp: *mut window_pane) -> c_int {
    unsafe {
        let lc = (*wp).layout_cell;
        if lc.is_null() || (*lc).flags & LAYOUT_CELL_FLOATING == 0 {
            return 0;
        }
        1
    }
}

#[expect(dead_code)] // floating-pane API, consumed from the next phase (F1+)
/// C `vendor/tmux/window.c`: `int window_has_floating_panes(struct window *w)`
pub unsafe fn window_has_floating_panes(w: *mut window) -> c_int {
    unsafe {
        for wp in tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).map(NonNull::as_ptr) {
            if window_pane_is_floating(wp) != 0 {
                return 1;
            }
        }
        0
    }
}

/// C `vendor/tmux/window.c`: `enum pane_lines window_get_pane_lines(struct window *w)`
pub unsafe fn window_get_pane_lines(w: *mut window) -> pane_lines {
    unsafe {
        options_get_number___::<i32>(&*(*w).options, "pane-border-lines")
            .try_into()
            .unwrap_or(pane_lines::PANE_LINES_SINGLE)
    }
}

/// C `vendor/tmux/window.c`: `int window_pane_zindex(struct window_pane *wp, u_int *i)`
///
/// Index of a pane counting only non-floating panes ahead of it in the z-index
/// list; returns -1 if the pane is not found.
pub unsafe fn window_pane_zindex(wp: *mut window_pane, i: *mut u32) -> c_int {
    unsafe {
        let w = (*wp).window;
        *i = 0;
        for wq in tailq_foreach::<_, discr_zentry>(&raw mut (*w).z_index).map(NonNull::as_ptr) {
            if wq == wp {
                if window_pane_is_floating(wp) == 0 {
                    *i += 1;
                }
                return 0;
            }
            if window_pane_is_floating(wq) != 0 {
                *i += 1;
            }
        }
        -1
    }
}

/// C `vendor/tmux/window.c:900`: `struct window_pane *window_pane_at_index(struct window *w, u_int idx)`
pub unsafe fn window_pane_at_index(w: *mut window, idx: u32) -> *mut window_pane {
    unsafe {
        let mut n: u32 = options_get_number___::<u32>(&*(*w).options, "pane-base-index");

        for wp in tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).map(NonNull::as_ptr) {
            if n == idx {
                return wp;
            }
            n += 1;
        }

        null_mut()
    }
}

/// C `vendor/tmux/window.c:915`: `struct window_pane *window_pane_next_by_number(struct window *w, struct window_pane *wp, u_int n)`
pub unsafe fn window_pane_next_by_number(
    w: *mut window,
    mut wp: *mut window_pane,
    n: u32,
) -> *mut window_pane {
    unsafe {
        for _ in 0..n {
            wp = tailq_next::<_, _, discr_entry>(wp);
            if wp.is_null() {
                wp = tailq_first(&raw mut (*w).panes);
            }
        }
    }

    wp
}

/// C `vendor/tmux/window.c:926`: `struct window_pane *window_pane_previous_by_number(struct window *w, struct window_pane *wp, u_int n)`
pub unsafe fn window_pane_previous_by_number(
    w: *mut window,
    mut wp: *mut window_pane,
    n: u32,
) -> *mut window_pane {
    unsafe {
        for _ in 0..n {
            wp = tailq_prev::<_, _, discr_entry>(wp);
            if wp.is_null() {
                wp = tailq_last(&raw mut (*w).panes);
            }
        }
    }

    wp
}

/// C `vendor/tmux/window.c:938`: `int window_pane_index(struct window_pane *wp, u_int *i)`
pub unsafe fn window_pane_index(wp: *mut window_pane, i: *mut u32) -> i32 {
    unsafe {
        let w = (*wp).window;

        *i = options_get_number___::<u32>(&*(*w).options, "pane-base-index") as _;
        for wq in tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).map(NonNull::as_ptr) {
            if wp == wq {
                return 0;
            }
            (*i) += 1;
        }
        -1
    }
}

/// C `vendor/tmux/window.c:975`: `u_int window_count_panes(struct window *w, int with_floating)`
pub unsafe fn window_count_panes(w: *mut window) -> u32 {
    unsafe { tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).count() as u32 }
}

/// C `vendor/tmux/window.c:988`: `void window_destroy_panes(struct window *w)`
pub unsafe fn window_destroy_panes(w: *mut window) {
    let mut wp: *mut window_pane;
    unsafe {
        while !tailq_empty(&raw mut (*w).last_panes) {
            wp = tailq_first(&raw mut (*w).last_panes);
            window_pane_stack_remove(&raw mut (*w).last_panes, wp);
        }

        while !tailq_empty(&raw mut (*w).panes) {
            wp = tailq_first(&raw mut (*w).panes);
            tailq_remove::<_, discr_entry>(&raw mut (*w).panes, wp);
            window_pane_destroy(wp);
        }
    }
}

/// C `vendor/tmux/window.c:1006`: `const char *window_printable_flags(struct winlink *wl, int escape)`
pub unsafe fn window_printable_flags(wl: *mut winlink, escape: i32) -> *const u8 {
    static mut FLAGS: [u8; 32] = [0; 32];

    unsafe {
        let s = (*wl).session;

        let mut pos = 0;
        if (*wl).flags.intersects(winlink_flags::WINLINK_ACTIVITY) {
            FLAGS[pos] = b'#';
            pos += 1;
            if escape != 0 {
                FLAGS[pos] = b'#';
                pos += 1;
            }
        }
        if (*wl).flags.intersects(winlink_flags::WINLINK_BELL) {
            FLAGS[pos] = b'!';
            pos += 1;
        }
        if (*wl).flags.intersects(winlink_flags::WINLINK_SILENCE) {
            FLAGS[pos] = b'~';
            pos += 1;
        }
        if wl == (*s).curw {
            FLAGS[pos] = b'*';
            pos += 1;
        }
        if wl == tailq_first(&raw mut (*s).lastw) {
            FLAGS[pos] = b'-';
            pos += 1;
        }
        if server_check_marked() && wl == MARKED_PANE.wl {
            FLAGS[pos] = b'M';
            pos += 1;
        }
        if (*(*wl).window).flags.intersects(window_flag::ZOOMED) {
            FLAGS[pos] = b'Z';
            pos += 1;
        }
        FLAGS[pos] = b'\0';
        &raw mut FLAGS as *mut u8
    }
}

/// C `vendor/tmux/window.c:1053`: `struct window_pane *window_pane_find_by_id_str(const char *s)`
pub unsafe fn window_pane_find_by_id_str(s: &str) -> *mut window_pane {
    unsafe {
        if !s.starts_with('%') {
            return null_mut();
        }

        match strtonum_(&s[1..], 0, u32::MAX) {
            Ok(id) => window_pane_find_by_id(id),
            Err(_errstr) => null_mut(),
        }
    }
}

/// C `vendor/tmux/window.c:1068`: `struct window_pane *window_pane_find_by_id(u_int id)`
pub unsafe fn window_pane_find_by_id(id: u32) -> *mut window_pane {
    unsafe {
        let mut wp: window_pane = zeroed();
        wp.id = id;
        rb_find(&raw mut ALL_WINDOW_PANES, &raw mut wp)
    }
}

/// C `vendor/tmux/window.c:1077`: `static struct window_pane *window_pane_create(struct window *w, u_int sx, u_int sy, u_int hlimit)`
pub unsafe fn window_pane_create(
    w: *mut window,
    sx: u32,
    sy: u32,
    hlimit: u32,
) -> *mut window_pane {
    static NEXT_WINDOW_PANE_ID: AtomicU32 = AtomicU32::new(0);

    unsafe {
        let mut host: [u8; HOST_NAME_MAX + 1] = zeroed();
        let wp: *mut window_pane = xcalloc_::<window_pane>(1).as_ptr();
        (*wp).window = w;
        (*wp).options = options_create((*w).options);
        (*wp).flags = window_pane_flags::PANE_STYLECHANGED;

        (*wp).id = NEXT_WINDOW_PANE_ID.fetch_add(1, atomic::Ordering::Relaxed);

        rb_insert(&raw mut ALL_WINDOW_PANES, wp);

        (*wp).fd = -1;

        tailq_init(&raw mut (*wp).modes);

        tailq_init(&raw mut (*wp).resize_queue);

        (*wp).sx = sx;
        (*wp).sy = sy;

        (*wp).pipe_fd = -1;

        (*wp).control_bg = -1;
        (*wp).control_fg = -1;

        (*wp).palette = colour_palette_init();
        colour_palette_from_option(Some(&mut (*wp).palette), (*wp).options);

        screen_init(&raw mut (*wp).base, sx, sy, hlimit);
        (*wp).screen = &raw mut (*wp).base;
        window_pane_default_cursor(wp);

        screen_init(&raw mut (*wp).status_screen, 1, 1, 0);

        if gethostname(host.as_mut_ptr(), size_of_val(&host)) == 0 {
            screen_set_title(&raw mut (*wp).base, host.as_ptr());
        }

        wp
    }
}

impl window_pane {
    /// Borrowed `char *` to the shell path, or NULL if unset.
    #[inline]
    pub(crate) fn shell_ptr(&self) -> *const u8 {
        match &self.shell {
            Some(c) => c.as_ptr().cast(),
            None => std::ptr::null(),
        }
    }
    /// Borrowed `char *` to the working directory, or NULL if unset.
    #[inline]
    pub(crate) fn cwd_ptr(&self) -> *const u8 {
        match &self.cwd {
            Some(c) => c.as_ptr().cast(),
            None => std::ptr::null(),
        }
    }
    /// Borrowed `char *` to the last search string, or NULL if unset.
    #[inline]
    pub(crate) fn searchstr_ptr(&self) -> *const u8 {
        match &self.searchstr {
            Some(c) => c.as_ptr().cast(),
            None => std::ptr::null(),
        }
    }
}

/// C `vendor/tmux/window.c:1205`: `static void window_pane_destroy(struct window_pane *wp)`
unsafe fn window_pane_destroy(wp: *mut window_pane) {
    unsafe {
        window_pane_reset_mode_all(wp);
        (*wp).searchstr = None;

        if (*wp).fd != -1 {
            #[cfg(feature = "utempter")]
            {
                utempter_remove_record((*wp).fd);
            }
            bufferevent_free((*wp).event);
            close((*wp).fd);
        }
        if !(*wp).ictx.is_null() {
            input_free((*wp).ictx);
        }

        screen_free(&raw mut (*wp).status_screen);

        screen_free(&raw mut (*wp).base);

        if (*wp).pipe_fd != -1 {
            bufferevent_free((*wp).pipe_event);
            close((*wp).pipe_fd);
        }

        if event_initialized(&raw mut (*wp).resize_timer) != 0 {
            event_del(&raw mut (*wp).resize_timer);
        }
        if event_initialized(&raw mut (*wp).sync_timer) != 0 {
            event_del(&raw mut (*wp).sync_timer);
        }
        for r in tailq_foreach(&raw mut (*wp).resize_queue).map(NonNull::as_ptr) {
            tailq_remove::<_, ()>(&raw mut (*wp).resize_queue, r);
            free_(r);
        }

        rb_remove(&raw mut ALL_WINDOW_PANES, wp);

        options_free((*wp).options);
        (*wp).cwd = None;
        (*wp).shell = None;
        cmd_free_argv((*wp).argc, (*wp).argv);
        colour_palette_free(Some(&mut (*wp).palette));
        free(wp as _);
    }
}

/// C `vendor/tmux/window.c:1256`: `static void window_pane_read_callback(__unused struct bufferevent *bufev, void *data)`
unsafe extern "C-unwind" fn window_pane_read_callback(_bufev: *mut bufferevent, data: *mut c_void) {
    unsafe {
        let wp: *mut window_pane = data as _;
        let evb: *mut evbuffer = (*(*wp).event).input;
        let wpo: *mut window_pane_offset = &raw mut (*wp).pipe_offset;
        let size = EVBUFFER_LENGTH(evb);
        let mut new_size: usize = 0;

        if (*wp).pipe_fd != -1 {
            let new_data = window_pane_get_new_data(wp, wpo, &raw mut new_size);
            if new_size > 0 {
                bufferevent_write((*wp).pipe_event, new_data, new_size);
                window_pane_update_used_data(wp, wpo, new_size);
            }
        }

        log_debug!("%%{} has {} bytes", (*wp).id, size);
        for c in tailq_foreach(&raw mut CLIENTS).map(NonNull::as_ptr) {
            if !(*c).session.is_null() && (*c).flags.intersects(client_flag::CONTROL) {
                control_write_output(c, wp);
            }
        }
        input_parse_pane(wp);
        bufferevent_disable((*wp).event, EV_READ);
    }
}

/// C `vendor/tmux/window.c:1284`: `static void window_pane_error_callback(__unused struct bufferevent *bufev, __unused short what, void *data)`
unsafe extern "C-unwind" fn window_pane_error_callback(
    _bufev: *mut bufferevent,
    _what: c_short,
    data: *mut c_void,
) {
    let wp: *mut window_pane = data as _;
    unsafe {
        log_debug!("%%{} error", (*wp).id);
        (*wp).flags |= window_pane_flags::PANE_EXITED;

        if window_pane_destroy_ready(wp) {
            server_destroy_pane(wp, 1);
        }
    }
}

/// C `vendor/tmux/window.c:1297`: `void window_pane_set_event(struct window_pane *wp)`
pub unsafe fn window_pane_set_event(wp: *mut window_pane) {
    unsafe {
        setblocking((*wp).fd, 0);

        (*wp).event = bufferevent_new(
            (*wp).fd,
            Some(window_pane_read_callback),
            None,
            Some(window_pane_error_callback),
            wp as _,
        );
        if (*wp).event.is_null() {
            fatalx("out of memory");
        }
        (*wp).ictx = input_init(wp, (*wp).event, &raw mut (*wp).palette);

        bufferevent_enable((*wp).event, EV_READ | EV_WRITE);
    }
}

/// C `vendor/tmux/window.c:1325`: `void window_pane_resize(struct window_pane *wp, u_int sx, u_int sy)`
pub unsafe fn window_pane_resize(wp: *mut window_pane, sx: u32, sy: u32) {
    unsafe {
        if sx == (*wp).sx && sy == (*wp).sy {
            return;
        }

        let r = Box::leak(Box::new(window_pane_resize {
            sx,
            sy,
            osx: (*wp).sx,
            osy: (*wp).sy,
            entry: tailq_entry::default(),
        }));
        tailq_insert_tail(&raw mut (*wp).resize_queue, r);

        (*wp).sx = sx;
        (*wp).sy = sy;

        log_debug!(
            "{}: %%{} resize {}x{}",
            "window_pane_resize",
            (*wp).id,
            sx,
            sy,
        );
        screen_resize(
            &raw mut (*wp).base,
            sx,
            sy,
            (*wp).base.saved_grid.is_null() as i32,
        );

        if let Some(wme) = NonNull::new(tailq_first(&raw mut (*wp).modes)) {
            ((*(*wme.as_ptr()).mode).resize)(wme, sx, sy);
        }
    }
}

/// C `vendor/tmux/window.c:1354`: `int window_pane_set_mode(struct window_pane *wp, struct window_pane *swp, const struct window_mode *mode, struct cmd_find_state *fs, struct args *args)`
pub unsafe fn window_pane_set_mode(
    wp: *mut window_pane,
    swp: *mut window_pane,
    mode: *const window_mode,
    fs: *mut cmd_find_state,
    args: *mut args,
) -> i32 {
    unsafe {
        if !tailq_empty(&raw mut (*wp).modes) && (*tailq_first(&raw mut (*wp).modes)).mode == mode {
            return 1;
        }

        let mut wme: *mut window_mode_entry = null_mut();
        for wme_ in tailq_foreach(&raw mut (*wp).modes).map(NonNull::as_ptr) {
            wme = wme_;
            if (*wme).mode == mode {
                break;
            }
        }

        if !wme.is_null() {
            tailq_remove::<_, ()>(&raw mut (*wp).modes, wme);
            tailq_insert_head(&raw mut (*wp).modes, wme);
        } else {
            wme = xcalloc_::<window_mode_entry>(1).as_ptr();
            (*wme).wp = wp;
            (*wme).swp = swp;
            (*wme).mode = mode;
            (*wme).prefix = 1;
            tailq_insert_head(&raw mut (*wp).modes, wme);
            (*wme).screen = ((*(*wme).mode).init)(NonNull::new_unchecked(wme), fs, args);
        }

        (*wp).screen = (*wme).screen;
        (*wp).flags |= window_pane_flags::PANE_REDRAW | window_pane_flags::PANE_CHANGED;

        server_redraw_window_borders((*wp).window);
        server_status_window((*wp).window);
        notify_pane(c"pane-mode-changed", wp);

        0
    }
}

/// C `vendor/tmux/window.c:1394`: `void window_pane_reset_mode(struct window_pane *wp)`
pub unsafe fn window_pane_reset_mode(wp: *mut window_pane) {
    let func = "window_pane_reset_mode";
    unsafe {
        if tailq_empty(&raw mut (*wp).modes) {
            return;
        }

        let wme = tailq_first(&raw mut (*wp).modes);
        tailq_remove::<_, ()>(&raw mut (*wp).modes, wme);
        ((*(*wme).mode).free)(NonNull::new(wme).unwrap());
        free(wme as _);

        if let Some(next) = NonNull::new(tailq_first(&raw mut (*wp).modes)) {
            log_debug!("{}: next mode is {}", func, (*(*next.as_ptr()).mode).name);
            (*wp).screen = (*next.as_ptr()).screen;
            ((*(*next.as_ptr()).mode).resize)(next, (*wp).sx, (*wp).sy);
        } else {
            (*wp).flags &= !window_pane_flags::PANE_UNSEENCHANGES;
            log_debug!("{}: no next mode", func);
            (*wp).screen = &raw mut (*wp).base;
        }
        (*wp).flags |= window_pane_flags::PANE_REDRAW | window_pane_flags::PANE_CHANGED;

        server_redraw_window_borders((*wp).window);
        server_status_window((*wp).window);
        notify_pane(c"pane-mode-changed", wp);
    }
}

/// C `vendor/tmux/window.c:1434`: `void window_pane_reset_mode_all(struct window_pane *wp)`
pub unsafe fn window_pane_reset_mode_all(wp: *mut window_pane) {
    unsafe {
        while !tailq_empty(&raw mut (*wp).modes) {
            window_pane_reset_mode(wp);
        }
    }
}

/// C `vendor/tmux/window.c:1618`: `static void window_pane_copy_key(struct window_pane *wp, key_code key)`
unsafe fn window_pane_copy_key(wp: *mut window_pane, key: key_code) {
    unsafe {
        for loop_ in
            tailq_foreach::<_, discr_entry>(&raw mut (*(*wp).window).panes).map(NonNull::as_ptr)
        {
            if loop_ != wp
                && tailq_empty(&raw mut (*loop_).modes)
                && (*loop_).fd != -1
                && !(*loop_).flags.intersects(window_pane_flags::PANE_INPUTOFF)
                && window_pane_visible(loop_)
                && options_get_number___::<i64>(&*(*loop_).options, "synchronize-panes") != 0
            {
                input_key_pane(loop_, key, null_mut());
            }
        }
    }
}

/// C `vendor/tmux/window.c:1653`: `int window_pane_key(struct window_pane *wp, struct client *c, struct session *s, struct winlink *wl, key_code key, struct mouse_event *m)`
pub unsafe fn window_pane_key(
    wp: *mut window_pane,
    c: *mut client,
    s: *mut session,
    wl: *mut winlink,
    mut key: key_code,
    m: *mut mouse_event,
) -> i32 {
    if KEYC_IS_MOUSE(key) && m.is_null() {
        return -1;
    }
    unsafe {
        if let Some(wme) = NonNull::new(tailq_first(&raw mut (*wp).modes))
            && let Some(key_fn) = (*(*wme.as_ptr()).mode).key
            && !c.is_null()
        {
            key &= !KEYC_MASK_FLAGS;
            key_fn(wme, c, s, wl, key, m);
            return 0;
        }

        if (*wp).fd == -1 || (*wp).flags.intersects(window_pane_flags::PANE_INPUTOFF) {
            return 0;
        }

        if input_key_pane(wp, key, m) != 0 {
            return -1;
        }

        if KEYC_IS_MOUSE(key) {
            return 0;
        }
        if options_get_number___::<i64>(&*(*wp).options, "synchronize-panes") != 0 {
            window_pane_copy_key(wp, key);
        }
    }

    0
}

pub unsafe fn window_pane_visible(wp: *const window_pane) -> bool {
    unsafe {
        if !(*(*wp).window).flags.intersects(window_flag::ZOOMED) {
            return true;
        }
        std::ptr::eq(wp, (*(*wp).window).active)
    }
}

/// C `vendor/tmux/window.c:1698`: `int window_pane_exited(struct window_pane *wp)`
pub unsafe fn window_pane_exited(wp: *mut window_pane) -> bool {
    unsafe { (*wp).fd == -1 || (*wp).flags.intersects(window_pane_flags::PANE_EXITED) }
}

/// C `vendor/tmux/window.c:1704`: `u_int window_pane_search(struct window_pane *wp, const char *term, int regex, int ignore)`
pub unsafe fn window_pane_search(
    wp: *mut window_pane,
    term: *const u8,
    regex: i32,
    ignore: i32,
) -> u32 {
    unsafe {
        let s: *mut screen = &raw mut (*wp).base;
        let mut r: regex_t = zeroed();
        let mut new: *mut u8 = null_mut();
        let mut flags = 0;

        if regex == 0 {
            if ignore != 0 {
                flags |= FNM_CASEFOLD;
            }
            new = format_nul!("*{}*", _s(term));
        } else {
            if ignore != 0 {
                flags |= REG_ICASE;
            }
            if regcomp(&raw mut r, term, flags | REG_EXTENDED) != 0 {
                return 0;
            }
        }

        // C uses a `for (i = 0; i < sy; i++)` loop and relies on `i` equalling
        // `sy` after a no-match run (the sentinel below). A Rust `for j in
        // 0..sy` loop leaves `i` at `sy - 1`, so use an explicit while loop.
        let mut i = 0;
        while i < screen_size_y(s) {
            let line = grid_view_string_cells((*s).grid, 0, i, screen_size_x(s));
            for n in (1..=strlen(line)).rev() {
                if isspace(line.add(n - 1) as c_uchar as c_int) == 0 {
                    break;
                }
                *line.add(n - 1) = b'\0' as _;
            }

            log_debug!("{}: {}", "window_pane_search", _s(line));
            let found = if regex == 0 {
                fnmatch(new, line, flags) == 0
            } else {
                regexec(&r, line, 0, null_mut(), 0) == 0
            };
            free(line as _);

            if found {
                break;
            }
            i += 1;
        }

        if regex == 0 {
            free(new as _);
        } else {
            regfree(&raw mut r);
        }

        if i == screen_size_y(s) {
            return 0;
        }

        i + 1
    }
}

/// Get MRU pane from a list.
/// C `vendor/tmux/window.c:1753`: `static struct window_pane *window_pane_choose_best(struct window_pane **list, u_int size)`
unsafe fn window_pane_choose_best(list: *mut *mut window_pane, size: u32) -> *mut window_pane {
    if size == 0 {
        return null_mut();
    }

    unsafe {
        let mut best = *list;
        for i in 1..size {
            let next = *list.add(i as usize);
            if (*next).active_point > (*best).active_point {
                best = next;
            }
        }
        best
    }
}

/// Find the pane directly above another. We build a list of those adjacent to top edge and then choose the best.
/// C `vendor/tmux/window.c:1801`: `struct window_pane *window_pane_find_up(struct window_pane *wp)`
pub unsafe fn window_pane_find_up(wp: *mut window_pane) -> *mut window_pane {
    unsafe {
        if wp.is_null() {
            return null_mut();
        }
        let w = (*wp).window;
        let status: pane_status = options_get_number___::<i32>(&*(*w).options, "pane-border-status")
            .try_into()
            .unwrap();

        let mut list: *mut *mut window_pane = null_mut();
        let mut size = 0;

        let mut edge = (*wp).yoff;
        match status {
            pane_status::PANE_STATUS_TOP => {
                if edge == 1 {
                    edge = (*w).sy + 1;
                }
            }
            pane_status::PANE_STATUS_BOTTOM => {
                if edge == 0 {
                    edge = (*w).sy;
                }
            }
            _ => {
                if edge == 0 {
                    edge = (*w).sy + 1;
                }
            }
        }

        let left = (*wp).xoff;
        let right = (*wp).xoff + (*wp).sx;

        for next in tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).map(NonNull::as_ptr) {
            if next == wp {
                continue;
            }
            if (*next).yoff + (*next).sy + 1 != edge {
                continue;
            }
            let end = (*next).xoff + (*next).sx - 1;

            let mut found = 0;
            #[expect(clippy::if_same_then_else)]
            if (*next).xoff < left && end > right {
                found = 1;
            } else if (*next).xoff >= left && (*next).xoff <= right {
                found = 1;
            } else if end >= left && end <= right {
                found = 1;
            }
            if found == 0 {
                continue;
            }
            list = xreallocarray_::<*mut window_pane>(list, size + 1).as_ptr();
            *list.add(size) = next;
            size += 1;
        }

        let best = window_pane_choose_best(list, size as u32);
        free(list as _);
        best
    }
}

/// Find the pane directly below another.
/// C `vendor/tmux/window.c:1862`: `struct window_pane *window_pane_find_down(struct window_pane *wp)`
pub unsafe fn window_pane_find_down(wp: *mut window_pane) -> *mut window_pane {
    unsafe {
        if wp.is_null() {
            return null_mut();
        }
        let w = (*wp).window;
        let status: pane_status = options_get_number___::<i32>(&*(*w).options, "pane-border-status")
            .try_into()
            .unwrap();

        let mut list: *mut *mut window_pane = null_mut();
        let mut size = 0;

        let mut edge = (*wp).yoff + (*wp).sy + 1;
        match status {
            pane_status::PANE_STATUS_TOP => {
                if edge >= (*w).sy {
                    edge = 1;
                }
            }
            pane_status::PANE_STATUS_BOTTOM => {
                if edge >= (*w).sy - 1 {
                    edge = 0;
                }
            }
            _ => {
                if edge >= (*w).sy {
                    edge = 0;
                }
            }
        }

        let left = (*wp).xoff;
        let right = (*wp).xoff + (*wp).sx;

        for next in tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).map(NonNull::as_ptr) {
            if next == wp {
                continue;
            }
            if (*next).yoff != edge {
                continue;
            }
            let end = (*next).xoff + (*next).sx - 1;

            let mut found = 0;
            #[expect(clippy::if_same_then_else)]
            if (*next).xoff < left && end > right {
                found = 1;
            } else if (*next).xoff >= left && (*next).xoff <= right {
                found = 1;
            } else if end >= left && end <= right {
                found = 1;
            }
            if found == 0 {
                continue;
            }
            list = xreallocarray_::<*mut window_pane>(list, size + 1).as_ptr();
            *list.add(size) = next;
            size += 1;
        }

        let best = window_pane_choose_best(list, size as u32);
        free(list as _);
        best
    }
}

/// Find the pane directly to the left of another.
/// C `vendor/tmux/window.c:1923`: `struct window_pane *window_pane_find_left(struct window_pane *wp)`
pub unsafe fn window_pane_find_left(wp: *mut window_pane) -> *mut window_pane {
    if wp.is_null() {
        return null_mut();
    }
    unsafe {
        let w = (*wp).window;

        let mut list: *mut *mut window_pane = null_mut();
        let mut size = 0;

        let mut edge = (*wp).xoff;
        if edge == 0 {
            edge = (*w).sx + 1;
        }

        let top = (*wp).yoff;
        let bottom = (*wp).yoff + (*wp).sy;

        for next in tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).map(NonNull::as_ptr) {
            if next == wp {
                continue;
            }
            if (*next).xoff + (*next).sx + 1 != edge {
                continue;
            }
            let end = (*next).yoff + (*next).sy - 1;

            let mut found = false;
            #[expect(clippy::if_same_then_else)]
            if (*next).yoff < top && end > bottom {
                found = true;
            } else if (*next).yoff >= top && (*next).yoff <= bottom {
                found = true;
            } else if end >= top && end <= bottom {
                found = true;
            }
            if !found {
                continue;
            }
            list = xreallocarray_::<*mut window_pane>(list, size + 1).as_ptr();
            *list.add(size) = next;
            size += 1;
        }

        let best = window_pane_choose_best(list, size as u32);
        free(list as _);
        best
    }
}

/// Find the pane directly to the right of another.
/// C `vendor/tmux/window.c:1975`: `struct window_pane *window_pane_find_right(struct window_pane *wp)`
pub unsafe fn window_pane_find_right(wp: *mut window_pane) -> *mut window_pane {
    if wp.is_null() {
        return null_mut();
    }
    unsafe {
        let w = (*wp).window;

        let mut list: *mut *mut window_pane = null_mut();
        let mut size = 0;

        let mut edge = (*wp).xoff + (*wp).sx + 1;
        if edge >= (*w).sx {
            edge = 0;
        }

        let top = (*wp).yoff;
        let bottom = (*wp).yoff + (*wp).sy;

        for next in tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).map(NonNull::as_ptr) {
            if next == wp {
                continue;
            }
            if (*next).xoff != edge {
                continue;
            }
            let end = (*next).yoff + (*next).sy - 1;

            let mut found = false;
            #[expect(clippy::if_same_then_else)]
            if (*next).yoff < top && end > bottom {
                found = true;
            } else if (*next).yoff >= top && (*next).yoff <= bottom {
                found = true;
            } else if end >= top && end <= bottom {
                found = true;
            }
            if !found {
                continue;
            }
            list = xreallocarray_::<*mut window_pane>(list, size + 1).as_ptr();
            *list.add(size) = next;
            size += 1;
        }

        let best = window_pane_choose_best(list, size as _);
        free(list as _);
        best
    }
}

/// C `vendor/tmux/window.c:2027`: `void window_pane_stack_push(struct window_panes *stack, struct window_pane *wp)`
pub unsafe fn window_pane_stack_push(stack: *mut window_panes, wp: *mut window_pane) {
    unsafe {
        if !wp.is_null() {
            window_pane_stack_remove(stack, wp);
            tailq_insert_head::<_, discr_sentry>(stack, wp);
            (*wp).flags |= window_pane_flags::PANE_VISITED;
        }
    }
}

/// C `vendor/tmux/window.c:2038`: `void window_pane_stack_remove(struct window_panes *stack, struct window_pane *wp)`
pub unsafe fn window_pane_stack_remove(stack: *mut window_panes, wp: *mut window_pane) {
    unsafe {
        if !wp.is_null() && (*wp).flags.intersects(window_pane_flags::PANE_VISITED) {
            tailq_remove::<_, crate::discr_sentry>(stack, wp);
            (*wp).flags &= !window_pane_flags::PANE_VISITED;
        }
    }
}

/// Clear alert flags for a winlink
/// C `vendor/tmux/window.c:2048`: `void winlink_clear_flags(struct winlink *wl)`
pub unsafe fn winlink_clear_flags(wl: *mut winlink) {
    unsafe {
        (*(*wl).window).flags &= !WINDOW_ALERTFLAGS;
        for loop_ in tailq_foreach::<_, crate::discr_wentry>(&raw mut (*(*wl).window).winlinks)
            .map(NonNull::as_ptr)
        {
            if (*loop_).flags.intersects(WINLINK_ALERTFLAGS) {
                (*loop_).flags &= !WINLINK_ALERTFLAGS;
                server_status_session((*loop_).session);
            }
        }
    }
}

/// Shuffle window indexes up.
/// C `vendor/tmux/window.c:2063`: `int winlink_shuffle_up(struct session *s, struct winlink *wl, int before)`
pub unsafe fn winlink_shuffle_up(s: *mut session, mut wl: *mut winlink, before: bool) -> i32 {
    if wl.is_null() {
        return -1;
    }
    unsafe {
        let idx = if before { (*wl).idx } else { (*wl).idx + 1 };

        // Find the next free index.
        let mut last = idx;
        for i in idx..i32::MAX {
            last = i;
            if winlink_find_by_index(&raw mut (*s).windows, last).is_null() {
                break;
            }
        }
        if last == i32::MAX {
            return -1;
        }

        // Move everything from last - 1 to idx up a bit.
        while last > idx {
            wl = winlink_find_by_index(&raw mut (*s).windows, last - 1);
            rb_remove(&raw mut (*s).windows, wl);
            (*wl).idx += 1;
            rb_insert(&raw mut (*s).windows, wl);
            last -= 1;
        }

        idx
    }
}

/// C `vendor/tmux/window.c:2094`: `static void window_pane_input_callback(struct client *c, __unused const char *path, int error, int closed, struct evbuffer *buffer, void *data)`
unsafe fn window_pane_input_callback(
    c: *mut client,
    _path: *mut u8,
    error: i32,
    closed: i32,
    buffer: *mut evbuffer,
    data: *mut c_void,
) {
    unsafe {
        let cdata: *mut window_pane_input_data = data as *mut window_pane_input_data;
        let buf: *mut c_uchar = EVBUFFER_DATA(buffer);
        let len: usize = EVBUFFER_LENGTH(buffer);

        let wp = window_pane_find_by_id((*cdata).wp);
        if !(*cdata).file.is_null() && (wp.is_null() || (*c).flags.intersects(client_flag::DEAD)) {
            if wp.is_null() {
                (*c).retval = 1;
                (*c).flags |= client_flag::EXIT;
            }
            file_cancel((*cdata).file);
        } else if (*cdata).file.is_null() || closed != 0 || error != 0 {
            cmdq_continue((*cdata).item);
            server_client_unref(c);
            free(cdata as _);
        } else {
            input_parse_buffer(wp, buf, len);
            evbuffer_drain(buffer, len);
        }
    }
}

/// C `vendor/tmux/window.c:2119`: `int window_pane_start_input(struct window_pane *wp, struct cmdq_item *item, char **cause)`
pub unsafe fn window_pane_start_input(
    wp: *mut window_pane,
    item: *mut cmdq_item,
    cause: *mut *mut u8,
) -> i32 {
    unsafe {
        let c: *mut client = cmdq_get_client(item);

        if !(*wp).flags.intersects(window_pane_flags::PANE_EMPTY) {
            *cause = xstrdup(c!("pane is not empty")).cast().as_ptr();
            return -1;
        }
        if (*c)
            .flags
            .intersects(client_flag::DEAD | client_flag::EXITED)
        {
            return 1;
        }
        if !(*c).session.is_null() {
            return 1;
        }

        let cdata = Box::leak(Box::new(window_pane_input_data {
            item,
            wp: (*wp).id,
            file: null_mut(),
        })) as *mut window_pane_input_data;
        (*cdata).file = file_read(c, c!("-"), Some(window_pane_input_callback), cdata as _);
        (*c).references += 1;

        0
    }
}

/// C `vendor/tmux/window.c:2144`: `void *window_pane_get_new_data(struct window_pane *wp, struct window_pane_offset *wpo, size_t *size)`
pub unsafe fn window_pane_get_new_data(
    wp: *mut window_pane,
    wpo: *mut window_pane_offset,
    size: *mut usize,
) -> *mut c_void {
    unsafe {
        let used = (*wpo).used - (*wp).base_offset;

        *size = EVBUFFER_LENGTH((*(*wp).event).input) - used;
        EVBUFFER_DATA((*(*wp).event).input).add(used) as _
    }
}

/// C `vendor/tmux/window.c:2154`: `void window_pane_update_used_data(struct window_pane *wp, struct window_pane_offset *wpo, size_t size)`
pub unsafe fn window_pane_update_used_data(
    wp: *mut window_pane,
    wpo: *mut window_pane_offset,
    mut size: usize,
) {
    unsafe {
        let used = (*wpo).used - (*wp).base_offset;

        if size > EVBUFFER_LENGTH((*(*wp).event).input) - used {
            size = EVBUFFER_LENGTH((*(*wp).event).input) - used;
        }
        (*wpo).used += size;
    }
}

/// C `vendor/tmux/window.c:2165`: `void window_set_fill_character(struct window *w)`
pub unsafe fn window_set_fill_character(w: NonNull<window>) {
    let w = w.as_ptr();
    unsafe {
        free((*w).fill_character as _);
        (*w).fill_character = null_mut();

        let value = options_get_string_((*w).options, "fill-character");
        if *value != b'\0' && utf8_isvalid(value) {
            let ud = utf8_fromcstr(value);
            if !ud.is_null() && (*ud).width == 1 {
                (*w).fill_character = ud;
            }
        }
    }
}

/// C `vendor/tmux/window.c:2184`: `void window_pane_default_cursor(struct window_pane *wp)`
pub unsafe fn window_pane_default_cursor(wp: *mut window_pane) {
    unsafe {
        let s = (*wp).screen;

        // cursor-colour is a STRING/IS_COLOUR option in next-3.7: expand and
        // parse via style_apply, then take the resolved fg (screen.c
        // screen_set_default_cursor).
        let mut cgc: grid_cell = zeroed();
        style_apply(&raw mut cgc, (*wp).options, c!("cursor-colour"), null_mut());
        (*s).default_ccolour = cgc.fg;

        let c: i32 = options_get_number___::<i32>(&*(*wp).options, "cursor-style");
        (*s).default_mode = mode_flag::empty();
        screen_set_cursor_style(
            c as u32,
            &raw mut (*s).default_cstyle,
            &raw mut (*s).default_mode,
        );
    }
}

/// C `vendor/tmux/window.c:2190`: `int window_pane_mode(struct window_pane *wp)`
pub unsafe fn window_pane_mode(wp: *mut window_pane) -> i32 {
    unsafe {
        if !tailq_first(&raw mut (*wp).modes).is_null() {
            if (*tailq_first(&raw mut (*wp).modes)).mode.addr()
                == (&raw const WINDOW_COPY_MODE).addr()
            {
                return WINDOW_PANE_COPY_MODE;
            }
            if (*tailq_first(&raw mut (*wp).modes)).mode.addr()
                == (&raw const WINDOW_VIEW_MODE).addr()
            {
                return WINDOW_PANE_VIEW_MODE;
            }
        }
        WINDOW_PANE_NO_MODE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The RB-tree comparators that order the windows / winlinks / panes trees
    // are total orders keyed on a single integer field. These are plain C-style
    // structs with no Drop impl, so a zeroed instance with one field set is a
    // safe way to exercise the ordering.

    // window_cmp orders by window id (vendor/tmux/window.c).
    #[test]
    fn test_window_cmp_by_id() {
        unsafe {
            let mut a: window = std::mem::zeroed();
            let mut b: window = std::mem::zeroed();
            a.id = 1;
            b.id = 2;
            assert_eq!(window_cmp(&a, &b), cmp::Ordering::Less);
            assert_eq!(window_cmp(&b, &a), cmp::Ordering::Greater);
            b.id = 1;
            assert_eq!(window_cmp(&a, &b), cmp::Ordering::Equal);
        }
    }

    // winlink_cmp orders by index, which can be negative.
    #[test]
    fn test_winlink_cmp_by_idx() {
        unsafe {
            let mut a: winlink = std::mem::zeroed();
            let mut b: winlink = std::mem::zeroed();
            a.idx = -1;
            b.idx = 0;
            assert_eq!(winlink_cmp(&a, &b), cmp::Ordering::Less);
            a.idx = 10;
            assert_eq!(winlink_cmp(&a, &b), cmp::Ordering::Greater);
        }
    }

    // window_pane_cmp orders by pane id.
    #[test]
    fn test_window_pane_cmp_by_id() {
        unsafe {
            let mut a: window_pane = std::mem::zeroed();
            let mut b: window_pane = std::mem::zeroed();
            a.id = 5;
            b.id = 9;
            assert_eq!(window_pane_cmp(&a, &b), cmp::Ordering::Less);
            assert_eq!(window_pane_cmp(&b, &a), cmp::Ordering::Greater);
            b.id = 5;
            assert_eq!(window_pane_cmp(&a, &b), cmp::Ordering::Equal);
            // Forget the zeroed panes: window_pane is large and, although it has
            // no Drop impl, this documents that we intentionally don't run any.
            std::mem::forget(a);
            std::mem::forget(b);
        }
    }
}
