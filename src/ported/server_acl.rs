// Copyright (c) 2021 Holland Schutte, Jayson Morberg
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
use crate::libc::{getpwuid, getuid};
use crate::*;

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Eq, PartialEq)]
    #[derive(Copy, Clone)]
    pub struct server_acl_user_flags: i32 {
        const SERVER_ACL_READONLY = 0x1;
        /// C `vendor/tmux/tmux.h`: the entry names a group id rather than a
        /// user id. Part of the key, so a uid and a gid with the same numeric
        /// value are two distinct entries.
        const SERVER_ACL_IS_GROUP = 0x2;
    }
}

pub struct server_acl_user {
    /// C `vendor/tmux/server-acl.c:34`: `id_t id` — a uid, or a gid when
    /// `SERVER_ACL_IS_GROUP` is set.
    pub uid: uid_t,

    pub flags: server_acl_user_flags,

    pub entry: rb_entry<server_acl_user>,
}

/// C `vendor/tmux/server-acl.c:42`: `static int server_acl_cmp(struct server_acl_entry *entry1, struct server_acl_entry *entry2)`
pub fn server_acl_cmp(user1: &server_acl_user, user2: &server_acl_user) -> cmp::Ordering {
    // Group entries sort after user entries, so the two id spaces never
    // collide (server-acl.c:45-49).
    let g1 = user1.flags.contains(server_acl_user_flags::SERVER_ACL_IS_GROUP);
    let g2 = user2.flags.contains(server_acl_user_flags::SERVER_ACL_IS_GROUP);
    if g1 != g2 {
        return if g1 {
            cmp::Ordering::Greater
        } else {
            cmp::Ordering::Less
        };
    }
    user1.uid.cmp(&user2.uid)
}

pub type server_acl_entries = rb_head<server_acl_user>;
static mut SERVER_ACL_ENTRIES: server_acl_entries = unsafe { zeroed() };

RB_GENERATE!(
    server_acl_entries,
    server_acl_user,
    entry,
    discr_entry,
    server_acl_cmp
);

/// C `vendor/tmux/server-acl.c:112`: `void server_acl_init(void)`
pub unsafe fn server_acl_init() {
    unsafe {
        rb_init(&raw mut SERVER_ACL_ENTRIES);

        if getuid() != 0 {
            server_acl_user_allow(0, server_acl_user_flags::empty());
        }
        server_acl_user_allow(getuid(), server_acl_user_flags::empty());
    }
}

pub unsafe fn server_acl_user_find(
    uid: uid_t,
    flags: server_acl_user_flags,
) -> *mut server_acl_user {
    unsafe {
        // server-acl.c:62-65: only the group bit is part of the key; READONLY
        // is state on the entry, not part of its identity.
        let mut find: server_acl_user = server_acl_user {
            uid,
            flags: flags & server_acl_user_flags::SERVER_ACL_IS_GROUP,
            ..zeroed()
        };

        rb_find::<_, _>(&raw mut SERVER_ACL_ENTRIES, &raw mut find)
    }
}

/// Check the client's uid, then its gid, against the tree.
/// C `vendor/tmux/server-acl.c:71`: `static struct server_acl_entry *server_acl_check(struct client *c)`
unsafe fn server_acl_check(c: *mut client) -> *mut server_acl_user {
    unsafe {
        let uid = proc_get_peer_uid((*c).peer);
        if uid == -1i32 as uid_t {
            return null_mut();
        }

        // A uid entry wins over a gid one (server-acl.c:217-219).
        let entry = server_acl_user_find(uid, server_acl_user_flags::empty());
        if !entry.is_null() {
            return entry;
        }

        let gid = proc_get_peer_gid((*c).peer);
        if gid == -1i32 as gid_t {
            return null_mut();
        }
        server_acl_user_find(gid, server_acl_user_flags::SERVER_ACL_IS_GROUP)
    }
}

/// Re-apply the tree to every connected client.
/// C `vendor/tmux/server-acl.c:93`: `static void server_acl_update(void)`
///
/// This was not ported before. Both `allow_write` and `deny_write` inlined a
/// loop in its place, and the one in `deny_write` cleared `CLIENT_READONLY`
/// where it had to set it — so a client denied write access stayed writable
/// until it reconnected. Neither inline loop dropped a client whose entry had
/// been removed, which is the other half of what this does.
unsafe fn server_acl_update() {
    unsafe {
        for c in tailq_foreach(&raw mut CLIENTS).map(NonNull::as_ptr) {
            let entry = server_acl_check(c);
            if entry.is_null() {
                (*c).exit_message = Some(c"access not allowed".to_owned());
                (*c).flags |= client_flag::EXIT;
            } else if (*entry)
                .flags
                .contains(server_acl_user_flags::SERVER_ACL_READONLY)
            {
                (*c).flags |= client_flag::READONLY;
            } else {
                (*c).flags &= !client_flag::READONLY;
            }
        }
    }
}

