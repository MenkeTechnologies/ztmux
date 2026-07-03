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

pub static CMD_PASTE_BUFFER_ENTRY: cmd_entry = cmd_entry {
    name: "paste-buffer",
    alias: Some("pasteb"),

    args: args_parse::new("db:prSs:t:", 0, 0, None),
    usage: "[-dprS] [-s separator] [-b buffer-name] [-t target-pane]",

    target: cmd_entry_flag::new(b't', cmd_find_type::CMD_FIND_PANE, cmd_find_flags::empty()),

    flags: cmd_flag::CMD_AFTERHOOK,
    exec: cmd_paste_buffer_exec,
    source: cmd_entry_flag::zeroed(),
};

/// Sanitise a run of buffer bytes before writing it to the pane — safe control
/// characters (`\a`, `\b`, `\r`, `\t`, `\n`) and printable text pass through,
/// other control bytes are vis-escaped.
/// C `vendor/tmux/cmd-paste-buffer.c`: `static void cmd_paste_buffer_paste(struct window_pane *wp, const char *buf, size_t len)`
unsafe fn cmd_paste_buffer_paste(wp: *mut window_pane, buf: *const u8, len: usize) {
    unsafe {
        let mut cp: *mut u8 = null_mut();
        let n = utf8_stravisx(
            &raw mut cp,
            buf,
            len,
            vis_flags::VIS_SAFE | vis_flags::VIS_NOSLASH,
        );
        bufferevent_write((*wp).event, cp.cast(), n as usize);
        free_(cp);
    }
}

/// C `vendor/tmux/cmd-paste-buffer.c:58`: `static enum cmd_retval cmd_paste_buffer_exec(struct cmd *self, struct cmdq_item *item)`
unsafe fn cmd_paste_buffer_exec(self_: *mut cmd, item: *mut cmdq_item) -> cmd_retval {
    unsafe {
        let args = cmd_get_args(self_);
        let target = cmdq_get_target(item);
        let wp = (*target).wp;
        let bracket = args_has(args, 'p');

        if window_pane_exited(wp) {
            cmdq_error!(item, "target pane has exited");
            return cmd_retval::CMD_RETURN_ERROR;
        }

        let mut bufname = None;
        if args_has(args, 'b') {
            bufname = Some(cstr_to_str(args_get(args, b'b')));
        }

        let pb;
        if let Some(bufname) = bufname {
            pb = paste_get_name(Some(bufname));
            if pb.is_null() {
                cmdq_error!(item, "no buffer {bufname}");
                return cmd_retval::CMD_RETURN_ERROR;
            }
        } else {
            pb = paste_get_top(null_mut());
        }

        if let Some(pb) = NonNull::new(pb)
            && !(*wp).flags.intersects(window_pane_flags::PANE_INPUTOFF)
        {
            let mut sepstr = args_get(args, b's');
            if sepstr.is_null() {
                if args_has(args, 'r') {
                    sepstr = c!("\n");
                } else {
                    sepstr = c!("\r");
                }
            }
            let seplen = strlen(sepstr);

            if bracket
                && (*(*wp).screen)
                    .mode
                    .intersects(mode_flag::MODE_BRACKETPASTE)
            {
                bufferevent_write((*wp).event, c!("\x1b[200~").cast(), 6);
            }

            let mut bufsize: usize = 0;
            let mut bufdata = paste_buffer_data_(pb, &mut bufsize);
            let bufend = bufdata.add(bufsize);

            // With -S write the raw buffer bytes; otherwise sanitise each line
            // through cmd_paste_buffer_paste (C cmd-paste-buffer.c:108-124).
            let raw = args_has(args, 'S');
            loop {
                let line: *mut u8 =
                    libc::memchr(bufdata as _, b'\n' as i32, bufend.addr() - bufdata.addr()).cast();
                if line.is_null() {
                    break;
                }

                let len = line.addr() - bufdata.addr();
                if raw {
                    bufferevent_write((*wp).event, bufdata.cast(), len);
                } else {
                    cmd_paste_buffer_paste(wp, bufdata, len);
                }
                bufferevent_write((*wp).event, sepstr.cast(), seplen);

                bufdata = line.add(1);
            }
            if bufdata != bufend {
                let len = bufend.addr() - bufdata.addr();
                if raw {
                    bufferevent_write((*wp).event, bufdata.cast(), len);
                } else {
                    cmd_paste_buffer_paste(wp, bufdata, len);
                }
            }

            if bracket
                && (*(*wp).screen)
                    .mode
                    .intersects(mode_flag::MODE_BRACKETPASTE)
            {
                bufferevent_write((*wp).event, c!("\x1b[201~").cast(), 6);
            }
        }

        if let Some(non_null_pb) = NonNull::new(pb)
            && args_has(args, 'd')
        {
            paste_free(non_null_pb);
        }

        cmd_retval::CMD_RETURN_NORMAL
    }
}
