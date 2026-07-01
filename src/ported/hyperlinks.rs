// Copyright (c) 2021 Will <author@will.party>
// Copyright (c) 2022 Jeff Chiang <pobomp@gmail.com>
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

const MAX_HYPERLINKS: u32 = 5000;

static HYPERLINKS_NEXT_EXTERNAL_ID: AtomicU64 = AtomicU64::new(1);
static GLOBAL_HYPERLINKS_COUNT: AtomicU32 = AtomicU32::new(0);

impl_tailq_entry!(hyperlinks_uri, list_entry, tailq_entry<hyperlinks_uri>);
#[repr(C)]
pub struct hyperlinks_uri {
    pub tree: *mut hyperlinks,

    pub inner: u32,
    pub internal_id: *mut u8,
    pub external_id: *mut u8,
    pub uri: *mut u8,

    // #[entry]
    pub list_entry: tailq_entry<hyperlinks_uri>,

    pub by_inner_entry: rb_entry<hyperlinks_uri>,
    pub by_uri_entry: rb_entry<hyperlinks_uri>,
}

pub type hyperlinks_by_uri_tree = rb_head<hyperlinks_uri>;
pub type hyperlinks_by_inner_tree = rb_head<hyperlinks_uri>;

pub type hyperlinks_list = tailq_head<hyperlinks_uri>;

static mut GLOBAL_HYPERLINKS: hyperlinks_list = TAILQ_HEAD_INITIALIZER!(GLOBAL_HYPERLINKS);

#[repr(C)]
pub struct hyperlinks {
    pub next_inner: u32,
    pub by_inner: hyperlinks_by_inner_tree,
    pub by_uri: hyperlinks_by_uri_tree,
    pub references: u32,
}

/// C `vendor/tmux/hyperlinks.c:76`: `static int hyperlinks_by_uri_cmp(struct hyperlinks_uri *left, struct hyperlinks_uri *right)`
fn hyperlinks_by_uri_cmp(left: &hyperlinks_uri, right: &hyperlinks_uri) -> cmp::Ordering {
    unsafe {
        if *left.internal_id == b'\0' || *right.internal_id == b'\0' {
            if *left.internal_id != b'\0' {
                return cmp::Ordering::Less;
            }
            if *right.internal_id != b'\0' {
                return cmp::Ordering::Greater;
            }
            return left.inner.cmp(&right.inner);
        }

        i32_to_ordering(libc::strcmp(left.internal_id, right.internal_id))
            .then_with(|| i32_to_ordering(crate::libc::strcmp(left.uri, right.uri)))
    }
}

RB_GENERATE!(
    hyperlinks_by_uri_tree,
    hyperlinks_uri,
    by_uri_entry,
    discr_by_uri_entry,
    hyperlinks_by_uri_cmp
);

/// C `vendor/tmux/hyperlinks.c:104`: `static int hyperlinks_by_inner_cmp(struct hyperlinks_uri *left, struct hyperlinks_uri *right)`
fn hyperlinks_by_inner_cmp(left: &hyperlinks_uri, right: &hyperlinks_uri) -> cmp::Ordering {
    left.inner.cmp(&right.inner)
}

RB_GENERATE!(
    hyperlinks_by_inner_tree,
    hyperlinks_uri,
    by_inner_entry,
    discr_by_inner_entry,
    hyperlinks_by_inner_cmp
);

/// C `vendor/tmux/hyperlinks.c:116`: `static void hyperlinks_remove(struct hyperlinks_uri *hlu)`
unsafe fn hyperlinks_remove(hlu: *mut hyperlinks_uri) {
    unsafe {
        let hl = (*hlu).tree;

        tailq_remove::<_, _>(&raw mut GLOBAL_HYPERLINKS, hlu);
        GLOBAL_HYPERLINKS_COUNT.fetch_sub(1, atomic::Ordering::Relaxed);

        rb_remove::<_, discr_by_inner_entry>(&raw mut (*hl).by_inner, hlu);
        rb_remove::<_, discr_by_uri_entry>(&raw mut (*hl).by_uri, hlu);

        free_((*hlu).internal_id);
        free_((*hlu).external_id);
        free_((*hlu).uri);
        free_(hlu);
    }
}

