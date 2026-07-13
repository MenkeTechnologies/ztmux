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
use crate::options_::*;

#[repr(C)]
pub struct paste_buffer {
    pub data: *mut u8,
    pub size: usize,

    pub name: Cow<'static, str>,
    pub created: time_t,
    pub automatic: i32,
    pub order: u32,

    pub name_entry: rb_entry<paste_buffer>,
    pub time_entry: rb_entry<paste_buffer>,
}

static mut PASTE_NEXT_INDEX: u32 = 0;
static mut PASTE_NEXT_ORDER: u32 = 0;
static mut PASTE_NUM_AUTOMATIC: u32 = 0;

type paste_name_tree = rb_head<paste_buffer>;
type paste_time_tree = rb_head<paste_buffer>;

static mut PASTE_BY_NAME: paste_name_tree = rb_initializer();
static mut PASTE_BY_TIME: paste_time_tree = rb_initializer();

RB_GENERATE!(
    paste_name_tree,
    paste_buffer,
    name_entry,
    discr_name_entry,
    paste_cmp_names
);
/// C `vendor/tmux/paste.c:47`: `static int paste_cmp_names(const struct paste_buffer *a, const struct paste_buffer *b)`
fn paste_cmp_names(a: *const paste_buffer, b: *const paste_buffer) -> cmp::Ordering {
    unsafe { (*a).name.cmp(&(*b).name) }
}

RB_GENERATE!(
    paste_time_tree,
    paste_buffer,
    time_entry,
    discr_time_entry,
    paste_cmp_times
);
/// C `vendor/tmux/paste.c:53`: `static int paste_cmp_times(const struct paste_buffer *a, const struct paste_buffer *b)`
fn paste_cmp_times(a: *const paste_buffer, b: *const paste_buffer) -> cmp::Ordering {
    unsafe {
        let x = (*a).order;
        let y = (*b).order;

        // C `vendor/tmux/paste.c`: a->order > b->order → -1, a->order < b->order
        // → 1. Higher `order` (newer) sorts first, so paste_by_time is
        // newest-first (RB_MIN is the most recent buffer, per paste_get_top).
        // `y.cmp(&x)` is that descending order (== the C's inverted comparison).
        u32::cmp(&y, &x)
    }
}

/// C `vendor/tmux/paste.c:64`: `const char *paste_buffer_name(struct paste_buffer *pb)`
pub unsafe fn paste_buffer_name<'a>(pb: NonNull<paste_buffer>) -> &'a str {
    unsafe { &(*pb.as_ptr()).name }
}

/// C `vendor/tmux/paste.c:71`: `u_int paste_buffer_order(struct paste_buffer *pb)`
pub unsafe fn paste_buffer_order(pb: NonNull<paste_buffer>) -> u32 {
    unsafe { (*pb.as_ptr()).order }
}

/// C `vendor/tmux/paste.c:78`: `time_t paste_buffer_created(struct paste_buffer *pb)`
pub unsafe fn paste_buffer_created(pb: NonNull<paste_buffer>) -> time_t {
    unsafe { (*pb.as_ptr()).created }
}

/// C `vendor/tmux/paste.c:85`: `const char *paste_buffer_data(struct paste_buffer *pb, size_t *size)`
pub unsafe fn paste_buffer_data(pb: *mut paste_buffer, size: *mut usize) -> *const u8 {
    unsafe {
        if !size.is_null() {
            *size = (*pb).size;
        }
        (*pb).data
    }
}
// all usages seen pass in a param and don't use null, so we can remove the check
pub unsafe fn paste_buffer_data_(pb: NonNull<paste_buffer>, size: &mut usize) -> *const u8 {
    unsafe {
        *size = (*pb.as_ptr()).size;
        (*pb.as_ptr()).data
    }
}

/// C `vendor/tmux/paste.c:94`: `struct paste_buffer *paste_walk(struct paste_buffer *pb)`
pub unsafe fn paste_walk(pb: *mut paste_buffer) -> *mut paste_buffer {
    unsafe {
        if pb.is_null() {
            return rb_min::<_, discr_time_entry>(&raw mut PASTE_BY_TIME);
        }
        rb_next::<_, discr_time_entry>(pb)
    }
}

/// C `vendor/tmux/paste.c:102`: `int paste_is_empty(void)`
pub unsafe fn paste_is_empty() -> bool {
    unsafe { PASTE_BY_TIME.rbh_root.is_null() }
}

