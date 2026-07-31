// Copyright (c) 2026 Nicholas Marriott <nicholas.marriott@gmail.com>
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

//! Port of `vendor/tmux/prompt-history.c`: the per-type prompt history lists,
//! their disk file, and the accessors the prompt and `show-prompt-history` read
//! them through.

use std::io::BufRead;
use std::io::Write;

use crate::*;
use crate::options_::*;

/// Prompt history. C `vendor/tmux/prompt-history.c:31`: `static char **prompt_hlist[PROMPT_NTYPES]`
pub static mut PROMPT_HLIST: [*mut *mut u8; PROMPT_NTYPES as usize] =
    [null_mut(); PROMPT_NTYPES as usize];

/// C `vendor/tmux/prompt-history.c:32`: `static u_int prompt_hsize[PROMPT_NTYPES]`
pub static mut PROMPT_HSIZE: [u32; PROMPT_NTYPES as usize] = [0; PROMPT_NTYPES as usize];

/// Find the history file to load/save from/to.
/// C `vendor/tmux/prompt-history.c:36`: `static char *prompt_find_history_file(void)`
unsafe fn prompt_find_history_file() -> Option<String> {
    unsafe {
        let history_file = options_get_string_(GLOBAL_OPTIONS, "history-file");
        if *history_file == b'\0' {
            return None;
        }
        if *history_file == b'/' {
            return Some(
                std::ffi::CStr::from_ptr(history_file.cast())
                    .to_string_lossy()
                    .into_owned(),
            );
        }

        if *history_file != b'~' || *history_file.add(1) != b'/' {
            return None;
        }

        let home = find_home()?;

        let str = format_nul!("{}{}", home.to_str().unwrap(), _s(history_file.add(1)));
        let out = std::ffi::CStr::from_ptr(str.cast())
            .to_string_lossy()
            .into_owned();
        free_(str);
        Some(out)
    }
}

/// Add loaded history item to the appropriate list.
/// C `vendor/tmux/prompt-history.c:57`: `static void prompt_add_typed_history(char *line)`
unsafe fn prompt_add_typed_history(mut line: *mut u8) {
    unsafe {
        let mut type_ = prompt_type::PROMPT_TYPE_INVALID;

        let typestr: *mut u8 = strsep(&raw mut line, c!(":"));
        if !line.is_null() {
            type_ = prompt_type(typestr);
        }
        if type_ == prompt_type::PROMPT_TYPE_INVALID {
            // Invalid types are not expected, but this provides backward
            // compatibility with old history files.
            if !line.is_null() {
                line = line.sub(1);
                *(line) = b':';
            }
            prompt_add_history(typestr, prompt_type::PROMPT_TYPE_COMMAND as u32);
        } else {
            prompt_add_history(line, type_ as u32);
        }
    }
}

/// Load prompt history from file.
/// C `vendor/tmux/prompt-history.c:79`: `void prompt_load_history(void)`
pub fn prompt_load_history() {
    unsafe {
        let Some(history_file) = prompt_find_history_file() else {
            return;
        };

        log_debug!("loading history from {}", &history_file);

        let Ok(file) = std::fs::OpenOptions::new().read(true).open(&history_file) else {
            log_debug!("{}: failed to open file", &history_file);
            return;
        };
        let reader = std::io::BufReader::new(file);

        for line in reader.lines() {
            let Ok(line) = line else { break };
            let mut line_bytes = line.into_bytes();
            line_bytes.push(b'\0');

            prompt_add_typed_history(line_bytes.as_mut_ptr());
        }
    }
}

/// Save prompt history to file.
/// C `vendor/tmux/prompt-history.c:119`: `void prompt_save_history(void)`
pub unsafe fn prompt_save_history() {
    unsafe {
        let Some(history_file) = prompt_find_history_file() else {
            return;
        };

        log_debug!("saving history to {}", &history_file);

        let Ok(mut file) = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&history_file)
        else {
            log_debug!("{}: failed to open file for writing", &history_file);
            return;
        };

        for type_ in 0..PROMPT_NTYPES {
            for i in 0..PROMPT_HSIZE[type_ as usize] {
                _ = writeln!(
                    file,
                    "{}:{}",
                    prompt_type_string(type_),
                    _s(*PROMPT_HLIST[type_ as usize].add(i as usize))
                );
            }
        }
    }
}

/// Get previous line from the history.
/// C `vendor/tmux/prompt-history.c:151`: `const char *prompt_up_history(u_int *idx, u_int type)`
pub unsafe fn prompt_up_history(idx: *mut u32, type_: u32) -> *const u8 {
    unsafe {
        // History runs from 0 to size - 1. Index is from 0 to size. Zero is
        // empty.
        if type_ >= PROMPT_NTYPES {
            return null_mut();
        }
        if PROMPT_HSIZE[type_ as usize] == 0
            || *idx.add(type_ as usize) == PROMPT_HSIZE[type_ as usize]
        {
            return null_mut();
        }
        *idx.add(type_ as usize) += 1;
        *PROMPT_HLIST[type_ as usize]
            .add((PROMPT_HSIZE[type_ as usize] - *idx.add(type_ as usize)) as usize)
    }
}

