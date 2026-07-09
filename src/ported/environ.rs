// Copyright (c) 2009 Nicholas Marriott <nicholas.marriott@gmail.com>
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

pub type environ = rb_head<environ_entry>;
RB_GENERATE!(environ, environ_entry, entry, discr_entry, environ_cmp);

/// C `vendor/tmux/environ.c:37`: `static int environ_cmp(struct environ_entry *envent1, struct environ_entry *envent2)`
pub fn environ_cmp(envent1: &environ_entry, envent2: &environ_entry) -> std::cmp::Ordering {
    unsafe {
        i32_to_ordering(libc::strcmp(
            transmute_ptr(envent1.name),
            transmute_ptr(envent2.name),
        ))
    }
}

/// C `vendor/tmux/environ.c:44`: `struct environ *environ_create(void)`
pub fn environ_create() -> NonNull<environ> {
    unsafe {
        let env = xcalloc1::<environ>();
        rb_init(env);
        NonNull::new_unchecked(env)
    }
}

/// C `vendor/tmux/environ.c:56`: `void environ_free(struct environ *env)`
pub unsafe fn environ_free(env: *mut environ) {
    unsafe {
        for envent in rb_foreach(env).map(NonNull::as_ptr) {
            rb_remove(env, envent);
            free_(transmute_ptr((*envent).name));
            free_(transmute_ptr((*envent).value));
            free_(envent);
        }
        free_(env);
    }
}

/// C `vendor/tmux/environ.c:73`: `struct environ_entry *environ_first(struct environ *env)`
pub unsafe fn environ_first(env: *mut environ) -> *mut environ_entry {
    unsafe { rb_min(env) }
}

/// C `vendor/tmux/environ.c:79`: `struct environ_entry *environ_next(struct environ_entry *envent)`
pub unsafe fn environ_next(envent: *mut environ_entry) -> *mut environ_entry {
    unsafe { rb_next(envent) }
}

/// C `vendor/tmux/environ.c:86`: `void environ_copy(struct environ *srcenv, struct environ *dstenv)`
pub unsafe fn environ_copy(srcenv: *mut environ, dstenv: *mut environ) {
    unsafe {
        for envent in rb_foreach(srcenv).map(NonNull::as_ptr) {
            if let Some(value) = (*envent).value {
                environ_set!(
                    dstenv,
                    (*envent).name.unwrap().as_ptr(),
                    (*envent).flags,
                    "{}",
                    _s(value.as_ptr()),
                );
            } else {
                environ_clear(dstenv, transmute_ptr((*envent).name));
            }
        }
    }
}

/// C `vendor/tmux/environ.c:102`: `struct environ_entry *environ_find(struct environ *env, const char *name)`
pub unsafe fn environ_find(env: *mut environ, name: *const u8) -> *mut environ_entry {
    let mut envent: MaybeUninit<environ_entry> = MaybeUninit::uninit();
    let envent = envent.as_mut_ptr();

    unsafe {
        (*envent).name = NonNull::new(name.cast_mut());
        // std::ptr::write(&raw mut (*envent).name, name);
    }

    unsafe { rb_find(env, envent) }
}

macro_rules! environ_set {
   ($env:expr, $name:expr, $flags:expr, $fmt:literal $(, $args:expr)* $(,)?) => {
        crate::environ_::environ_set_($env, $name, $flags, format_args!($fmt $(, $args)*))
    };
}
pub(crate) use environ_set;
pub unsafe fn environ_set_(
    env: *mut environ,
    name: *const u8,
    flags: environ_flags,
    args: std::fmt::Arguments,
) {
    unsafe {
        let mut envent = environ_find(env, name);
        let mut s = args.to_string();
        s.push('\0');
        let s = NonNull::new(s.leak().as_mut_ptr().cast());

        if !envent.is_null() {
            (*envent).flags = flags;
            free_(transmute_ptr((*envent).value));
            (*envent).value = s;
        } else {
            envent = Box::leak(Box::new(environ_entry {
                name : Some(xstrdup(name).cast()),
                value: s,
                flags,
                entry: rb_entry::default(),
            }));
            rb_insert(env, envent);
        }
    }
}