/// C `vendor/tmux/paste.c:109`: `struct paste_buffer *paste_get_top(char **name)`
pub unsafe fn paste_get_top(name: *mut Option<&str>) -> *mut paste_buffer {
    unsafe {
        let mut pb = rb_min::<_, discr_time_entry>(&raw mut PASTE_BY_TIME);
        while !pb.is_null() && (*pb).automatic == 0 {
            pb = rb_next::<_, discr_time_entry>(pb);
        }
        if pb.is_null() {
            return null_mut();
        }
        if !name.is_null() {
            *name = Some(&(*pb).name);
        }

        pb
    }
}

/// C `vendor/tmux/paste.c:125`: `struct paste_buffer *paste_get_name(const char *name)`
pub unsafe fn paste_get_name(name: Option<&str>) -> *mut paste_buffer {
    unsafe {
        let Some(name) = name else {
            return null_mut();
        };
        if name.is_empty() {
            return null_mut();
        }

        // C uses a throwaway stack `struct paste_buffer` as the RB_FIND key. `name` is
        // an owned Cow here, so search by key instead of fabricating one.
        rb_find_by::<_, discr_name_entry, _>(&raw mut PASTE_BY_NAME, |pb| name.cmp(&pb.name))
    }
}

/// C `vendor/tmux/paste.c:138`: `void paste_free(struct paste_buffer *pb)`
pub unsafe fn paste_free(pb: NonNull<paste_buffer>) {
    unsafe {
        let pb = pb.as_ptr();
        notify_paste_buffer(&(*pb).name, true);

        rb_remove::<_, discr_name_entry>(&raw mut PASTE_BY_NAME, pb);
        rb_remove::<_, discr_time_entry>(&raw mut PASTE_BY_TIME, pb);
        if (*pb).automatic != 0 {
            PASTE_NUM_AUTOMATIC -= 1;
        }

        free_((*pb).data);
        // Reclaim the Box allocated in paste_add / paste_set. Dropping it frees the
        // owned name; C freed pb->name by hand, and free_(pb) here skipped Drop
        // entirely, which is why the name had to be cleared by assignment first.
        drop(Box::from_raw(pb));
    }
}

/// C `vendor/tmux/paste.c:157`: `void paste_add(const char *prefix, char *data, size_t size)`
pub unsafe fn paste_add(mut prefix: *const u8, data: *mut u8, size: usize) {
    unsafe {
        if prefix.is_null() {
            prefix = c!("buffer");
        }

        if size == 0 {
            free_(data);
            return;
        }

        let limit = options_get_number_(GLOBAL_OPTIONS, "buffer-limit");
        for pb in rb_foreach_reverse::<_, discr_time_entry>(&raw mut PASTE_BY_TIME) {
            if (PASTE_NUM_AUTOMATIC as i64) < limit {
                break;
            }
            if (*pb.as_ptr()).automatic != 0 {
                paste_free(pb);
            }
        }

        let pb = Box::into_raw(Box::new(paste_buffer {
            data,
            size,
            name: Cow::Borrowed(""),
            created: libc::time(null_mut()),
            automatic: 1,
            order: PASTE_NEXT_ORDER,
            name_entry: zeroed(),
            time_entry: zeroed(),
        }));
        PASTE_NUM_AUTOMATIC += 1;
        PASTE_NEXT_ORDER += 1;

        loop {
            let tmp = PASTE_NEXT_INDEX;
            (*pb).name = Cow::Owned(format!("{}{}", _s(prefix), tmp));
            PASTE_NEXT_INDEX += 1;
            if paste_get_name(Some(&(*pb).name)).is_null() {
                break;
            }
        }
        rb_insert::<_, discr_name_entry>(&raw mut PASTE_BY_NAME, pb);
        rb_insert::<_, discr_time_entry>(&raw mut PASTE_BY_TIME, pb);

        notify_paste_buffer(&(*pb).name, false);
    }
}