/// C `vendor/tmux/server-acl.c:130`: `void server_acl_display(struct cmdq_item *item)`
pub unsafe fn server_acl_display(item: *mut cmdq_item) {
    unsafe {
        for loop_ in rb_foreach(&raw mut SERVER_ACL_ENTRIES).map(NonNull::as_ptr) {
            let (name, type_) = if !(*loop_)
                .flags
                .contains(server_acl_user_flags::SERVER_ACL_IS_GROUP)
            {
                if (*loop_).uid == 0 {
                    continue;
                }
                let pw = getpwuid((*loop_).uid);
                let name: *const u8 = if !pw.is_null() {
                    (*pw).pw_name.cast()
                } else {
                    c!("unknown")
                };
                (name, 'U')
            } else {
                let gr = libc::getgrgid((*loop_).uid);
                let name: *const u8 = if !gr.is_null() {
                    (*gr).gr_name.cast()
                } else {
                    c!("unknown")
                };
                (name, 'G')
            };
            if (*loop_)
                .flags
                .contains(server_acl_user_flags::SERVER_ACL_READONLY)
            {
                cmdq_print!(item, "{} ({},R)", _s(name), type_);
            } else {
                cmdq_print!(item, "{} ({},W)", _s(name), type_);
            }
        }
    }
}

/// C `vendor/tmux/server-acl.c:163`: `void server_acl_allow(id_t id, int flags)`
pub unsafe fn server_acl_user_allow(uid: uid_t, flags: server_acl_user_flags) {
    unsafe {
        let mut user = server_acl_user_find(uid, flags);
        if user.is_null() {
            user = xcalloc1();
            (*user).uid = uid;
            (*user).flags = flags & server_acl_user_flags::SERVER_ACL_IS_GROUP;
            rb_insert(&raw mut SERVER_ACL_ENTRIES, user);
        }
    }
}

/// C `vendor/tmux/server-acl.c:178`: `void server_acl_deny(id_t id, int flags)`
pub unsafe fn server_acl_user_deny(uid: uid_t, flags: server_acl_user_flags) {
    unsafe {
        let user = server_acl_user_find(uid, flags);
        if !user.is_null() {
            rb_remove(&raw mut SERVER_ACL_ENTRIES, user);
            free_(user);
            server_acl_update();
        }
    }
}

/// C `vendor/tmux/server-acl.c:192`: `void server_acl_allow_write(id_t id, int flags)`
pub unsafe fn server_acl_user_allow_write(uid: uid_t, flags: server_acl_user_flags) {
    unsafe {
        let user = server_acl_user_find(uid, flags);
        if user.is_null() {
            return;
        }
        (*user).flags &= !server_acl_user_flags::SERVER_ACL_READONLY;
        server_acl_update();
    }
}

/// C `vendor/tmux/server-acl.c:205`: `void server_acl_deny_write(id_t id, int flags)`
pub unsafe fn server_acl_user_deny_write(uid: uid_t, flags: server_acl_user_flags) {
    unsafe {
        let user = server_acl_user_find(uid, flags);
        if user.is_null() {
            return;
        }
        (*user).flags |= server_acl_user_flags::SERVER_ACL_READONLY;
        server_acl_update();
    }
}