/// C `vendor/tmux/environ.c:135`: `void environ_clear(struct environ *env, const char *name)`
pub unsafe fn environ_clear(env: *mut environ, name: *const u8) {
    unsafe {
        let mut envent = environ_find(env, name);
        if !envent.is_null() {
            free_(transmute_ptr((*envent).value));
            (*envent).value = None;
        } else {
            envent = Box::leak(Box::new(environ_entry {
                name : Some(xstrdup(name).cast()),
                value: None,
                flags: environ_flags::empty(),
                entry: rb_entry::default(),
            }));
            rb_insert(env, envent);
        }
    }
}

/// C `vendor/tmux/environ.c:153`: `void environ_put(struct environ *env, const char *var, int flags)`
pub unsafe fn environ_put(env: *mut environ, var: *const u8, flags: environ_flags) {
    unsafe {
        let mut value = libc::strchr(var, b'=' as c_int);
        if value.is_null() {
            return;
        }
        value = value.add(1);

        let name: *mut u8 = xstrdup(var).cast().as_ptr();
        *name.add(libc::strcspn(name, c!("="))) = b'\0';

        environ_set!(env, name, flags, "{}", _s(value));
        free_(name);
    }
}

/// C `vendor/tmux/environ.c:172`: `void environ_unset(struct environ *env, const char *name)`
pub unsafe fn environ_unset(env: *mut environ, name: *const u8) {
    unsafe {
        let envent = environ_find(env, name);
        if envent.is_null() {
            return;
        }
        rb_remove(env, envent);
        free_(transmute_ptr((*envent).name));
        free_(transmute_ptr((*envent).value));
        free_(envent);
    }
}

/// C `vendor/tmux/environ.c:186`: `void environ_update(struct options *oo, struct environ *src, struct environ *dst)`
pub unsafe fn environ_update(oo: *mut options, src: *mut environ, dst: *mut environ) {
    unsafe {
        let mut found;

        let o = options_get(&mut *oo, "update-environment");
        if o.is_null() {
            return;
        }
        let mut a = options_array_first(o);
        while !a.is_null() {
            let ov = options_array_item_value(a);
            found = false;
            for envent in rb_foreach(src).map(NonNull::as_ptr) {
                if libc::fnmatch((*ov).string, transmute_ptr((*envent).name), 0) == 0 {
                    environ_set!(
                        dst,
                        transmute_ptr((*envent).name),
                        environ_flags::empty(),
                        "{}",
                        _s(transmute_ptr((*envent).value)),
                    );
                    found = true;
                }
            }
            if !found {
                environ_clear(dst, (*ov).string);
            }
            a = options_array_next(a);
        }
    }
}

/// C `vendor/tmux/environ.c:216`: `void environ_push(struct environ *env)`
pub unsafe fn environ_push(env: *mut environ) {
    unsafe {
        environ = xcalloc_::<*mut u8>(1).as_ptr();
        for envent in rb_foreach(env).map(NonNull::as_ptr) {
            if (*envent).value.is_some()
                && *(*envent).name.unwrap().as_ptr() != b'\0'
                && !(*envent).flags.intersects(ENVIRON_HIDDEN)
            {
                // Called only in the forked child before exec (job.rs, spawn.rs).
                // Use libc setenv (as upstream tmux does) rather than
                // std::env::set_var, which takes std's ENV_LOCK - a lock that is
                // not reset across fork() and would deadlock/abort the child.
                ::libc::setenv(
                    (*envent).name.unwrap().as_ptr().cast(),
                    (*envent).value.unwrap().as_ptr().cast(),
                    1,
                );
            }
        }
    }
}

macro_rules! environ_log {
   ($env:expr, $fmt:literal $(, $args:expr)* $(,)?) => {
        crate::environ_::environ_log_($env, format_args!($fmt $(, $args)*))
    };
}
pub(crate) use environ_log;