/// Get next line from the history.
/// C `vendor/tmux/prompt-history.c:168`: `const char *prompt_down_history(u_int *idx, u_int type)`
pub unsafe fn prompt_down_history(idx: *mut u32, type_: u32) -> *const u8 {
    unsafe {
        if type_ >= PROMPT_NTYPES {
            return c!("");
        }
        if PROMPT_HSIZE[type_ as usize] == 0 || *idx.add(type_ as usize) == 0 {
            return c!("");
        }
        *idx.add(type_ as usize) -= 1;
        if *idx.add(type_ as usize) == 0 {
            return c!("");
        }

        *PROMPT_HLIST[type_ as usize]
            .add((PROMPT_HSIZE[type_ as usize] - *idx.add(type_ as usize)) as usize)
    }
}

/// Add line to the history.
/// C `vendor/tmux/prompt-history.c:182`: `void prompt_add_history(const char *line, u_int type)`
pub unsafe fn prompt_add_history(line: *const u8, type_: u32) {
    unsafe {
        let mut new: u32 = 1;
        let newsize: u32;
        let mut freecount: u32;
        let movesize: usize;

        if type_ >= PROMPT_NTYPES {
            return;
        }

        let oldsize = PROMPT_HSIZE[type_ as usize];
        if oldsize > 0
            && libc::strcmp(*PROMPT_HLIST[type_ as usize].add(oldsize as usize - 1), line) == 0
        {
            new = 0;
        }

        let hlimit = options_get_number_(GLOBAL_OPTIONS, "prompt-history-limit") as u32;
        if hlimit > oldsize {
            if new == 0 {
                return;
            }
            newsize = oldsize + new;
        } else {
            newsize = hlimit;
            freecount = oldsize + new - newsize;
            if freecount > oldsize {
                freecount = oldsize;
            }
            if freecount == 0 {
                return;
            }
            for i in 0..freecount {
                free_(*PROMPT_HLIST[type_ as usize].add(i as usize));
            }
            movesize = (oldsize - freecount) as usize * size_of::<*mut u8>();
            if movesize > 0 {
                libc::memmove(
                    PROMPT_HLIST[type_ as usize].cast(),
                    PROMPT_HLIST[type_ as usize].add(freecount as usize).cast(),
                    movesize,
                );
            }
        }

        if newsize == 0 {
            free_(PROMPT_HLIST[type_ as usize]);
            PROMPT_HLIST[type_ as usize] = null_mut();
        } else if newsize != oldsize {
            PROMPT_HLIST[type_ as usize] =
                xreallocarray_(PROMPT_HLIST[type_ as usize], newsize as usize).as_ptr();
        }

        if new == 1 && newsize > 0 {
            *PROMPT_HLIST[type_ as usize].add(newsize as usize - 1) = xstrdup(line).as_ptr();
        }
        PROMPT_HSIZE[type_ as usize] = newsize;
    }
}

/// Get history size.
/// C `vendor/tmux/prompt-history.c:233`: `u_int prompt_history_size(enum prompt_type type)`
pub unsafe fn prompt_history_size(type_: u32) -> u32 {
    unsafe {
        if type_ >= PROMPT_NTYPES {
            return 0;
        }
        PROMPT_HSIZE[type_ as usize]
    }
}

/// Get history entry.
/// C `vendor/tmux/prompt-history.c:242`: `const char *prompt_history_get(enum prompt_type type, u_int idx)`
pub unsafe fn prompt_history_get(type_: u32, idx: u32) -> *const u8 {
    unsafe {
        if type_ >= PROMPT_NTYPES {
            return null();
        }
        if idx >= PROMPT_HSIZE[type_ as usize] {
            return null();
        }
        *PROMPT_HLIST[type_ as usize].add(idx as usize)
    }
}

/// Clear prompt history.
/// C `vendor/tmux/prompt-history.c:253`: `void prompt_history_clear(enum prompt_type type)`
pub unsafe fn prompt_history_clear(type_: u32) {
    unsafe {
        if type_ >= PROMPT_NTYPES {
            return;
        }
        for idx in 0..PROMPT_HSIZE[type_ as usize] {
            free_(*PROMPT_HLIST[type_ as usize].add(idx as usize));
        }
        free_(PROMPT_HLIST[type_ as usize]);
        PROMPT_HLIST[type_ as usize] = null_mut();
        PROMPT_HSIZE[type_ as usize] = 0;
    }
}