/// C `vendor/tmux/paste.c:203`: `int paste_rename(const char *oldname, const char *newname, char **cause)`
pub unsafe fn paste_rename(
    oldname: Option<&str>,
    newname: Option<&str>,
    cause: *mut *mut u8,
) -> i32 {
    unsafe {
        if !cause.is_null() {
            *cause = null_mut();
        }

        if oldname.is_none_or(str::is_empty) {
            if !cause.is_null() {
                *cause = xstrdup_(c"no buffer").as_ptr();
            }
            return -1;
        }
        if newname.is_none_or(str::is_empty) {
            if !cause.is_null() {
                *cause = xstrdup_(c"new name is empty").as_ptr();
            }
            return -1;
        }

        let pb = paste_get_name(oldname);
        if pb.is_null() {
            if !cause.is_null() {
                *cause = format_nul!("no buffer {}", oldname.unwrap());
            }
            return -1;
        }

        if let Some(pb_new) = NonNull::new(paste_get_name(newname)) {
            paste_free(pb_new);
        }

        rb_remove::<_, discr_name_entry>(&raw mut PASTE_BY_NAME, pb);

        (*pb).name = Cow::Owned(newname.unwrap().to_string());

        if (*pb).automatic != 0 {
            PASTE_NUM_AUTOMATIC -= 1;
        }
        (*pb).automatic = 0;

        rb_insert::<_, discr_name_entry>(&raw mut PASTE_BY_NAME, pb);

        notify_paste_buffer(oldname.unwrap(), true);
        notify_paste_buffer(newname.unwrap(), false);
    }
    0
}

/// C `vendor/tmux/paste.c:267`: `int paste_set(char *data, size_t size, const char *name, char **cause)`
pub unsafe fn paste_set(
    data: *mut u8,
    size: usize,
    name: Option<&str>,
    cause: *mut *mut u8,
) -> i32 {
    unsafe {
        if !cause.is_null() {
            *cause = null_mut();
        }

        if size == 0 {
            free_(data);
            return 0;
        }
        let Some(name) = name else {
            paste_add(null_mut(), data, size);
            return 0;
        };

        if name.is_empty() {
            if !cause.is_null() {
                *cause = xstrdup_(c"empty buffer name").as_ptr();
            }
            return -1;
        }

        let pb = Box::into_raw(Box::new(paste_buffer {
            data,
            size,
            name: Cow::Owned(name.to_string()),
            created: libc::time(null_mut()),
            automatic: 0,
            order: PASTE_NEXT_ORDER,
            name_entry: rb_entry::default(),
            time_entry: rb_entry::default(),
        }));
        PASTE_NEXT_ORDER += 1;

        if let Some(old) = NonNull::new(paste_get_name(Some(name))) {
            paste_free(old);
        }

        rb_insert::<_, discr_name_entry>(&raw mut PASTE_BY_NAME, pb);
        rb_insert::<_, discr_time_entry>(&raw mut PASTE_BY_TIME, pb);

        notify_paste_buffer(name, false);
    }
    0
}

/// C `vendor/tmux/paste.c:321`: `void paste_replace(struct paste_buffer *pb, char *data, size_t size)`
pub unsafe fn paste_replace(pb: NonNull<paste_buffer>, data: *mut u8, size: usize) {
    unsafe {
        free_((*pb.as_ptr()).data);
        (*pb.as_ptr()).data = data;
        (*pb.as_ptr()).size = size;

        notify_paste_buffer(&(*pb.as_ptr()).name, false);
    }
}