pub unsafe fn environ_log_(env: *mut environ, args: std::fmt::Arguments) {
    unsafe {
        let prefix = args.to_string();

        for envent in rb_foreach(env).map(NonNull::as_ptr) {
            if (*envent).value.is_some() && *(*envent).name.unwrap().as_ptr() != b'\0' {
                log_debug!(
                    "{}{}={}",
                    prefix,
                    _s(transmute_ptr((*envent).name)),
                    _s(transmute_ptr((*envent).value))
                );
            }
        }
    }
}

/// C `vendor/tmux/environ.c:253`: `struct environ *environ_for_session(struct session *s, int no_TERM)`
pub unsafe fn environ_for_session(s: *mut session, no_term: c_int) -> *mut environ {
    let env: *mut environ = environ_create().as_ptr();

    unsafe {
        environ_copy(GLOBAL_ENVIRON, env);
        if !s.is_null() {
            environ_copy((*s).environ, env);
        }

        if no_term == 0 {
            let value = options_get_string_(GLOBAL_OPTIONS, "default-terminal");
            environ_set!(env, c!("TERM"), environ_flags::empty(), "{}", _s(value));
            environ_set!(
                env,
                c!("TERM_PROGRAM"),
                environ_flags::empty(),
                "{}",
                "tmux"
            );
            environ_set!(
                env,
                c!("TERM_PROGRAM_VERSION"),
                environ_flags::empty(),
                "{}",
                getversion()
            );
        }

        #[cfg(feature = "systemd")]
        {
            environ_clear(env, c!("LISTEN_PID"));
            environ_clear(env, c!("LISTEN_FDS"));
            environ_clear(env, c!("LISTEN_FDNAMES"));
        }

        let idx = if !s.is_null() { (*s).id as i32 } else { -1 };

        // Advertise the server to panes via $TMUX, pointing at ztmux's own
        // socket. The wider ecosystem (powerline, tpm, prompts, zpwr scripts)
        // detects a multiplexer by $TMUX being set, so we keep that name and
        // never introduce a $ZTMUX variable. ztmux never adopts this socket for
        // resolution (see tmux.rs), so it can never end up on a foreign tmux's
        // socket even when nested inside real tmux.
        environ_set!(
            env,
            c!("TMUX"),
            environ_flags::empty(),
            "{},{},{}",
            _s(SOCKET_PATH),
            std::process::id(),
            idx,
        );

        env
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Build a throwaway entry keyed by `name` (value unset) so environ_cmp can be
    // exercised directly. Free with `free_entry`.
    unsafe fn make_entry(name: *const u8) -> environ_entry {
        environ_entry {
            name: Some(unsafe { xstrdup(name) }),
            value: None,
            flags: environ_flags::empty(),
            entry: rb_entry::default(),
        }
    }

    unsafe fn free_entry(e: &environ_entry) {
        unsafe {
            free_(transmute_ptr(e.name));
            free_(transmute_ptr(e.value));
        }
    }

    // Return the string value of `name` in `env`, or None if the entry is absent
    // or present-but-cleared (value == NULL).
    unsafe fn value_of(env: *mut environ, name: *const u8) -> Option<String> {
        unsafe {
            let e = environ_find(env, name);
            if e.is_null() || (*e).value.is_none() {
                None
            } else {
                Some(cstr_to_str(transmute_ptr((*e).value)).to_string())
            }
        }
    }

    // Collect the names of every entry in tree order (environ_first + environ_next).
    unsafe fn names_in_order(env: *mut environ) -> Vec<String> {
        unsafe {
            let mut out = Vec::new();
            let mut e = environ_first(env);
            while !e.is_null() {
                out.push(cstr_to_str(transmute_ptr((*e).name)).to_string());
                e = environ_next(e);
            }
            out
        }
    }

    #[test]
    fn set_then_find_returns_value() {
        unsafe {
            let env = environ_create().as_ptr();
            assert!(environ_find(env, crate::c!("FOO")).is_null());

            environ_set!(env, crate::c!("FOO"), environ_flags::empty(), "{}", "bar");

            let e = environ_find(env, crate::c!("FOO"));
            assert!(!e.is_null());
            assert_eq!(value_of(env, crate::c!("FOO")).as_deref(), Some("bar"));

            environ_free(env);
        }
    }

    #[test]
    fn set_existing_name_overwrites_value() {
        unsafe {
            let env = environ_create().as_ptr();
            environ_set!(env, crate::c!("FOO"), environ_flags::empty(), "{}", "one");
            environ_set!(env, crate::c!("FOO"), environ_flags::empty(), "{}", "two");

            assert_eq!(value_of(env, crate::c!("FOO")).as_deref(), Some("two"));
            // Overwrite must not create a second entry.
            assert_eq!(names_in_order(env), vec!["FOO".to_string()]);

            environ_free(env);
        }
    }

    #[test]
    fn put_parses_name_equals_value() {
        unsafe {
            let env = environ_create().as_ptr();
            environ_put(env, crate::c!("PATH=/usr/bin"), environ_flags::empty());

            assert_eq!(value_of(env, crate::c!("PATH")).as_deref(), Some("/usr/bin"));

            // A string with no '=' is ignored entirely.
            environ_put(env, crate::c!("NOEQUALS"), environ_flags::empty());
            assert!(environ_find(env, crate::c!("NOEQUALS")).is_null());

            environ_free(env);
        }
    }

    #[test]
    fn next_iterates_in_sorted_order() {
        unsafe {
            let env = environ_create().as_ptr();
            // Insert out of order; the tree is keyed by environ_cmp (strcmp).
            environ_set!(env, crate::c!("CHARLIE"), environ_flags::empty(), "{}", "3");
            environ_set!(env, crate::c!("ALPHA"), environ_flags::empty(), "{}", "1");
            environ_set!(env, crate::c!("BRAVO"), environ_flags::empty(), "{}", "2");

            assert_eq!(
                names_in_order(env),
                vec!["ALPHA".to_string(), "BRAVO".to_string(), "CHARLIE".to_string()]
            );

            environ_free(env);
        }
    }

    #[test]
    fn clear_keeps_entry_but_nulls_value() {
        unsafe {
            let env = environ_create().as_ptr();
            environ_set!(env, crate::c!("FOO"), environ_flags::empty(), "{}", "bar");

            environ_clear(env, crate::c!("FOO"));

            // environ_clear keeps the entry present but with a NULL value.
            let e = environ_find(env, crate::c!("FOO"));
            assert!(!e.is_null());
            assert!((*e).value.is_none());

            environ_free(env);
        }
    }

    #[test]
    fn unset_removes_entry() {
        unsafe {
            let env = environ_create().as_ptr();
            environ_set!(env, crate::c!("FOO"), environ_flags::empty(), "{}", "bar");
            assert!(!environ_find(env, crate::c!("FOO")).is_null());

            environ_unset(env, crate::c!("FOO"));

            assert!(environ_find(env, crate::c!("FOO")).is_null());
            assert!(names_in_order(env).is_empty());

            environ_free(env);
        }
    }

    #[test]
    fn copy_duplicates_entries() {
        unsafe {
            let src = environ_create().as_ptr();
            let dst = environ_create().as_ptr();
            environ_set!(src, crate::c!("FOO"), environ_flags::empty(), "{}", "bar");
            environ_set!(src, crate::c!("BAZ"), environ_flags::empty(), "{}", "qux");

            environ_copy(src, dst);

            assert_eq!(value_of(dst, crate::c!("FOO")).as_deref(), Some("bar"));
            assert_eq!(value_of(dst, crate::c!("BAZ")).as_deref(), Some("qux"));

            // The copies are independent allocations: mutating dst leaves src intact.
            environ_set!(dst, crate::c!("FOO"), environ_flags::empty(), "{}", "changed");
            assert_eq!(value_of(src, crate::c!("FOO")).as_deref(), Some("bar"));
            assert_eq!(value_of(dst, crate::c!("FOO")).as_deref(), Some("changed"));

            environ_free(src);
            environ_free(dst);
        }
    }

    #[test]
    fn cmp_orders_by_name() {
        unsafe {
            let a = make_entry(crate::c!("AAA"));
            let b = make_entry(crate::c!("BBB"));
            let a2 = make_entry(crate::c!("AAA"));

            assert_eq!(environ_cmp(&a, &b), std::cmp::Ordering::Less);
            assert_eq!(environ_cmp(&b, &a), std::cmp::Ordering::Greater);
            assert_eq!(environ_cmp(&a, &a2), std::cmp::Ordering::Equal);

            free_entry(&a);
            free_entry(&b);
            free_entry(&a2);
        }
    }

    // environ_copy replicates a cleared (value==NULL) src entry as a cleared
    // dst entry via environ_clear (environ.c:92-95), not as a value.
    #[test]
    fn copy_propagates_cleared_entries() {
        unsafe {
            let src = environ_create().as_ptr();
            let dst = environ_create().as_ptr();
            environ_set!(src, crate::c!("KEEP"), environ_flags::empty(), "{}", "v");
            environ_clear(src, crate::c!("GONE")); // present but value NULL

            environ_copy(src, dst);

            assert_eq!(value_of(dst, crate::c!("KEEP")).as_deref(), Some("v"));
            // GONE exists in dst but with a NULL value.
            let e = environ_find(dst, crate::c!("GONE"));
            assert!(!e.is_null());
            assert!((*e).value.is_none());

            environ_free(src);
            environ_free(dst);
        }
    }

    // environ_put with "NAME=" sets an empty-string value (environ.c:158-160:
    // value is the char after '='), distinct from a cleared entry.
    #[test]
    fn put_empty_value_is_empty_string_not_null() {
        unsafe {
            let env = environ_create().as_ptr();
            environ_put(env, crate::c!("EMPTY="), environ_flags::empty());
            let e = environ_find(env, crate::c!("EMPTY"));
            assert!(!e.is_null());
            // Value present but empty (Some("")), not cleared (None).
            assert_eq!(value_of(env, crate::c!("EMPTY")).as_deref(), Some(""));
            environ_free(env);
        }
    }

    // environ_put splits on the FIRST '=' (environ.c:157-159: strcspn to the
    // first '='), so "K=a=b" -> name "K", value "a=b".
    #[test]
    fn put_splits_on_first_equals_only() {
        unsafe {
            let env = environ_create().as_ptr();
            environ_put(env, crate::c!("K=a=b"), environ_flags::empty());
            assert_eq!(value_of(env, crate::c!("K")).as_deref(), Some("a=b"));
            environ_free(env);
        }
    }

    // environ_clear on an absent name inserts a present entry with a NULL value
    // (environ.c:141-146): it shows in iteration but value_of is None.
    #[test]
    fn clear_absent_inserts_null_entry() {
        unsafe {
            let env = environ_create().as_ptr();
            assert!(environ_find(env, crate::c!("NEW")).is_null());
            environ_clear(env, crate::c!("NEW"));
            let e = environ_find(env, crate::c!("NEW"));
            assert!(!e.is_null());
            assert!((*e).value.is_none());
            assert_eq!(names_in_order(env), vec!["NEW".to_string()]);
            environ_free(env);
        }
    }

    // environ_set records the flags on the entry, and overwriting a name updates
    // its flags too (environ.c:113-116: existing branch assigns envent->flags).
    #[test]
    fn set_records_and_updates_flags() {
        unsafe {
            let env = environ_create().as_ptr();
            environ_set!(env, crate::c!("H"), ENVIRON_HIDDEN, "{}", "1");
            let e = environ_find(env, crate::c!("H"));
            assert!((*e).flags.intersects(ENVIRON_HIDDEN));
            // Overwrite with empty flags clears the flag on the same entry.
            environ_set!(env, crate::c!("H"), environ_flags::empty(), "{}", "2");
            let e = environ_find(env, crate::c!("H"));
            assert!(!(*e).flags.intersects(ENVIRON_HIDDEN));
            assert_eq!(value_of(env, crate::c!("H")).as_deref(), Some("2"));
            environ_free(env);
        }
    }

    // environ_unset on an absent name returns early with no insertion
    // (environ.c:172-176).
    #[test]
    fn unset_absent_is_noop() {
        unsafe {
            let env = environ_create().as_ptr();
            environ_unset(env, crate::c!("NOPE"));
            assert!(names_in_order(env).is_empty());
            environ_free(env);
        }
    }
}