/// C `vendor/tmux/hyperlinks.c:134`: `u_int hyperlinks_put(struct hyperlinks *hl, const char *uri_in, const char *internal_id_in)`
pub unsafe fn hyperlinks_put(
    hl: *mut hyperlinks,
    uri_in: *const u8,
    mut internal_id_in: *const u8,
) -> u32 {
    unsafe {
        let mut uri = null_mut();
        let mut internal_id = null_mut();

        // Anonymous URI are stored with an empty internal ID and the tree
        // comparator will make sure they never match each other (so each
        // anonymous URI is unique).
        if internal_id_in.is_null() {
            internal_id_in = c!("");
        }

        utf8_stravis(
            &raw mut uri,
            uri_in,
            vis_flags::VIS_OCTAL | vis_flags::VIS_CSTYLE,
        );
        utf8_stravis(
            &raw mut internal_id,
            internal_id_in,
            vis_flags::VIS_OCTAL | vis_flags::VIS_CSTYLE,
        );

        if *internal_id_in != b'\0' {
            let mut find = MaybeUninit::<hyperlinks_uri>::uninit();
            let find = find.as_mut_ptr();
            (*find).uri = uri;
            (*find).internal_id = internal_id;

            let hlu = rb_find::<_, discr_by_uri_entry>(&raw mut (*hl).by_uri, find);
            if !hlu.is_null() {
                free_(uri);
                free_(internal_id);
                return (*hlu).inner;
            }
        }

        let id = HYPERLINKS_NEXT_EXTERNAL_ID.fetch_add(1, atomic::Ordering::Relaxed);
        let external_id: *mut u8 = format_nul!("tmux{:X}", id);

        let hlu = xcalloc1::<hyperlinks_uri>() as *mut hyperlinks_uri;
        (*hlu).inner = (*hl).next_inner;
        (*hl).next_inner += 1;
        (*hlu).internal_id = internal_id;
        (*hlu).external_id = external_id;
        (*hlu).uri = uri;
        (*hlu).tree = hl;
        rb_insert::<_, discr_by_uri_entry>(&raw mut (*hl).by_uri, hlu);
        rb_insert::<_, discr_by_inner_entry>(&raw mut (*hl).by_inner, hlu);

        tailq_insert_tail(&raw mut GLOBAL_HYPERLINKS, hlu);
        if GLOBAL_HYPERLINKS_COUNT.fetch_add(1, atomic::Ordering::Relaxed) + 1 == MAX_HYPERLINKS {
            hyperlinks_remove(tailq_first(&raw mut GLOBAL_HYPERLINKS));
        }

        (*hlu).inner
    }
}

/// C `vendor/tmux/hyperlinks.c:186`: `int hyperlinks_get(struct hyperlinks *hl, u_int inner, const char **uri_out, const char **internal_id_out, const char **external_id_out)`
pub unsafe fn hyperlinks_get(
    hl: *mut hyperlinks,
    inner: u32,
    uri_out: *mut *const u8,
    internal_id_out: *mut *const u8,
    external_id_out: *mut *const u8,
) -> bool {
    unsafe {
        let mut find = MaybeUninit::<hyperlinks_uri>::uninit();
        let find = find.as_mut_ptr();
        (*find).inner = inner;

        let hlu = rb_find::<_, discr_by_inner_entry>(&raw mut (*hl).by_inner, find);
        if hlu.is_null() {
            return false;
        }
        if !internal_id_out.is_null() {
            *internal_id_out = (*hlu).internal_id;
        }
        if !external_id_out.is_null() {
            *external_id_out = (*hlu).external_id;
        }
        *uri_out = (*hlu).uri as _;
        true
    }
}