/// C `vendor/tmux/paste.c:332`: `char *paste_make_sample(struct paste_buffer *pb)`
pub unsafe fn paste_make_sample(pb: *mut paste_buffer) -> String {
    unsafe {
        let width = 200;

        let mut len = (*pb).size;
        if len > width {
            len = width;
        }
        let mut buf: Vec<u8> = Vec::with_capacity(len * (4 + 4));

        utf8_strvis_(
            &mut buf,
            (*pb).data,
            len,
            vis_flags::VIS_OCTAL | vis_flags::VIS_CSTYLE | vis_flags::VIS_TAB | vis_flags::VIS_NL,
        );
        if (*pb).size > width || buf.len() > width {
            buf.extend(b"...");
        }
        String::from_utf8(buf).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;
    use std::sync::Mutex;

    // The paste store is a set of process-global RB trees plus the
    // PASTE_NEXT_INDEX / PASTE_NEXT_ORDER / PASTE_NUM_AUTOMATIC counters. Cargo
    // runs tests in parallel threads that all share this state, so every test
    // holds this mutex for its whole body and starts/ends by draining the trees
    // so siblings never observe each other's buffers.
    static PASTE_LOCK: Mutex<()> = Mutex::new(());

    // paste_add() reads options_get_number(global_options, "buffer-limit")
    // (vendor/tmux/paste.c:170). In a unit-test process tmux's main() never ran,
    // so GLOBAL_OPTIONS is NULL; populate it with the server-scope defaults the
    // same way tmux.rs does at startup so the lookup succeeds.
    unsafe fn ensure_global_options() {
        unsafe {
            if GLOBAL_OPTIONS.is_null() {
                GLOBAL_OPTIONS = options_create(null_mut());
                for oe in &OPTIONS_TABLE {
                    if oe.scope & OPTIONS_TABLE_SERVER != 0 {
                        options_default(GLOBAL_OPTIONS, oe);
                    }
                }
            }
        }
    }

    // Free every buffer currently in the store (paste_free touches no global
    // options and does not call paste_get_name, so it is always safe here).
    unsafe fn drain_all() {
        unsafe {
            loop {
                let pb = paste_walk(null_mut());
                if pb.is_null() {
                    break;
                }
                paste_free(NonNull::new(pb).unwrap());
            }
        }
    }

    // Collect the names visited by paste_walk(), starting from NULL, in order.
    unsafe fn walk_names() -> Vec<String> {
        unsafe {
            let mut out = Vec::new();
            let mut pb = paste_walk(null_mut());
            while !pb.is_null() {
                out.push((*pb).name.to_string());
                pb = paste_walk(pb);
            }
            out
        }
    }

    // A test guard that locks the global mutex and guarantees the store is empty
    // on entry and on (normal) exit.
    struct Guard<'a>(#[expect(dead_code)] std::sync::MutexGuard<'a, ()>);
    impl Drop for Guard<'_> {
        fn drop(&mut self) {
            unsafe { drain_all() };
        }
    }
    fn setup() -> Guard<'static> {
        let g = PASTE_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        unsafe {
            ensure_global_options();
            drain_all();
        }
        Guard(g)
    }

    // paste_set() with an explicit name creates a non-automatic buffer
    // (vendor/tmux/paste.c:267): automatic=0, order taken from paste_next_order,
    // reachable by paste_get_name, and paste_buffer_{name,data,order,created}
    // report the stored fields. paste_free removes it from both trees.
    #[test]
    fn test_paste_set_get_name_and_free() {
        let _g = setup();
        unsafe {
            assert!(paste_is_empty());

            let before = PASTE_NEXT_ORDER;
            let rc = paste_set(xstrdup__("hello"), 5, Some("buf_set1"), null_mut());
            assert_eq!(rc, 0);
            assert!(!paste_is_empty());

            let pb = paste_get_name(Some("buf_set1"));
            assert!(!pb.is_null());
            let nn = NonNull::new(pb).unwrap();

            // paste_buffer_name (paste.c:64) returns the stored name.
            assert_eq!(paste_buffer_name(nn), "buf_set1");
            // paste_buffer_order (paste.c:71): paste_set assigns paste_next_order++.
            assert_eq!(paste_buffer_order(nn), before);
            let next_order = PASTE_NEXT_ORDER;
            assert_eq!(next_order, before + 1);
            // Explicit-name buffers are not automatic (paste.c:303).
            assert_eq!((*pb).automatic, 0);
            // paste_buffer_created (paste.c:78) is set to time(NULL); just a sanity
            // bound rather than an exact value.
            assert!(paste_buffer_created(nn) > 0);

            // paste_buffer_data (paste.c:85) reports data + size via out-param.
            let mut size: usize = 0;
            let data = paste_buffer_data(pb, &mut size);
            assert_eq!(size, 5);
            assert_eq!(std::slice::from_raw_parts(data, 5), b"hello");
            // And the NULL-size overload just returns the pointer.
            assert_eq!(paste_buffer_data(pb, null_mut()), (*pb).data);
            // paste_buffer_data_ is the NonNull variant.
            let mut size2: usize = 0;
            let d2 = paste_buffer_data_(nn, &mut size2);
            assert_eq!(size2, 5);
            assert_eq!(d2, (*pb).data);

            // paste_free (paste.c:138) removes from name+time trees.
            paste_free(nn);
            assert!(paste_get_name(Some("buf_set1")).is_null());
            assert!(paste_is_empty());
        }
    }

    // paste_get_name (paste.c:125) returns NULL for a NULL name or an empty
    // string, and NULL for a name that is not present.
    #[test]
    fn test_paste_get_name_none_empty_and_missing() {
        let _g = setup();
        unsafe {
            assert!(paste_get_name(None).is_null());
            assert!(paste_get_name(Some("")).is_null());
            assert!(paste_get_name(Some("definitely_absent_buffer")).is_null());
        }
    }

    // paste_set with size 0 frees the data and adds nothing (paste.c:275-278).
    #[test]
    fn test_paste_set_size_zero_is_noop() {
        let _g = setup();
        unsafe {
            let rc = paste_set(xstrdup__("x"), 0, Some("buf_zero"), null_mut());
            assert_eq!(rc, 0);
            assert!(paste_is_empty());
            assert!(paste_get_name(Some("buf_zero")).is_null());
        }
    }

    // paste_set with an empty (but non-NULL) name is rejected with -1 and sets
    // *cause = "empty buffer name" (paste.c:284-288).
    #[test]
    fn test_paste_set_empty_name_errors() {
        let _g = setup();
        unsafe {
            let mut cause: *mut u8 = null_mut();
            let rc = paste_set(xstrdup__("x"), 1, Some(""), &mut cause);
            assert_eq!(rc, -1);
            assert!(!cause.is_null());
            assert_eq!(CStr::from_ptr(cause.cast()).to_bytes(), b"empty buffer name");
            free_(cause);
            assert!(paste_is_empty());
        }
    }

    // Re-setting an existing name replaces the old buffer: the old one is freed
    // (paste.c:308-309) and only the new data/order remain under that name.
    #[test]
    fn test_paste_set_replaces_existing_name() {
        let _g = setup();
        unsafe {
            paste_set(xstrdup__("AAAA"), 4, Some("dup"), null_mut());
            let first = paste_get_name(Some("dup"));
            let first_order = paste_buffer_order(NonNull::new(first).unwrap());

            paste_set(xstrdup__("BB"), 2, Some("dup"), null_mut());
            let second = paste_get_name(Some("dup"));
            assert!(!second.is_null());

            // New buffer, later order, new contents.
            let second_order = paste_buffer_order(NonNull::new(second).unwrap());
            assert!(second_order > first_order);
            let mut size: usize = 0;
            let data = paste_buffer_data(second, &mut size);
            assert_eq!(size, 2);
            assert_eq!(std::slice::from_raw_parts(data, 2), b"BB");

            // Exactly one buffer named "dup" remains.
            assert_eq!(walk_names().iter().filter(|n| *n == "dup").count(), 1);
        }
    }

    // paste_add (paste.c:157) creates an automatic buffer named "<prefix><n>",
    // where prefix defaults to "buffer" when NULL, and bumps paste_num_automatic.
    #[test]
    fn test_paste_add_automatic_naming() {
        let _g = setup();
        unsafe {
            let num_before = PASTE_NUM_AUTOMATIC;

            paste_add(c!("myprefix"), xstrdup__("data1"), 5);
            let num_after = PASTE_NUM_AUTOMATIC;
            assert_eq!(num_after, num_before + 1);

            // The single buffer is automatic (paste.c:189) and named "<prefix><n>"
            // where <n> is a global counter, so assert the shape not the number.
            let names = walk_names();
            assert_eq!(names.len(), 1);
            let nm = &names[0];
            assert!(nm.starts_with("myprefix"), "name was {nm}");
            assert!(nm["myprefix".len()..].chars().all(|c| c.is_ascii_digit()));
            let pb = paste_get_name(Some(nm));
            assert!(!pb.is_null());
            assert_eq!((*pb).automatic, 1);

            // A second add with NULL prefix uses the "buffer" default
            // (paste.c:162-163) and yields a distinct buffer/name.
            paste_add(null_mut(), xstrdup__("data2"), 5);
            let names = walk_names();
            assert_eq!(names.len(), 2);
            assert!(
                names.iter().any(|n| n.starts_with("buffer")),
                "names were {names:?}"
            );
            assert_ne!(names[0], names[1]);
        }
    }

    // paste_get_top (paste.c:109) returns the first automatic buffer in walk
    // order. paste_cmp_times sorts newest-first (paste.c:53), so RB_MIN — and
    // therefore the top — is the MOST RECENTLY added automatic buffer, matching
    // vendor C.
    #[test]
    fn test_paste_get_top_returns_newest_automatic() {
        let _g = setup();
        unsafe {
            paste_add(c!("gt"), xstrdup__("11"), 2);
            paste_add(c!("gt"), xstrdup__("22"), 2);
            // Newest-first walk: the most recent buffer heads the list.
            let newest_name = walk_names()[0].clone();

            let mut name: Option<&str> = None;
            let top = paste_get_top(&mut name);
            assert!(!top.is_null());
            // The most recently added automatic buffer is returned.
            assert_eq!(name.unwrap(), newest_name);
            assert_eq!(paste_buffer_order(NonNull::new(top).unwrap()), (*top).order);

            // Its data is the most recent payload ("22"), confirming "newest".
            let mut size: usize = 0;
            let data = paste_buffer_data(top, &mut size);
            assert_eq!(std::slice::from_raw_parts(data, size), b"22");
        }
    }

    // paste_add with size 0 frees the data and adds nothing (paste.c:165-168).
    #[test]
    fn test_paste_add_size_zero_is_noop() {
        let _g = setup();
        unsafe {
            paste_add(c!("pfx"), xstrdup__("x"), 0);
            assert!(paste_is_empty());
        }
    }

    // paste_walk (paste.c:94) iterates the time tree; passing NULL yields
    // RB_MIN and each call returns RB_NEXT. paste_cmp_times (paste.c:53) sorts
    // by `order` newest-first (larger order sorts first), so RB_MIN is the most
    // recently set buffer and the walk yields buffers newest-first.
    #[test]
    fn test_paste_walk_order_by_insertion() {
        let _g = setup();
        unsafe {
            // Empty store: walk starts at NULL and immediately ends.
            assert!(paste_walk(null_mut()).is_null());

            paste_set(xstrdup__("11"), 2, Some("w_first"), null_mut());
            paste_set(xstrdup__("22"), 2, Some("w_second"), null_mut());
            paste_set(xstrdup__("33"), 2, Some("w_third"), null_mut());

            // Newest-first: the most recently set buffer walks first.
            let names = walk_names();
            assert_eq!(names, vec!["w_third", "w_second", "w_first"]);

            // Orders are strictly descending along the walk direction.
            let mut prev: Option<u32> = None;
            let mut pb = paste_walk(null_mut());
            while !pb.is_null() {
                let o = paste_buffer_order(NonNull::new(pb).unwrap());
                if let Some(p) = prev {
                    assert!(o < p);
                }
                prev = Some(o);
                pb = paste_walk(pb);
            }
        }
    }

    // paste_get_top (paste.c:109) returns the first *automatic* buffer in walk
    // order, skipping explicitly-named (non-automatic) ones, and NULL when there
    // are none. It also copies the buffer name into *name.
    #[test]
    fn test_paste_get_top_skips_non_automatic() {
        let _g = setup();
        unsafe {
            // Only explicit (non-automatic) buffers -> no top.
            paste_set(xstrdup__("aa"), 2, Some("named_only"), null_mut());
            assert!(paste_get_top(null_mut()).is_null());

            // Add an automatic buffer: it is now the top.
            paste_add(c!("auto"), xstrdup__("bb"), 2);
            let mut name: Option<&str> = None;
            let top = paste_get_top(&mut name);
            assert!(!top.is_null());
            assert_eq!((*top).automatic, 1);
            assert!(name.unwrap().starts_with("auto"));
            // Passing NULL for name is allowed.
            assert_eq!(paste_get_top(null_mut()), top);
        }
    }

    // paste_is_empty (paste.c:102) reflects the time-tree root.
    #[test]
    fn test_paste_is_empty() {
        let _g = setup();
        unsafe {
            assert!(paste_is_empty());
            paste_set(xstrdup__("z"), 1, Some("emptycheck"), null_mut());
            assert!(!paste_is_empty());
            paste_free(NonNull::new(paste_get_name(Some("emptycheck"))).unwrap());
            assert!(paste_is_empty());
        }
    }

    // paste_rename (paste.c:203) moves a buffer to a new name, marking it
    // non-automatic. The old name disappears and the new one resolves to the
    // same buffer with unchanged data.
    #[test]
    fn test_paste_rename_success() {
        let _g = setup();
        unsafe {
            paste_add(c!("ren"), xstrdup__("payload"), 7);
            // Grab the auto-assigned name of the buffer we just added.
            let mut oldname: Option<&str> = None;
            let pb = paste_get_top(&mut oldname);
            assert!(!pb.is_null());
            let oldname = oldname.unwrap().to_string();
            assert_eq!((*pb).automatic, 1);

            let mut cause: *mut u8 = null_mut();
            let rc = paste_rename(Some(&oldname), Some("renamed_buf"), &mut cause);
            assert_eq!(rc, 0);
            assert!(cause.is_null());

            assert!(paste_get_name(Some(&oldname)).is_null());
            let moved = paste_get_name(Some("renamed_buf"));
            assert_eq!(moved, pb);
            // Renaming clears the automatic flag (paste.c:252).
            assert_eq!((*moved).automatic, 0);
            let mut size: usize = 0;
            let data = paste_buffer_data(moved, &mut size);
            assert_eq!(size, 7);
            assert_eq!(std::slice::from_raw_parts(data, 7), b"payload");
        }
    }

    // paste_rename error paths (paste.c:211-235): empty old name, empty new
    // name, and a non-existent source buffer each return -1 with a *cause.
    #[test]
    fn test_paste_rename_errors() {
        let _g = setup();
        unsafe {
            let mut cause: *mut u8 = null_mut();
            assert_eq!(paste_rename(None, Some("x"), &mut cause), -1);
            assert_eq!(CStr::from_ptr(cause.cast()).to_bytes(), b"no buffer");
            free_(cause);

            let mut cause: *mut u8 = null_mut();
            assert_eq!(paste_rename(Some(""), Some("x"), &mut cause), -1);
            assert_eq!(CStr::from_ptr(cause.cast()).to_bytes(), b"no buffer");
            free_(cause);

            let mut cause: *mut u8 = null_mut();
            assert_eq!(paste_rename(Some("something"), None, &mut cause), -1);
            assert_eq!(CStr::from_ptr(cause.cast()).to_bytes(), b"new name is empty");
            free_(cause);

            let mut cause: *mut u8 = null_mut();
            assert_eq!(paste_rename(Some("something"), Some(""), &mut cause), -1);
            assert_eq!(CStr::from_ptr(cause.cast()).to_bytes(), b"new name is empty");
            free_(cause);

            // Non-existent source: "no buffer <oldname>".
            let mut cause: *mut u8 = null_mut();
            assert_eq!(
                paste_rename(Some("no_such_src"), Some("dst"), &mut cause),
                -1
            );
            assert_eq!(
                CStr::from_ptr(cause.cast()).to_bytes(),
                b"no buffer no_such_src"
            );
            free_(cause);
        }
    }

    // Renaming onto an existing (different) name frees the target buffer first
    // (paste.c:242-243), leaving a single buffer under the destination name.
    #[test]
    fn test_paste_rename_over_existing_target() {
        let _g = setup();
        unsafe {
            paste_set(xstrdup__("SRC"), 3, Some("src_buf"), null_mut());
            paste_set(xstrdup__("DST"), 3, Some("dst_buf"), null_mut());

            let mut cause: *mut u8 = null_mut();
            let rc = paste_rename(Some("src_buf"), Some("dst_buf"), &mut cause);
            assert_eq!(rc, 0);
            assert!(cause.is_null());

            assert!(paste_get_name(Some("src_buf")).is_null());
            let dst = paste_get_name(Some("dst_buf"));
            assert!(!dst.is_null());
            // Content is the source buffer's data (old target was freed).
            let mut size: usize = 0;
            let data = paste_buffer_data(dst, &mut size);
            assert_eq!(std::slice::from_raw_parts(data, size), b"SRC");
            assert_eq!(walk_names().iter().filter(|n| *n == "dst_buf").count(), 1);
        }
    }

    // paste_replace (paste.c:321) swaps the data/size of an existing buffer in
    // place, keeping its name/order/automatic identity.
    #[test]
    fn test_paste_replace() {
        let _g = setup();
        unsafe {
            paste_set(xstrdup__("old"), 3, Some("rep"), null_mut());
            let pb = paste_get_name(Some("rep"));
            let nn = NonNull::new(pb).unwrap();
            let order_before = paste_buffer_order(nn);

            paste_replace(nn, xstrdup__("newdata"), 7);

            let mut size: usize = 0;
            let data = paste_buffer_data(pb, &mut size);
            assert_eq!(size, 7);
            assert_eq!(std::slice::from_raw_parts(data, 7), b"newdata");
            // Identity preserved.
            assert_eq!(paste_buffer_order(nn), order_before);
            assert_eq!(paste_get_name(Some("rep")), pb);
        }
    }

    // paste_make_sample (paste.c:332): a short printable buffer visualises to
    // itself; a buffer longer than the 200-char width is truncated and gets a
    // "..." suffix appended.
    #[test]
    fn test_paste_make_sample() {
        let _g = setup();
        unsafe {
            paste_set(xstrdup__("hello world"), 11, Some("samp"), null_mut());
            let pb = paste_get_name(Some("samp"));
            assert_eq!(paste_make_sample(pb), "hello world");

            let big = "a".repeat(250);
            paste_set(xstrdup__(&big), big.len(), Some("bigsamp"), null_mut());
            let pb2 = paste_get_name(Some("bigsamp"));
            let sample = paste_make_sample(pb2);
            // 200 visualised 'a's plus the appended "...".
            assert_eq!(sample.len(), 203);
            assert!(sample.ends_with("..."));
            assert!(sample[..200].bytes().all(|b| b == b'a'));
        }
    }

    // paste_make_sample width boundary (paste.c:335-337 `if (len > width)` and
    // paste.c:365 `if (pb->size > width || ...)`): exactly 200 bytes fits and
    // gets no ellipsis; 201 bytes trips the `>` and appends "...".
    #[test]
    fn test_paste_make_sample_width_boundary() {
        let _g = setup();
        unsafe {
            let exact = "b".repeat(200);
            paste_set(xstrdup__(&exact), 200, Some("exact200"), null_mut());
            let s = paste_make_sample(paste_get_name(Some("exact200")));
            assert_eq!(s.len(), 200);
            assert!(!s.ends_with("..."));

            let over = "b".repeat(201);
            paste_set(xstrdup__(&over), 201, Some("over201"), null_mut());
            let s2 = paste_make_sample(paste_get_name(Some("over201")));
            // 200 visualised bytes (content capped at width) plus "...".
            assert_eq!(s2.len(), 203);
            assert!(s2.ends_with("..."));
        }
    }

    // paste_set(.., None, ..) delegates to paste_add (paste.c:280-281): the
    // buffer is created automatic, named "buffer<n>", and bumps the automatic
    // counter — none of which the explicit-name path does.
    #[test]
    fn test_paste_set_none_name_delegates_to_add() {
        let _g = setup();
        unsafe {
            let before = PASTE_NUM_AUTOMATIC;
            let rc = paste_set(xstrdup__("auto"), 4, None, null_mut());
            assert_eq!(rc, 0);
            let after = PASTE_NUM_AUTOMATIC;
            assert_eq!(after, before + 1);

            let names = walk_names();
            assert_eq!(names.len(), 1);
            assert!(names[0].starts_with("buffer"), "got {}", names[0]);

            let pb = paste_get_name(Some(&names[0]));
            assert_eq!((*pb).automatic, 1);
        }
    }

    // PASTE_NUM_AUTOMATIC tracks live automatic buffers: paste_add increments it,
    // paste_free of an automatic decrements it (paste.c:141-142), and promoting
    // an automatic to named via paste_rename also decrements it (paste.c:243-244).
    #[test]
    fn test_paste_num_automatic_tracks_add_free_rename() {
        let _g = setup();
        unsafe {
            let base = PASTE_NUM_AUTOMATIC;
            paste_add(null_mut(), xstrdup__("a"), 1);
            paste_add(null_mut(), xstrdup__("b"), 1);
            let n2 = PASTE_NUM_AUTOMATIC;
            assert_eq!(n2, base + 2);

            // Free one automatic -> counter drops by one.
            let top = paste_get_top(null_mut());
            paste_free(NonNull::new(top).unwrap());
            let n1 = PASTE_NUM_AUTOMATIC;
            assert_eq!(n1, base + 1);

            // Rename the remaining automatic -> it becomes named, counter drops.
            let names = walk_names();
            assert_eq!(names.len(), 1);
            let rc = paste_rename(Some(&names[0]), Some("promoted"), null_mut());
            assert_eq!(rc, 0);
            let n0 = PASTE_NUM_AUTOMATIC;
            assert_eq!(n0, base);
            assert_eq!((*paste_get_name(Some("promoted"))).automatic, 0);
        }
    }

    // paste_add evicts the oldest automatic buffers once the count reaches
    // buffer-limit (paste.c:170-178): the reverse-time walk frees automatics
    // until PASTE_NUM_AUTOMATIC < limit, so with limit=3 a fourth add drops the
    // oldest and the store never exceeds the limit.
    #[test]
    fn test_paste_add_evicts_oldest_at_buffer_limit() {
        let _g = setup();
        unsafe {
            let saved_limit = options_get_number_(GLOBAL_OPTIONS, "buffer-limit");
            options_set_number(GLOBAL_OPTIONS, "buffer-limit", 3);

            paste_add(null_mut(), xstrdup__("d0"), 2);
            paste_add(null_mut(), xstrdup__("d1"), 2);
            paste_add(null_mut(), xstrdup__("d2"), 2);
            // Oldest ("d0") is present before the limit is hit.
            let mut sz = 0usize;
            let mut names = walk_names();
            assert_eq!(names.len(), 3);

            // Fourth add: count was == limit, so the eviction loop frees the
            // oldest automatic ("d0") before inserting the new one.
            paste_add(null_mut(), xstrdup__("d3"), 2);
            names = walk_names();
            assert_eq!(names.len(), 3, "store must not exceed buffer-limit");

            // Confirm the surviving payloads are the three most recent (d1,d2,d3),
            // walk order newest-first.
            let top = paste_get_top(null_mut());
            let data = paste_buffer_data(top, &mut sz);
            assert_eq!(std::slice::from_raw_parts(data, sz), b"d3");

            // Restore the global so sibling tests see the default limit.
            options_set_number(GLOBAL_OPTIONS, "buffer-limit", saved_limit);
        }
    }
}
