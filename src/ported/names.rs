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
use crate::event_::{event_add, event_initialized};
use crate::libc::{gettimeofday, memcpy, strchr, strcmp, strcspn, strlen, strncmp};
use crate::*;
use crate::options_::*;

/// C `vendor/tmux/names.c:34`: `static void name_time_callback(__unused int fd, __unused short events, void *arg)`
pub unsafe extern "C-unwind" fn name_time_callback(
    _fd: c_int,
    _events: c_short,
    w: NonNull<window>,
) {
    unsafe {
        log_debug!("@{} timer expired", (*w.as_ptr()).id);
    }
}

/// C `vendor/tmux/names.c:43`: `static int name_time_expired(struct window *w, struct timeval *tv)`
pub unsafe fn name_time_expired(w: *mut window, tv: *mut timeval) -> c_int {
    unsafe {
        let mut offset: MaybeUninit<timeval> = MaybeUninit::<timeval>::uninit();

        timersub(tv, &raw mut (*w).name_time, offset.as_mut_ptr());
        let offset = offset.assume_init_ref();

        if offset.tv_sec != 0 || offset.tv_usec > NAME_INTERVAL {
            0
        } else {
            (NAME_INTERVAL - offset.tv_usec) as c_int
        }
    }
}

/// C `vendor/tmux/names.c:54`: `void check_window_name(struct window *w)`
pub unsafe fn check_window_name(w: *mut window) {
    unsafe {
        let mut tv: timeval = zeroed();
        let mut next: timeval = zeroed();

        if (*w).active.is_null() {
            return;
        }

        if options_get_number_((*w).options, "automatic-rename") == 0 {
            return;
        }

        if !(*(*w).active)
            .flags
            .intersects(window_pane_flags::PANE_CHANGED)
        {
            // log_debug!("@{} pane not changed", (*w).id);
            return;
        }
        log_debug!("@{} pane changed", (*w).id);

        gettimeofday(&raw mut tv, null_mut());
        let left = name_time_expired(w, &raw mut tv);
        if left != 0 {
            if event_initialized(&raw mut (*w).name_event) == 0 {
                evtimer_set(
                    &raw mut (*w).name_event,
                    name_time_callback,
                    NonNull::new_unchecked(w),
                );
            }
            if evtimer_pending(&raw mut (*w).name_event, null_mut()) == 0 {
                log_debug!("@{} timer queued ({})", (*w).id, left);
                timerclear(&raw mut next);
                next.tv_usec = left as libc::suseconds_t;
                event_add(&raw mut (*w).name_event, &raw const next);
            } else {
                log_debug!("@{} timer already queued ({})", (*w).id, left);
            }
            return;
        }
        memcpy(
            &raw mut (*w).name_time as _,
            &raw const tv as _,
            size_of::<timeval>(),
        );
        if event_initialized(&raw mut (*w).name_event) != 0 {
            evtimer_del(&raw mut (*w).name_event);
        }

        (*(*w).active).flags &= !window_pane_flags::PANE_CHANGED;

        let name = format_window_name(w);
        if strcmp(name, (*w).name) != 0 {
            log_debug!("@{} name {} (was {})", (*w).id, _s(name), _s((*w).name));
            window_set_name(w, name);
            server_redraw_window_borders(w);
            server_status_window(w);
        } else {
            log_debug!("@{} not changed (still {})", (*w).id, _s((*w).name));
        }

        free(name as _);
    }
}

/// C `vendor/tmux/names.c:108`: `char *default_window_name(struct window *w)`
pub unsafe fn default_window_name(w: *mut window) -> String {
    unsafe {
        if (*w).active.is_null() {
            return String::new();
        }

        let cmd =
            CString::new(cmd_stringify_argv((*(*w).active).argc, (*(*w).active).argv)).unwrap();
        if !cmd.is_empty() {
            parse_window_name(cmd.as_ptr().cast())
        } else {
            parse_window_name((*(*w).active).shell_ptr())
        }
    }
}

/// C `vendor/tmux/names.c:124`: `static char *format_window_name(struct window *w)`
unsafe fn format_window_name(w: *mut window) -> *const u8 {
    unsafe {
        let ft = format_create(
            null_mut(),
            null_mut(),
            (FORMAT_WINDOW | (*w).id) as i32,
            format_flags::empty(),
        );
        format_defaults_window(ft, w);
        format_defaults_pane(ft, (*w).active);

        let fmt = options_get_string_((*w).options, "automatic-rename-format");
        let name = format_expand(ft, fmt);

        format_free(ft);
        name
    }
}