/// C `vendor/tmux/server-acl.c:222`: `int server_acl_join(struct client *c)`
pub unsafe fn server_acl_join(c: *mut client) -> c_int {
    unsafe {
        let entry = server_acl_check(c);
        if entry.is_null() {
            return 0;
        }
        if (*entry)
            .flags
            .contains(server_acl_user_flags::SERVER_ACL_READONLY)
        {
            (*c).flags |= client_flag::READONLY;
        }
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // SERVER_ACL_ENTRIES is a single process-global RB tree (server-acl.c:56).
    // Cargo runs tests on parallel threads that all share it, so every test
    // holds this mutex for its whole body and drains the tree on entry/exit so
    // siblings never observe each other's entries. We also use unique high UIDs
    // that cannot collide with the real getuid()/0 entries.
    static ACL_LOCK: Mutex<()> = Mutex::new(());

    // Remove and free every entry currently in the tree so the global state is
    // clean. Mirrors server_acl_user_deny (server-acl.c:178) for each uid.
    unsafe fn drain_all() {
        unsafe {
            loop {
                let min = rb_min::<server_acl_user, discr_entry>(&raw mut SERVER_ACL_ENTRIES);
                if min.is_null() {
                    break;
                }
                rb_remove(&raw mut SERVER_ACL_ENTRIES, min);
                free_(min);
            }
        }
    }

    struct Guard<'a>(#[expect(dead_code)] std::sync::MutexGuard<'a, ()>);
    impl Drop for Guard<'_> {
        fn drop(&mut self) {
            unsafe { drain_all() };
        }
    }
    fn setup() -> Guard<'static> {
        let g = ACL_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        unsafe {
            rb_init(&raw mut SERVER_ACL_ENTRIES);
            drain_all();
        }
        Guard(g)
    }

    // server_acl_cmp (server-acl.c:41) orders purely by uid. The Rust port has
    // no group flag, so it is a straight uid comparison.
    #[test]
    fn test_server_acl_cmp_orders_by_uid() {
        let a = server_acl_user {
            uid: 5,
            flags: server_acl_user_flags::empty(),
            entry: unsafe { zeroed() },
        };
        let b = server_acl_user {
            uid: 10,
            flags: server_acl_user_flags::empty(),
            entry: unsafe { zeroed() },
        };
        assert_eq!(server_acl_cmp(&a, &b), cmp::Ordering::Less);
        assert_eq!(server_acl_cmp(&b, &a), cmp::Ordering::Greater);
        assert_eq!(server_acl_cmp(&a, &a), cmp::Ordering::Equal);
        // Flags must not affect ordering: only uid matters.
        let c = server_acl_user {
            uid: 5,
            flags: server_acl_user_flags::SERVER_ACL_READONLY,
            entry: unsafe { zeroed() },
        };
        assert_eq!(server_acl_cmp(&a, &c), cmp::Ordering::Equal);
    }

    // server_acl_user_allow (server-acl.c:163) inserts a fresh xcalloc'd entry
    // whose uid it sets; server_acl_user_find (server-acl.c:60/122) then locates
    // it. A freshly-allowed entry has flags 0 (write access).
    #[test]
    fn test_allow_then_find() {
        let _g = setup();
        unsafe {
            let uid: uid_t = 900_001;
            assert!(server_acl_user_find(uid, server_acl_user_flags::empty()).is_null());

            server_acl_user_allow(uid, server_acl_user_flags::empty());
            let user = server_acl_user_find(uid, server_acl_user_flags::empty());
            assert!(!user.is_null());
            assert_eq!((*user).uid, uid);
            // xcalloc zeroes flags -> not read-only (write access).
            assert!(
                !(*user)
                    .flags
                    .contains(server_acl_user_flags::SERVER_ACL_READONLY)
            );
        }
    }

    // Allowing the same uid twice must not create a second entry
    // (server-acl.c:167 guards on the existing find).
    #[test]
    fn test_allow_is_idempotent() {
        let _g = setup();
        unsafe {
            let uid: uid_t = 900_002;
            server_acl_user_allow(uid, server_acl_user_flags::empty());
            let first = server_acl_user_find(uid, server_acl_user_flags::empty());
            server_acl_user_allow(uid, server_acl_user_flags::empty());
            let second = server_acl_user_find(uid, server_acl_user_flags::empty());
            assert!(!first.is_null());
            // Same allocation, i.e. only one entry exists.
            assert_eq!(first, second);
        }
    }

    // server_acl_user_deny (server-acl.c:178) removes the entry; find then
    // returns NULL. Denying an unknown uid is a no-op.
    #[test]
    fn test_deny_removes_and_unknown_is_noop() {
        let _g = setup();
        unsafe {
            let uid: uid_t = 900_003;
            server_acl_user_allow(uid, server_acl_user_flags::empty());
            assert!(!server_acl_user_find(uid, server_acl_user_flags::empty()).is_null());

            server_acl_user_deny(uid, server_acl_user_flags::empty());
            assert!(server_acl_user_find(uid, server_acl_user_flags::empty()).is_null());

            // Denying again / an unknown uid must not crash or resurrect it.
            server_acl_user_deny(uid, server_acl_user_flags::empty());
            server_acl_user_deny(900_999, server_acl_user_flags::empty());
            assert!(server_acl_user_find(uid, server_acl_user_flags::empty()).is_null());
        }
    }

    // Unknown uid -> NULL (server-acl.c:67 RB_FIND miss).
    #[test]
    fn test_find_unknown_returns_null() {
        let _g = setup();
        unsafe {
            assert!(server_acl_user_find(900_004, server_acl_user_flags::empty()).is_null());
        }
    }

    // server_acl_user_deny_write (server-acl.c:205) sets SERVER_ACL_READONLY;
    // server_acl_user_allow_write (server-acl.c:192) clears it. CLIENTS is empty
    // in unit tests so the client-flag loop is a no-op and only the entry flag
    // is observed here.
    #[test]
    fn test_write_flag_toggling() {
        let _g = setup();
        unsafe {
            let uid: uid_t = 900_005;
            server_acl_user_allow(uid, server_acl_user_flags::empty());
            let user = server_acl_user_find(uid, server_acl_user_flags::empty());
            assert!(!user.is_null());

            // Fresh entry has write access.
            assert!(
                !(*user)
                    .flags
                    .contains(server_acl_user_flags::SERVER_ACL_READONLY)
            );

            server_acl_user_deny_write(uid, server_acl_user_flags::empty());
            assert!(
                (*server_acl_user_find(uid, server_acl_user_flags::empty()))
                    .flags
                    .contains(server_acl_user_flags::SERVER_ACL_READONLY)
            );

            server_acl_user_allow_write(uid, server_acl_user_flags::empty());
            assert!(
                !(*server_acl_user_find(uid, server_acl_user_flags::empty()))
                    .flags
                    .contains(server_acl_user_flags::SERVER_ACL_READONLY)
            );

            // Toggling again is stable.
            server_acl_user_deny_write(uid, server_acl_user_flags::empty());
            server_acl_user_deny_write(uid, server_acl_user_flags::empty());
            assert!(
                (*server_acl_user_find(uid, server_acl_user_flags::empty()))
                    .flags
                    .contains(server_acl_user_flags::SERVER_ACL_READONLY)
            );
        }
    }

    // allow_write / deny_write on an unknown uid return early (server-acl.c:197,
    // 210) without inserting anything.
    #[test]
    fn test_write_flag_unknown_uid_is_noop() {
        let _g = setup();
        unsafe {
            let uid: uid_t = 900_006;
            server_acl_user_allow_write(uid, server_acl_user_flags::empty());
            assert!(server_acl_user_find(uid, server_acl_user_flags::empty()).is_null());
            server_acl_user_deny_write(uid, server_acl_user_flags::empty());
            assert!(server_acl_user_find(uid, server_acl_user_flags::empty()).is_null());
        }
    }

    // server_acl_init (server-acl.c:112) always allows uid 0 and getuid(): if
    // getuid()==0 the single allow(getuid()) covers it; otherwise both 0 and
    // getuid() are added explicitly. So after init both must be present.
    #[test]
    fn test_init_allows_root_and_current_uid() {
        let _g = setup();
        unsafe {
            server_acl_init();
            assert!(!server_acl_user_find(0, server_acl_user_flags::empty()).is_null());
            let me = crate::libc::getuid();
            assert!(!server_acl_user_find(me, server_acl_user_flags::empty()).is_null());
        }
    }

    // Several allowed uids each resolve independently; denying one leaves the
    // others intact (server-acl.c RB tree keyed by uid, server-acl.c:178 removes
    // only the found entry).
    #[test]
    fn test_multiple_users_independent() {
        let _g = setup();
        unsafe {
            for uid in [900_010u32, 900_011, 900_012] {
                server_acl_user_allow(uid, server_acl_user_flags::empty());
            }
            for uid in [900_010u32, 900_011, 900_012] {
                assert!(!server_acl_user_find(uid, server_acl_user_flags::empty()).is_null());
            }
            server_acl_user_deny(900_011, server_acl_user_flags::empty());
            assert!(server_acl_user_find(900_011, server_acl_user_flags::empty()).is_null());
            // Neighbours survive the removal.
            assert!(!server_acl_user_find(900_010, server_acl_user_flags::empty()).is_null());
            assert!(!server_acl_user_find(900_012, server_acl_user_flags::empty()).is_null());
            assert_eq!(
                (*server_acl_user_find(900_010, server_acl_user_flags::empty())).uid,
                900_010
            );
        }
    }

    // Read-only is a per-entry flag: denying write on one uid must not flip a
    // different uid's flag (server-acl.c:205 mutates only the found entry).
    #[test]
    fn test_write_flag_is_per_user() {
        let _g = setup();
        unsafe {
            server_acl_user_allow(900_020, server_acl_user_flags::empty());
            server_acl_user_allow(900_021, server_acl_user_flags::empty());
            server_acl_user_deny_write(900_020, server_acl_user_flags::empty());
            assert!(
                (*server_acl_user_find(900_020, server_acl_user_flags::empty()))
                    .flags
                    .contains(server_acl_user_flags::SERVER_ACL_READONLY)
            );
            assert!(
                !(*server_acl_user_find(900_021, server_acl_user_flags::empty()))
                    .flags
                    .contains(server_acl_user_flags::SERVER_ACL_READONLY)
            );
        }
    }

    // Denying then re-allowing a uid yields a fresh xcalloc'd entry with write
    // access, dropping any prior read-only flag (server-acl.c:163-168).
    #[test]
    fn test_reallow_resets_flags() {
        let _g = setup();
        unsafe {
            server_acl_user_allow(900_030, server_acl_user_flags::empty());
            server_acl_user_deny_write(900_030, server_acl_user_flags::empty());
            server_acl_user_deny(900_030, server_acl_user_flags::empty());
            server_acl_user_allow(900_030, server_acl_user_flags::empty());
            assert!(
                !(*server_acl_user_find(900_030, server_acl_user_flags::empty()))
                    .flags
                    .contains(server_acl_user_flags::SERVER_ACL_READONLY)
            );
        }
    }

    // A uid and a gid with the same numeric value are distinct entries: the
    // group bit is part of the key (server-acl.c:45-49, :62-65).
    #[test]
    fn test_group_and_user_ids_do_not_collide() {
        let _g = setup();
        unsafe {
            let id = 900_100;
            server_acl_user_allow(id, server_acl_user_flags::empty());

            assert!(!server_acl_user_find(id, server_acl_user_flags::empty()).is_null());
            assert!(
                server_acl_user_find(id, server_acl_user_flags::SERVER_ACL_IS_GROUP).is_null(),
                "a user entry must not answer a group lookup for the same id"
            );

            server_acl_user_allow(id, server_acl_user_flags::SERVER_ACL_IS_GROUP);
            let u = server_acl_user_find(id, server_acl_user_flags::empty());
            let g = server_acl_user_find(id, server_acl_user_flags::SERVER_ACL_IS_GROUP);
            assert!(!u.is_null() && !g.is_null());
            assert_ne!(u, g, "the two entries must be separate nodes");

            // Removing one must leave the other.
            server_acl_user_deny(id, server_acl_user_flags::empty());
            assert!(server_acl_user_find(id, server_acl_user_flags::empty()).is_null());
            assert!(!server_acl_user_find(id, server_acl_user_flags::SERVER_ACL_IS_GROUP).is_null());
        }
    }

    // Group entries sort after user entries whatever their ids (server-acl.c:45).
    #[test]
    fn test_cmp_orders_groups_after_users() {
        let user = server_acl_user {
            uid: 5000,
            flags: server_acl_user_flags::empty(),
            ..unsafe { zeroed() }
        };
        let group_lower = server_acl_user {
            uid: 1,
            flags: server_acl_user_flags::SERVER_ACL_IS_GROUP,
            ..unsafe { zeroed() }
        };
        assert_eq!(server_acl_cmp(&user, &group_lower), cmp::Ordering::Less);
        assert_eq!(server_acl_cmp(&group_lower, &user), cmp::Ordering::Greater);
    }

    // READONLY is state, not identity: an entry stays findable after its write
    // flag is toggled (server-acl.c:64 masks the key to the group bit alone).
    #[test]
    fn test_readonly_is_not_part_of_the_key() {
        let _g = setup();
        unsafe {
            let id = 900_101;
            server_acl_user_allow(id, server_acl_user_flags::empty());
            server_acl_user_deny_write(id, server_acl_user_flags::empty());

            let e = server_acl_user_find(id, server_acl_user_flags::empty());
            assert!(!e.is_null(), "entry must still be found once read-only");
            assert!(
                (*e).flags
                    .contains(server_acl_user_flags::SERVER_ACL_READONLY),
                "deny_write must set READONLY on the entry"
            );

            server_acl_user_allow_write(id, server_acl_user_flags::empty());
            let e = server_acl_user_find(id, server_acl_user_flags::empty());
            assert!(!e.is_null());
            assert!(
                !(*e).flags
                    .contains(server_acl_user_flags::SERVER_ACL_READONLY),
                "allow_write must clear READONLY on the entry"
            );
        }
    }
}