/// C `vendor/tmux/hyperlinks.c:206`: `struct hyperlinks *hyperlinks_init(void)`
pub unsafe fn hyperlinks_init() -> *mut hyperlinks {
    unsafe {
        let hl = xcalloc_::<hyperlinks>(1).as_ptr();
        (*hl).next_inner = 1;
        rb_init(&raw mut (*hl).by_uri);
        rb_init(&raw mut (*hl).by_inner);
        (*hl).references = 1;
        hl
    }
}

/// C `vendor/tmux/hyperlinks.c:220`: `struct hyperlinks *hyperlinks_copy(struct hyperlinks *hl)`
pub unsafe fn hyperlinks_copy(hl: *mut hyperlinks) -> *mut hyperlinks {
    unsafe {
        (*hl).references += 1;
    }
    hl
}

/// C `vendor/tmux/hyperlinks.c:228`: `void hyperlinks_reset(struct hyperlinks *hl)`
pub unsafe fn hyperlinks_reset(hl: *mut hyperlinks) {
    unsafe {
        for hlu in rb_foreach::<_, discr_by_inner_entry>(&raw mut (*hl).by_inner) {
            hyperlinks_remove(hlu.as_ptr());
        }
    }
}

/// C `vendor/tmux/hyperlinks.c:238`: `void hyperlinks_free(struct hyperlinks *hl)`
pub unsafe fn hyperlinks_free(hl: *mut hyperlinks) {
    unsafe {
        (*hl).references -= 1;
        if (*hl).references == 0 {
            hyperlinks_reset(hl);
            free_(hl);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // These functions mutate module-global state (GLOBAL_HYPERLINKS,
    // GLOBAL_HYPERLINKS_COUNT, HYPERLINKS_NEXT_EXTERNAL_ID), so serialize the
    // whole module's tests and clean up (hyperlinks_free) at the end of each.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    // Helper: compare a C string pointer against a Rust &str.
    unsafe fn cstr_eq(p: *const u8, s: &str) -> bool {
        let want = std::ffi::CString::new(s).unwrap();
        unsafe { crate::libc::strcmp(p, want.as_ptr() as *const u8) == 0 }
    }

    #[test]
    fn test_init_defaults() {
        let _g = TEST_LOCK.lock().unwrap();
        unsafe {
            // C hyperlinks_init (hyperlinks.c:206): next_inner = 1, references = 1.
            let hl = hyperlinks_init();
            assert!(!hl.is_null());
            assert_eq!((*hl).next_inner, 1);
            assert_eq!((*hl).references, 1);
            hyperlinks_free(hl);
        }
    }

    #[test]
    fn test_put_get_roundtrip() {
        let _g = TEST_LOCK.lock().unwrap();
        unsafe {
            let hl = hyperlinks_init();

            // First put returns inner == next_inner (1) per hyperlinks.c:169.
            let inner = hyperlinks_put(hl, crate::c!("http://example.com/a"), crate::c!("id1"));
            assert_eq!(inner, 1);
            // next_inner incremented after the put.
            assert_eq!((*hl).next_inner, 2);

            let mut uri: *const u8 = null();
            let mut internal_id: *const u8 = null();
            let mut external_id: *const u8 = null();
            let found = hyperlinks_get(
                hl,
                inner,
                &raw mut uri,
                &raw mut internal_id,
                &raw mut external_id,
            );
            assert!(found);
            // Plain ASCII passes through utf8_stravis unchanged.
            assert!(cstr_eq(uri, "http://example.com/a"));
            assert!(cstr_eq(internal_id, "id1"));
            // external_id is "tmux%llX" (hyperlinks.c:166); at least prefixed "tmux".
            assert_eq!(std::slice::from_raw_parts(external_id, 4), b"tmux");

            hyperlinks_free(hl);
        }
    }

    #[test]
    fn test_same_uri_and_id_dedups() {
        let _g = TEST_LOCK.lock().unwrap();
        unsafe {
            let hl = hyperlinks_init();

            // Same (uri, internal_id) must dedup to the same inner and NOT
            // allocate a new node (hyperlinks.c:155-164).
            let a = hyperlinks_put(hl, crate::c!("http://dup.example/x"), crate::c!("same"));
            let b = hyperlinks_put(hl, crate::c!("http://dup.example/x"), crate::c!("same"));
            assert_eq!(a, b);
            // next_inner advanced only once (dedup path returns early).
            assert_eq!((*hl).next_inner, 2);

            hyperlinks_free(hl);
        }
    }

    #[test]
    fn test_distinct_uris_get_distinct_inners() {
        let _g = TEST_LOCK.lock().unwrap();
        unsafe {
            let hl = hyperlinks_init();

            let a = hyperlinks_put(hl, crate::c!("http://a.example/"), crate::c!("id"));
            let b = hyperlinks_put(hl, crate::c!("http://b.example/"), crate::c!("id"));
            assert_ne!(a, b);
            assert_eq!(a, 1);
            assert_eq!(b, 2);
            assert_eq!((*hl).next_inner, 3);

            // Both retrievable and mapped to their own URIs.
            let mut uri: *const u8 = null();
            assert!(hyperlinks_get(hl, a, &raw mut uri, null_mut(), null_mut()));
            assert!(cstr_eq(uri, "http://a.example/"));
            assert!(hyperlinks_get(hl, b, &raw mut uri, null_mut(), null_mut()));
            assert!(cstr_eq(uri, "http://b.example/"));

            hyperlinks_free(hl);
        }
    }

    #[test]
    fn test_same_uri_different_id_distinct() {
        let _g = TEST_LOCK.lock().unwrap();
        unsafe {
            let hl = hyperlinks_init();

            // by_uri comparator keys on (internal_id, uri) (hyperlinks.c:93-96),
            // so the same URI with different internal IDs is distinct.
            let a = hyperlinks_put(hl, crate::c!("http://same.example/"), crate::c!("id1"));
            let b = hyperlinks_put(hl, crate::c!("http://same.example/"), crate::c!("id2"));
            assert_ne!(a, b);

            hyperlinks_free(hl);
        }
    }

    #[test]
    fn test_anonymous_uris_are_unique() {
        let _g = TEST_LOCK.lock().unwrap();
        unsafe {
            let hl = hyperlinks_init();

            // NULL internal_id => anonymous; comparator makes each unique even
            // with an identical URI (hyperlinks.c:80-91, 145-146).
            let a = hyperlinks_put(hl, crate::c!("http://anon.example/"), null());
            let b = hyperlinks_put(hl, crate::c!("http://anon.example/"), null());
            assert_ne!(a, b);
            assert_eq!((*hl).next_inner, 3);

            // Anonymous URIs get an empty internal ID.
            let mut internal_id: *const u8 = null();
            let mut uri: *const u8 = null();
            assert!(hyperlinks_get(hl, a, &raw mut uri, &raw mut internal_id, null_mut()));
            assert!(cstr_eq(internal_id, ""));

            hyperlinks_free(hl);
        }
    }

    #[test]
    fn test_get_missing_inner_returns_false() {
        let _g = TEST_LOCK.lock().unwrap();
        unsafe {
            let hl = hyperlinks_init();
            let _ = hyperlinks_put(hl, crate::c!("http://x/"), crate::c!("id"));

            let mut uri: *const u8 = null();
            // inner 999 was never stored -> RB_FIND misses -> returns 0/false.
            let found = hyperlinks_get(hl, 999, &raw mut uri, null_mut(), null_mut());
            assert!(!found);

            hyperlinks_free(hl);
        }
    }

    #[test]
    fn test_reset_clears_entries() {
        let _g = TEST_LOCK.lock().unwrap();
        unsafe {
            let hl = hyperlinks_init();
            let inner = hyperlinks_put(hl, crate::c!("http://reset/"), crate::c!("id"));

            let mut uri: *const u8 = null();
            assert!(hyperlinks_get(hl, inner, &raw mut uri, null_mut(), null_mut()));

            // hyperlinks_reset removes all nodes (hyperlinks.c:228-234) but keeps
            // the set alive; the previously stored inner is gone afterwards.
            hyperlinks_reset(hl);
            assert!(!hyperlinks_get(hl, inner, &raw mut uri, null_mut(), null_mut()));

            hyperlinks_free(hl);
        }
    }

    #[test]
    fn test_copy_bumps_references() {
        let _g = TEST_LOCK.lock().unwrap();
        unsafe {
            let hl = hyperlinks_init();
            assert_eq!((*hl).references, 1);

            // hyperlinks_copy just increments references (hyperlinks.c:220-224).
            let hl2 = hyperlinks_copy(hl);
            assert_eq!(hl2, hl);
            assert_eq!((*hl).references, 2);

            // First free decrements to 1 and must NOT free the set.
            hyperlinks_free(hl);
            assert_eq!((*hl).references, 1);
            // Second free drops to 0 and frees.
            hyperlinks_free(hl);
        }
    }

    // C hyperlinks.c:166 external_id = "tmux%llX": a global monotonic counter
    // rendered as UPPERCASE hexadecimal, prefixed "tmux". The counter is process
    // -global (HYPERLINKS_NEXT_EXTERNAL_ID) so we can't pin an exact value, but
    // two distinct puts must yield two distinct, well-formed external IDs.
    #[test]
    fn test_external_id_format_and_uniqueness() {
        let _g = TEST_LOCK.lock().unwrap();
        unsafe {
            let hl = hyperlinks_init();
            let a = hyperlinks_put(hl, crate::c!("http://ext/a"), crate::c!("ida"));
            let b = hyperlinks_put(hl, crate::c!("http://ext/b"), crate::c!("idb"));

            let read_ext = |inner: u32| -> Vec<u8> {
                let mut uri: *const u8 = null();
                let mut ext: *const u8 = null();
                assert!(hyperlinks_get(hl, inner, &raw mut uri, null_mut(), &raw mut ext));
                CStr::from_ptr(ext.cast()).to_bytes().to_vec()
            };
            let ea = read_ext(a);
            let eb = read_ext(b);

            // Prefixed "tmux" and the remainder is uppercase hex.
            for e in [&ea, &eb] {
                assert!(e.starts_with(b"tmux"), "external id must start with tmux");
                for &c in &e[4..] {
                    assert!(
                        c.is_ascii_digit() || (b'A'..=b'F').contains(&c),
                        "external id tail must be uppercase hex, got {}",
                        c as char
                    );
                }
                assert!(e.len() > 4, "external id must have a hex tail");
            }
            // Distinct puts -> distinct external ids (counter advanced).
            assert_ne!(ea, eb);

            hyperlinks_free(hl);
        }
    }

    // C hyperlinks.c:186 hyperlinks_get: the out pointers are all optional except
    // uri_out. Passing NULL for internal_id_out / external_id_out must still
    // populate uri_out and return non-zero (the NULL guards at :192-197).
    #[test]
    fn test_get_with_null_optional_outs() {
        let _g = TEST_LOCK.lock().unwrap();
        unsafe {
            let hl = hyperlinks_init();
            let inner = hyperlinks_put(hl, crate::c!("http://only-uri/"), crate::c!("id"));

            let mut uri: *const u8 = null();
            let found = hyperlinks_get(hl, inner, &raw mut uri, null_mut(), null_mut());
            assert!(found);
            assert!(cstr_eq(uri, "http://only-uri/"));

            hyperlinks_free(hl);
        }
    }

    // C hyperlinks.c:134 the dedup guard is `if (*internal_id_in != '\0')`, so an
    // explicit EMPTY internal id ("") is treated like an anonymous URI: it is
    // NOT deduplicated even for an identical URI, and each gets its own inner.
    #[test]
    fn test_empty_internal_id_is_not_deduped() {
        let _g = TEST_LOCK.lock().unwrap();
        unsafe {
            let hl = hyperlinks_init();
            let a = hyperlinks_put(hl, crate::c!("http://empty-id/"), crate::c!(""));
            let b = hyperlinks_put(hl, crate::c!("http://empty-id/"), crate::c!(""));
            assert_ne!(a, b);
            assert_eq!((*hl).next_inner, 3);

            hyperlinks_free(hl);
        }
    }

    // C hyperlinks.c:228 hyperlinks_reset removes the nodes but does NOT reset
    // hl->next_inner, so inner ids keep climbing after a reset (they are never
    // reused). A fresh put after reset returns the continued counter value.
    #[test]
    fn test_reset_does_not_rewind_next_inner() {
        let _g = TEST_LOCK.lock().unwrap();
        unsafe {
            let hl = hyperlinks_init();
            let a = hyperlinks_put(hl, crate::c!("http://r/a"), crate::c!("a"));
            let b = hyperlinks_put(hl, crate::c!("http://r/b"), crate::c!("b"));
            assert_eq!(a, 1);
            assert_eq!(b, 2);

            hyperlinks_reset(hl);
            // next_inner is untouched by reset; the next put continues at 3.
            let c = hyperlinks_put(hl, crate::c!("http://r/c"), crate::c!("c"));
            assert_eq!(c, 3);
            assert_eq!((*hl).next_inner, 4);
            // And the earlier inners are truly gone.
            let mut uri: *const u8 = null();
            assert!(!hyperlinks_get(hl, a, &raw mut uri, null_mut(), null_mut()));
            assert!(!hyperlinks_get(hl, b, &raw mut uri, null_mut(), null_mut()));
            assert!(hyperlinks_get(hl, c, &raw mut uri, null_mut(), null_mut()));

            hyperlinks_free(hl);
        }
    }

    // Two separate hyperlink sets are independent: an inner stored in one is not
    // visible through the other (each has its own by_inner tree).
    #[test]
    fn test_two_sets_are_independent() {
        let _g = TEST_LOCK.lock().unwrap();
        unsafe {
            let h1 = hyperlinks_init();
            let h2 = hyperlinks_init();
            let i1 = hyperlinks_put(h1, crate::c!("http://one/"), crate::c!("x"));

            // The same inner value does not resolve in the other set.
            let mut uri: *const u8 = null();
            assert!(!hyperlinks_get(h2, i1, &raw mut uri, null_mut(), null_mut()));
            // But it resolves in its own set.
            assert!(hyperlinks_get(h1, i1, &raw mut uri, null_mut(), null_mut()));

            hyperlinks_free(h1);
            hyperlinks_free(h2);
        }
    }

    // C hyperlinks.c:76 the by_uri comparator keys on (internal_id, uri): a put
    // with the same internal_id but a DIFFERENT uri is a distinct entry (no
    // dedup), so its inner id advances. Complements test_same_uri_different_id.
    #[test]
    fn test_same_id_different_uri_distinct() {
        let _g = TEST_LOCK.lock().unwrap();
        unsafe {
            let hl = hyperlinks_init();
            let a = hyperlinks_put(hl, crate::c!("http://a/"), crate::c!("shared"));
            let b = hyperlinks_put(hl, crate::c!("http://b/"), crate::c!("shared"));
            assert_ne!(a, b);
            assert_eq!((*hl).next_inner, 3);

            // Re-putting the FIRST (uri, id) pair now dedups back to `a`.
            let a2 = hyperlinks_put(hl, crate::c!("http://a/"), crate::c!("shared"));
            assert_eq!(a2, a);
            assert_eq!((*hl).next_inner, 3);

            hyperlinks_free(hl);
        }
    }
}