/// C `vendor/tmux/names.c:142`: `char *parse_window_name(const char *in)`
pub unsafe fn parse_window_name(in_: *const u8) -> String {
    unsafe {
        let sizeof_exec: usize = 6; // sizeof "exec "
        let copy: *mut u8 = xstrdup(in_).cast().as_ptr();
        let mut name = copy;
        if *name == b'"' {
            name = name.wrapping_add(1);
        }
        *name.add(strcspn(name, c!("\""))) = b'\0';

        if strncmp(name, c!("exec "), sizeof_exec - 1) == 0 {
            name = name.wrapping_add(sizeof_exec - 1);
        }

        while *name == b' ' || *name == b'-' {
            name = name.wrapping_add(1);
        }

        let mut ptr = strchr(name, b' ' as _);
        if !ptr.is_null() {
            *ptr = b'\0' as _;
        }

        if *name != b'\0' {
            ptr = name.add(strlen(name) - 1);
            while ptr > name
                && !(*ptr).is_ascii_alphanumeric()
                && !(*ptr).is_ascii_punctuation()
            {
                *ptr = b'\0';
                ptr = ptr.wrapping_sub(1);
            }
        }

        // C names.c:167-168 — `if (*name == '/') name = basename(name);`.
        // Do basename on the raw `char*` in place so a name containing invalid
        // UTF-8 survives, instead of round-tripping through a Rust `&str` (which
        // would panic in cstr_to_str). This mirrors POSIX basename for the inputs
        // parse_window_name feeds it: strip trailing '/', take the last component.
        if *name == b'/' {
            let len = strlen(name);
            let mut end = len;
            while end > 1 && *name.add(end - 1) == b'/' {
                end -= 1;
            }
            *name.add(end) = b'\0';
            let mut p = name;
            let mut last = name;
            while *p != b'\0' {
                if *p == b'/' {
                    last = p.add(1);
                }
                p = p.add(1);
            }
            name = last;
        }

        // C names.c:169 — `name = clean_name(name, 0);`. clean_name (tmux.c:285)
        // runs utf8_isvalid then utf8_stravis on the raw `char*`. A name with a
        // byte that is not valid UTF-8 (e.g. 0xff) fails utf8_isvalid, so
        // clean_name returns NULL, which C maps to "" (names.c:171-172). Passing
        // the raw pointer straight through means invalid-UTF-8 input yields "",
        // never a panic — matching tmux exactly.
        let cleaned = clean_name(name, 0);
        free(copy as _);
        if cleaned.is_null() {
            return String::new();
        }
        let result = cstr_to_str(cleaned).to_string();
        free_(cleaned);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Small helper so each case reads as input -> expected. Lives inside
    // `mod tests` so it is exempt from the anti-drift gate.
    fn pwn(input: *const u8) -> String {
        unsafe { parse_window_name(input) }
    }

    // Plain command with no special handling passes through unchanged.
    // C names.c:142 `parse_window_name`.
    #[test]
    fn test_plain_name() {
        assert_eq!(pwn(crate::c!("vim")), "vim");
    }

    // A leading `"` is skipped (name++), then `strcspn(name, "\"")` truncates
    // at the closing quote (names.c:147-149).
    #[test]
    fn test_quoted_name() {
        assert_eq!(pwn(crate::c!("\"vim\"")), "vim");
    }

    // Everything after the first `"` (past the skipped leading one) is dropped.
    #[test]
    fn test_quote_truncates_middle() {
        // 'v' is not a quote, so no name++; strcspn finds the `"` at index 4
        // and truncates -> "vim ", then the space truncation gives "vim".
        assert_eq!(pwn(crate::c!("vim \"foo\"")), "vim");
    }

    // A leading "exec " (5 chars) is stripped (names.c:151-152).
    #[test]
    fn test_exec_prefix_stripped() {
        assert_eq!(pwn(crate::c!("exec vim")), "vim");
    }

    // Quote handling runs before the exec strip: leading `"` skipped, closing
    // `"` truncates, then "exec " prefix removed.
    #[test]
    fn test_quoted_exec_prefix() {
        assert_eq!(pwn(crate::c!("\"exec vim\"")), "vim");
    }

    // Leading spaces and dashes are skipped (names.c:154-155).
    #[test]
    fn test_leading_spaces_and_dashes() {
        assert_eq!(pwn(crate::c!("  --foo")), "foo");
        assert_eq!(pwn(crate::c!("-bash")), "bash");
    }

    // Truncated at the first literal space via strchr (names.c:156-157).
    #[test]
    fn test_truncate_at_first_space() {
        assert_eq!(pwn(crate::c!("vim file.txt other")), "vim");
    }

    // Quote-truncation happens first, so the inner space still truncates the
    // survivor: `"vim foo"` -> "vim foo" -> "vim".
    #[test]
    fn test_quote_then_space() {
        assert_eq!(pwn(crate::c!("\"vim foo\"")), "vim");
    }

    // Trailing bytes that are neither alnum nor punct (e.g. tab, newline) are
    // trimmed off the end (names.c:159-165). These are not the space byte, so
    // strchr(' ') does not truncate them first.
    #[test]
    fn test_trailing_control_trimmed() {
        assert_eq!(pwn(crate::c!("vim\t")), "vim");
        assert_eq!(pwn(crate::c!("vim\n")), "vim");
        assert_eq!(pwn(crate::c!("vim\r\n")), "vim");
    }

    // Trailing punctuation is kept (ispunct is true), only whitespace/control
    // is stripped.
    #[test]
    fn test_trailing_punct_kept() {
        assert_eq!(pwn(crate::c!("vim!")), "vim!");
        assert_eq!(pwn(crate::c!("a.out")), "a.out");
    }

    // A leading `/` path is basenamed (names.c:167-168).
    #[test]
    fn test_absolute_path_basenamed() {
        assert_eq!(pwn(crate::c!("/usr/bin/vim")), "vim");
        assert_eq!(pwn(crate::c!("/bin/sh")), "sh");
    }

    // Non-absolute paths are NOT basenamed (the `*name == '/'` gate only fires
    // when the string begins with `/`).
    #[test]
    fn test_relative_path_not_basenamed() {
        assert_eq!(pwn(crate::c!("bin/vim")), "bin/vim");
    }

    // Empty input yields empty output: strcspn returns 0, name[0] set to NUL.
    #[test]
    fn test_empty_input() {
        assert_eq!(pwn(crate::c!("")), "");
    }

    // "exec " with nothing after it strips to empty.
    #[test]
    fn test_exec_only() {
        assert_eq!(pwn(crate::c!("exec ")), "");
    }

    // A lone quote is skipped, leaving an empty string.
    #[test]
    fn test_lone_quote() {
        assert_eq!(pwn(crate::c!("\"")), "");
    }

    // Only dashes/spaces skip to empty.
    #[test]
    fn test_only_dashes() {
        assert_eq!(pwn(crate::c!("---")), "");
        assert_eq!(pwn(crate::c!("   ")), "");
    }

    // The trailing-trim loop uses `ptr > name`, so it never removes the very
    // first character even when it is non-alnum/non-punct — a lone tab survives
    // the trim. But the final clean_name(name, 0) step (names.c:169) then rejects
    // it: a control byte fails utf8_isvalid, so the name reduces to "".
    #[test]
    fn test_single_control_char_cleaned_to_empty() {
        assert_eq!(pwn(crate::c!("\t")), "");
    }

    // basename strips trailing slashes: "/usr/bin/" -> "bin". The trailing `/`
    // is punctuation so it is not trimmed before basename runs.
    #[test]
    fn test_absolute_path_trailing_slash() {
        assert_eq!(pwn(crate::c!("/usr/bin/")), "bin");
    }

    // Combined: leading spaces + exec prefix ordering. Spaces/dashes are only
    // skipped AFTER the exec strip, so "  exec  -vim" -> strcspn keeps all,
    // strncmp against a leading "  ex" fails, so "exec " is NOT stripped here;
    // instead leading spaces are skipped to "exec  -vim", then space truncates
    // to "exec".
    #[test]
    fn test_leading_space_before_exec_not_stripped() {
        assert_eq!(pwn(crate::c!("  exec vim")), "exec");
    }

    // --- Known ztmux port divergence (ignored until fixed) -----------------

    // ztmux BUG: parse_window_name omits the final `clean_name(name, 0)` call
    // that tmux makes (vendor/tmux/names.c:169). clean_name runs utf8_isvalid +
    // utf8_stravis (vendor/tmux/tmux.c:285), so a name containing a control byte
    // is reduced to "" in tmux (invalid UTF-8); ztmux returns the raw bytes.
    // Remove #[ignore] once clean_name is ported and called here.
    #[test]
    fn bug_control_char_name_cleaned_to_empty() {
        let input = [b'a', 0x07u8, b'b', 0u8];
        assert_eq!(pwn(input.as_ptr()), "");
    }

    // A doubled leading quote: the first `"` is skipped (name++), then strcspn
    // finds the second `"` at index 0 and truncates to "" (names.c:147-149).
    #[test]
    fn test_double_leading_quote_is_empty() {
        assert_eq!(pwn(crate::c!("\"\"vim")), "");
    }

    // Leading run of interleaved spaces and dashes is fully skipped before the
    // survivor is taken (names.c:154-155).
    #[test]
    fn test_interleaved_leading_space_dash() {
        assert_eq!(pwn(crate::c!(" - - foo")), "foo");
    }

    // An absolute path with a trailing argument: strchr(' ') truncates at the
    // space first (names.c:156-157), then the leading `/` triggers basename.
    #[test]
    fn test_absolute_path_with_argument() {
        assert_eq!(pwn(crate::c!("/usr/bin/vim file.c")), "vim");
    }

    // "exec " strip runs before the `/`-basename check, so an absolute path after
    // exec is still basenamed (names.c:151-152 then 167-168).
    #[test]
    fn test_exec_then_absolute_path() {
        assert_eq!(pwn(crate::c!("exec /bin/bash")), "bash");
    }

    // A leading dot is punctuation, not stripped; the whole dotfile name survives.
    #[test]
    fn test_dotfile_name_kept() {
        assert_eq!(pwn(crate::c!(".vimrc")), ".vimrc");
    }

    // A dash in the MIDDLE of the name is kept: the leading-skip loop stops at the
    // first non-space/non-dash byte ('f'), and '-' is neither trimmed nor a space.
    #[test]
    fn test_internal_dash_kept() {
        assert_eq!(pwn(crate::c!("foo-bar")), "foo-bar");
    }

    // A trailing quote (no leading one, so no name++) is found by strcspn and
    // truncates the name there (names.c:149).
    #[test]
    fn test_trailing_quote_truncates() {
        assert_eq!(pwn(crate::c!("vim\"")), "vim");
    }

    // "exec" without the trailing space does NOT match "exec " (5 bytes incl.
    // space), so nothing is stripped (names.c:151).
    #[test]
    fn test_exec_without_space_not_stripped() {
        assert_eq!(pwn(crate::c!("execvim")), "execvim");
    }

    // A relative path with a trailing slash is not basenamed (only leading `/`
    // triggers it); the trailing `/` is punctuation so it is kept.
    #[test]
    fn test_relative_trailing_slash_kept() {
        assert_eq!(pwn(crate::c!("bin/")), "bin/");
    }

    // tmux's parse_window_name keeps the name as a raw `char *` and only rejects
    // invalid UTF-8 at the final clean_name(name, 0) step (names.c:169 ->
    // utf8_isvalid), returning "" for a name containing 0xff (names.c:171-172).
    // The ztmux port now operates on the raw bytes through the basename/clean
    // step (names.rs) instead of forcing a cstr_to_str &str conversion, so a 0xff
    // mid-name no longer panics — it cleans to "" exactly like tmux. For this
    // input: no leading quote/exec/space/dash and 'b' is alnum (no trailing trim),
    // so the whole "a\xffb" reaches clean_name, utf8_isvalid fails, NULL -> "".
    #[test]
    fn test_middle_invalid_utf8_cleaned_to_empty() {
        let input = [b'a', 0xffu8, b'b', 0u8];
        assert_eq!(pwn(input.as_ptr()), "");
    }

    // Digits and mixed alphanumerics are ordinary name bytes.
    #[test]
    fn test_alnum_name_kept() {
        assert_eq!(pwn(crate::c!("python3")), "python3");
    }

    // Trailing whitespace/control run past the first space: strchr(' ') truncates
    // at the space, then the trailing tab/newline are stripped by the trim loop.
    #[test]
    fn test_trailing_ctrl_after_space_truncated() {
        assert_eq!(pwn(crate::c!("vim\t\n arg")), "vim");
    }
}
