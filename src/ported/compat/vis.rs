// Copyright (c) 1989, 1993
// The Regents of the University of California.  All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions
// are met:
// 1. Redistributions of source code must retain the above copyright
//    notice, this list of conditions and the following disclaimer.
// 2. Redistributions in binary form must reproduce the above copyright
//    notice, this list of conditions and the following disclaimer in the
//    documentation and/or other materials provided with the distribution.
// 3. Neither the name of the University nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE REGENTS AND CONTRIBUTORS ``AS IS'' AND
// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED.  IN NO EVENT SHALL THE REGENTS OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
// OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
// HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
// LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
// OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
// SUCH DAMAGE.
use core::ffi::c_int;

// documentation from vis(3bsd)
bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq)]
    pub(crate) struct vis_flags: i32 {
        /// Use a three digit octal sequence. The form is '\ddd' where each 'd' represents an octal
        /// digit.
        ///
        /// ztmux considers this flag to be set unconditionally.
        const VIS_OCTAL   = 0x0001;

        /// Use C-style backslash sequences to represent standard non-printable characters.
        /// The following sequences are used to represent the indicated characters:
        /// \a - BEL (007)
        /// \b - BS  (010)
        /// \t - HT  (011)
        /// \n - NL  (012)
        /// \v - VT  (013)
        /// \f - NP  (014)
        /// \r - CR  (015)
        /// \s - SP  (040)
        /// \0 - NUL (000)
        ///
        /// ztmux considers this flag to be set unconditionally.
        const VIS_CSTYLE  = 0x0002;

        /// encode tab
        const VIS_TAB     = 0x0008;

        /// encode newline
        const VIS_NL      = 0x0010;

        /// inhibit the doubling of backslashes and the backslash before the default format
        /// (that is, control characters are represented by ‘^C’ and meta characters as ‘M-C’).
        /// with this flag set, the encoding is ambiguous and non-invertible.
        const VIS_NOSLASH = 0x0040;

        /// encode double quote
        const VIS_DQ      = 0x0200;
    }
}

/// copies into dst a string which represents the character c. If c needs no encoding, it is copied in unaltered.
/// The string is null terminated, and a pointer to the end of the string is returned.
pub unsafe fn vis_(dst: *mut u8, c: c_int, flag: vis_flags, nextc: c_int) -> *mut u8 {
    unsafe {
        match c as u8 {
            b'\0' if !matches!(nextc as u8, b'0'..=b'7') => encode_cstyle(dst, b'0'),
            b'\t' if flag.intersects(vis_flags::VIS_TAB) => encode_cstyle(dst, b't'),
            b'\n' if flag.intersects(vis_flags::VIS_NL) => encode_cstyle(dst, b'n'),
            b'\\' if !flag.intersects(vis_flags::VIS_NOSLASH) => encode_cstyle(dst, b'\\'),
            b'"' if flag.intersects(vis_flags::VIS_DQ) => encode_cstyle(dst, b'"'),
            7..9 | 11..14 => {
                const CSTYLE: [u8; 7] = [b'a', b'b', 0, 0, b'v', b'f', b'r'];
                encode_cstyle(dst, CSTYLE[c as usize - 7])
            }
            0..7 | 14..32 | 127.. => encode_octal(dst, c),
            _ => encode_passthrough(dst, c),
        }
    }
}

pub fn vis__(dst: &mut Vec<u8>, c: c_int, flag: vis_flags, nextc: c_int) {
    match c as u8 {
        b'\0' if !matches!(nextc as u8, b'0'..=b'7') => encode_cstyle_(dst, b'0'),
        b'\t' if flag.intersects(vis_flags::VIS_TAB) => encode_cstyle_(dst, b't'),
        b'\n' if flag.intersects(vis_flags::VIS_NL) => encode_cstyle_(dst, b'n'),
        b'\\' if !flag.intersects(vis_flags::VIS_NOSLASH) => encode_cstyle_(dst, b'\\'),
        b'"' if flag.intersects(vis_flags::VIS_DQ) => encode_cstyle_(dst, b'"'),
        7..9 | 11..14 => {
            const CSTYLE: [u8; 7] = [b'a', b'b', 0, 0, b'v', b'f', b'r'];
            encode_cstyle_(dst, CSTYLE[c as usize - 7]);
        }
        0..7 | 14..32 | 127.. => encode_octal_(dst, c),
        _ => encode_passthrough_(dst, c),
    }
}

#[inline]
unsafe fn encode_passthrough(dst: *mut u8, ch: i32) -> *mut u8 {
    unsafe {
        *dst = ch as u8;
        *dst.add(1) = b'\0';
        dst.add(1)
    }
}

#[inline]
fn encode_passthrough_(dst: &mut Vec<u8>, ch: i32) {
    dst.push(ch as u8);
}

#[inline]
unsafe fn encode_cstyle(dst: *mut u8, ch: u8) -> *mut u8 {
    unsafe {
        *dst = b'\\';
        *dst.add(1) = ch;
        *dst.add(2) = b'\0';
        dst.add(2)
    }
}

#[inline]
fn encode_cstyle_(dst: &mut Vec<u8>, ch: u8) {
    dst.push(b'\\');
    dst.push(ch);
}

#[inline]
unsafe fn encode_octal(dst: *mut u8, c: i32) -> *mut u8 {
    unsafe {
        let c = c as u8;
        let ones_place = c % 8;
        let eights_place = (c / 8) % 8;
        let sixty_four_place = c / 64;
        *dst = b'\\';
        *dst.add(1) = sixty_four_place + b'0';
        *dst.add(2) = eights_place + b'0';
        *dst.add(3) = ones_place + b'0';
        *dst.add(4) = b'\0';
        dst.add(4)
    }
}

fn encode_octal_(dst: &mut Vec<u8>, c: i32) {
    let c = c as u8;
    let ones_place = c % 8;
    let eights_place = (c / 8) % 8;
    let sixty_four_place = c / 64;
    dst.push(b'\\');
    dst.push(sixty_four_place + b'0');
    dst.push(eights_place + b'0');
    dst.push(ones_place + b'0');
}

/// C `vendor/tmux/compat/vis.c:155`: `int strvis(char *dst, const char *src, int flag)`
pub unsafe fn strvis(mut dst: *mut u8, mut src: *const u8, flag: vis_flags) -> i32 {
    unsafe {
        let start = dst;

        while *src != 0 {
            dst = vis_(dst, *src as i32, flag, *src.add(1) as i32);
            src = src.add(1);
        }
        *dst = 0;

        dst.offset_from(start) as i32
    }
}

/// C `vendor/tmux/compat/vis.c:167`: `int strnvis(char *dst, const char *src, size_t siz, int flag)`
///
/// Faithful port. `dlen` is C's `siz`: strnvis writes at most `siz - 1` visible
/// bytes followed by a trailing NUL, and returns the number of bytes the *fully*
/// encoded string would occupy so callers can detect truncation (vis.c:148-149,
/// return at vis.c:206).
///
/// C's strnvis (vis.c:174-197) special-cases `isvisible(c, flag)` (vis.c:41-51)
/// into a direct 1-or-2-byte copy purely to skip the `tbuf`/`memcpy` on the
/// common printable path; that branch emits bytes identical to `vis()`. This
/// port's `vis_` (the port of C `vis`, vis.rs:73) already folds the `isvisible`
/// predicate into its match — its passthrough arm (vis.rs:85-86) matches exactly
/// `isgraph(c) || c==' ' || (!VIS_TAB && c=='\t') || (!VIS_NL && c=='\n')`, which
/// is `isvisible` with the port's flag set (there is no `VIS_ALL`/`VIS_GLOB`/
/// `VIS_SP`/`VIS_SAFE` here, so those clauses are constant-0). Encoding every byte through
/// `vis_` and bounds-checking the encoded length therefore reproduces C's output,
/// truncation boundary, and return value without a standalone `isvisible`, and
/// reuses the file's only vis primitive as required.
pub unsafe fn strnvis(mut dst: *mut u8, mut src: *const u8, dlen: usize, flag: vis_flags) -> i32 {
    unsafe {
        let mut tbuf = [0u8; 8]; // C `char tbuf[5]` (vis.c:170); vis_ writes <=5 bytes
        let start = dst;
        // C `end = start + siz - 1` (vis.c:174); wrapping keeps siz==0 well-defined.
        let end = start.wrapping_add(dlen).wrapping_sub(1);
        let mut i: usize = 0;

        // C `for (...; (c = *src) && dst < end; )` (vis.c:174)
        while *src != 0 && dst < end {
            // C `i = vis(tbuf, c, flag, *++src) - tbuf;` (vis.c:189)
            let tend = vis_(tbuf.as_mut_ptr(), *src as i32, flag, *src.add(1) as i32);
            i = tend.offset_from_unsigned(tbuf.as_mut_ptr());
            src = src.add(1);
            if dst.wrapping_add(i) <= end {
                // C `memcpy(dst, tbuf, i); dst += i;` (vis.c:191-192)
                core::ptr::copy_nonoverlapping(tbuf.as_ptr(), dst, i);
                dst = dst.add(i);
            } else {
                // C `src--; break;` (vis.c:194-195)
                src = src.sub(1);
                break;
            }
        }
        // C `if (siz > 0) *dst = '\0';` (vis.c:199-200)
        if dlen > 0 {
            *dst = 0;
        }
        // C `if (dst + i > end) { while ((c = *src)) dst += vis(...) - tbuf; }` (vis.c:201-205)
        if dst.wrapping_add(i) > end {
            while *src != 0 {
                let tend = vis_(tbuf.as_mut_ptr(), *src as i32, flag, *src.add(1) as i32);
                dst = dst.wrapping_add(tend.offset_from_unsigned(tbuf.as_mut_ptr()));
                src = src.add(1);
            }
        }
        // C `return (dst - start);` (vis.c:206). Address subtraction: the
        // truncation walk above deliberately runs `dst` past the buffer.
        (dst as usize).wrapping_sub(start as usize) as i32
    }
}

/// C `vendor/tmux/compat/vis.c:210`: `int stravis(char **outp, const char *src, int flag)`
pub unsafe fn stravis(outp: *mut *mut u8, src: *const u8, flag: vis_flags) -> i32 {
    unsafe {
        let buf: *mut u8 = libc::calloc(4, crate::libc::strlen(src) + 1).cast();
        if buf.is_null() {
            return -1;
        }
        let len = strvis(buf, src, flag);
        let serrno = crate::errno!();
        *outp = libc::realloc(buf.cast(), len as usize + 1).cast();
        if (*outp).is_null() {
            *outp = buf;
            crate::errno!() = serrno;
        }

        len
    }
}

/// C `vendor/tmux/compat/vis.c:57`: `char *vis(char *dst, int c, int flag, int nextc)`
pub unsafe fn vis(dst: *mut u8, c: c_int, flag: vis_flags, nextc: c_int) -> *mut u8 {
    unsafe { vis_(dst, c, flag, nextc) }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_vis() {
        let mut c_dst_arr: [u8; 16] = [0; 16];
        let mut rs_dst_arr: [u8; 16] = [0; 16];

        let c_dst = &raw mut c_dst_arr as *mut u8;
        let rs_dst = &raw mut rs_dst_arr as *mut u8;

        unsafe {
            for f1 in [
                vis_flags::VIS_OCTAL,
                vis_flags::VIS_CSTYLE,
                vis_flags::VIS_OCTAL | vis_flags::VIS_CSTYLE,
            ] {
                for f2 in [
                    vis_flags::VIS_TAB | vis_flags::VIS_NL,
                    vis_flags::VIS_TAB,
                    vis_flags::VIS_NL,
                    vis_flags::VIS_DQ,
                    vis_flags::VIS_NOSLASH,
                ] {
                    for ch in 0..=u8::MAX {
                        for nextc in [b'\0' as i32, b'0' as i32] {
                            let flag = f1 | f2;
                            let rs_out = vis_(rs_dst, ch as i32, flag, nextc);
                            let c_out = vis(c_dst, ch as i32, flag, nextc);

                            assert_eq!(
                                c_dst_arr,
                                rs_dst_arr,
                                "mismatch when encoding vis(_, _, _, {ch}) => {} != {}",
                                crate::_s(c_dst),
                                crate::_s(rs_dst)
                            );

                            assert_eq!(rs_out.offset_from(rs_dst), c_out.offset_from(c_dst));

                            c_dst_arr.fill(0);
                            rs_dst_arr.fill(0);
                        }
                    }
                }
            }
        }
    }

    const NONE: vis_flags = vis_flags::empty();

    // Encode a single character with vis_ and return the bytes written (the
    // routine also NUL-terminates and returns a pointer to that NUL, so the
    // length is the pointer delta).
    unsafe fn enc(c: i32, flag: vis_flags, nextc: i32) -> Vec<u8> {
        unsafe {
            let mut buf = [0u8; 8];
            let end = vis_(buf.as_mut_ptr(), c, flag, nextc);
            let len = end.offset_from(buf.as_ptr()) as usize;
            buf[..len].to_vec()
        }
    }

    #[test]
    fn test_vis_printable_passthrough() {
        unsafe {
            // Graphic ASCII passes through unaltered.
            assert_eq!(enc(b'A' as i32, NONE, 0), b"A");
            assert_eq!(enc(b'z' as i32, NONE, 0), b"z");
            assert_eq!(enc(b'~' as i32, NONE, 0), b"~");
            // This port passes a literal space through (it is not in any of the
            // control/octal ranges of vis_, vis.rs:75-86).
            assert_eq!(enc(b' ' as i32, NONE, 0), b" ");
        }
    }

    #[test]
    fn test_vis_backslash() {
        unsafe {
            // Default: backslash is doubled (vis.c:61).
            assert_eq!(enc(b'\\' as i32, NONE, 0), b"\\\\");
            // VIS_NOSLASH inhibits the doubling -> single literal backslash.
            assert_eq!(enc(b'\\' as i32, vis_flags::VIS_NOSLASH, 0), b"\\");
        }
    }

    #[test]
    fn test_vis_double_quote() {
        unsafe {
            // VIS_DQ escapes '"' as '\"' (vis.c:60).
            assert_eq!(enc(b'"' as i32, vis_flags::VIS_DQ, 0), b"\\\"");
            // Without VIS_DQ the quote is a plain printable.
            assert_eq!(enc(b'"' as i32, NONE, 0), b"\"");
        }
    }

    #[test]
    fn test_vis_tab() {
        unsafe {
            // VIS_TAB -> C-style "\t"; without it the tab passes through raw.
            assert_eq!(enc(b'\t' as i32, vis_flags::VIS_TAB, 0), b"\\t");
            assert_eq!(enc(b'\t' as i32, NONE, 0), b"\t");
        }
    }

    #[test]
    fn test_vis_newline() {
        unsafe {
            // VIS_NL -> "\n"; without it the newline passes through raw.
            assert_eq!(enc(b'\n' as i32, vis_flags::VIS_NL, 0), b"\\n");
            assert_eq!(enc(b'\n' as i32, NONE, 0), b"\n");
        }
    }

    #[test]
    fn test_vis_cstyle_controls() {
        unsafe {
            // The named C-style escapes (vis.c:82-93), always active in this
            // port regardless of flags.
            assert_eq!(enc(0x07, NONE, 0), b"\\a"); // BEL
            assert_eq!(enc(0x08, NONE, 0), b"\\b"); // BS
            assert_eq!(enc(0x0b, NONE, 0), b"\\v"); // VT
            assert_eq!(enc(0x0c, NONE, 0), b"\\f"); // FF
            assert_eq!(enc(0x0d, NONE, 0), b"\\r"); // CR
        }
    }

    #[test]
    fn test_vis_nul() {
        unsafe {
            // NUL with a non-octal following char -> short "\0" form (vis.c:102).
            assert_eq!(enc(0, NONE, 0), b"\\0");
            assert_eq!(enc(0, NONE, b'x' as i32), b"\\0");
            // NUL followed by an octal digit must use the full three-digit octal
            // so the decoder can't merge the digits (vis.c:105 doubles the 0;
            // this port emits the octal escape instead).
            assert_eq!(enc(0, NONE, b'0' as i32), b"\\000");
            assert_eq!(enc(0, NONE, b'7' as i32), b"\\000");
        }
    }

    #[test]
    fn test_vis_octal_ranges() {
        unsafe {
            // Non-named control chars and all high-bit bytes -> three-digit
            // octal (vis.rs:85).
            assert_eq!(enc(0x01, NONE, 0), b"\\001");
            assert_eq!(enc(0x06, NONE, 0), b"\\006");
            assert_eq!(enc(0x0e, NONE, 0), b"\\016");
            assert_eq!(enc(0x1f, NONE, 0), b"\\037");
            assert_eq!(enc(0x7f, NONE, 0), b"\\177"); // DEL
            assert_eq!(enc(0x80, NONE, 0), b"\\200");
            assert_eq!(enc(0xff, NONE, 0), b"\\377");
        }
    }

    #[test]
    fn test_strvis_literals_and_length() {
        unsafe {
            let mut dst = [0u8; 32];
            let ret = strvis(dst.as_mut_ptr(), crate::c!("abc"), NONE);
            assert_eq!(ret, 3);
            assert_eq!(&dst[..4], b"abc\0");
        }
    }

    #[test]
    fn test_strvis_tab_and_backslash() {
        unsafe {
            // "a<TAB>b" with VIS_TAB -> "a\tb" (4 visible bytes).
            let mut dst = [0u8; 32];
            let ret = strvis(dst.as_mut_ptr(), crate::c!("a\tb"), vis_flags::VIS_TAB);
            assert_eq!(ret, 4);
            assert_eq!(&dst[..5], b"a\\tb\0");

            // A literal backslash in the source is doubled.
            let mut dst2 = [0u8; 32];
            let ret2 = strvis(dst2.as_mut_ptr(), crate::c!("a\\b"), NONE);
            assert_eq!(ret2, 4);
            assert_eq!(&dst2[..5], b"a\\\\b\0");
        }
    }

    #[test]
    fn test_strvis_control_expands_to_octal() {
        unsafe {
            // A lone 0x01 expands to the four-byte octal escape "\001".
            let mut dst = [0u8; 32];
            let ret = strvis(dst.as_mut_ptr(), crate::c!("\x01"), NONE);
            assert_eq!(ret, 4);
            assert_eq!(&dst[..5], b"\\001\0");
        }
    }

    #[test]
    fn test_strnvis_truncates_to_dlen() {
        // Faithful BSD strnvis (vis.c:167): writes at most siz-1 visible bytes
        // plus a NUL, and RETURNS the length the fully-encoded string would have
        // had, so callers can detect truncation (vis.c:148-149, 206). Previously
        // this port added `dst.offset_from_unsigned(dst)` (always 0), so the dlen
        // bound was never enforced and the return was always 0; that is now
        // fixed. Here dlen=2 => siz-1=1 visible byte fits ("a"), the NUL lands at
        // index 1, and the return is 3 (the full encoding of "abc").
        unsafe {
            let mut dst = [0u8; 32];
            let ret = strnvis(dst.as_mut_ptr(), crate::c!("abc"), 2, NONE);
            assert_eq!(ret, 3);
            assert_eq!(&dst[..2], b"a\0");
        }
    }

    #[test]
    fn test_strnvis_in_bounds() {
        // A comfortably-sized dst encodes fully, NUL-terminates, and returns the
        // visible length (vis.c:206). "a<TAB>b" with VIS_TAB -> "a\tb" (4 bytes).
        unsafe {
            let mut dst = [0u8; 32];
            let ret = strnvis(dst.as_mut_ptr(), crate::c!("abc"), 32, NONE);
            assert_eq!(ret, 3);
            assert_eq!(&dst[..4], b"abc\0");

            let mut dst2 = [0u8; 32];
            let ret2 = strnvis(dst2.as_mut_ptr(), crate::c!("a\tb"), 32, vis_flags::VIS_TAB);
            assert_eq!(ret2, 4);
            assert_eq!(&dst2[..5], b"a\\tb\0");
        }
    }

    #[test]
    fn test_strnvis_stops_at_multibyte_boundary() {
        // With VIS_TAB the tab encodes to the two bytes "\t". dlen=3 leaves only
        // one free slot after writing 'a' (siz-1 = 2, one used by 'a'), so the
        // two-byte escape does not fit: strnvis stops after 'a', NUL-terminates
        // at index 1, and still returns the full would-be length 4 ("a\tb" ->
        // "a\\tb"). Exercises the vis.c:193-195 `src--; break;` truncation path
        // plus the vis.c:201-205 return-value adjustment walk.
        unsafe {
            let mut dst = [0u8; 32];
            let ret = strnvis(dst.as_mut_ptr(), crate::c!("a\tb"), 3, vis_flags::VIS_TAB);
            assert_eq!(ret, 4);
            assert_eq!(&dst[..2], b"a\0");
        }
    }

    // strvis output must round-trip through the ported strunvis for a mix of a
    // literal, an octal-escaped control byte, and a doubled backslash. This
    // pins the encode/decode pair, not just the encoder in isolation.
    #[test]
    fn test_strvis_roundtrips_through_strunvis() {
        unsafe {
            for src in [c"abc".as_ptr(), c"a\x01b".as_ptr(), c"x\\y".as_ptr(), c"\x7f\xff".as_ptr()] {
                let mut enc = [0u8; 32];
                let n = strvis(enc.as_mut_ptr(), src.cast(), NONE);
                assert!(n >= 0);
                // enc is NUL-terminated by strvis; decode it back.
                let mut dec = [0u8; 32];
                let m = crate::compat::strunvis(dec.as_mut_ptr(), enc.as_ptr());
                assert!(m >= 0, "strunvis failed on encoding of {}", crate::_s(src.cast::<u8>()));
                let orig = std::slice::from_raw_parts(src.cast::<u8>(), crate::libc::strlen(src.cast()));
                assert_eq!(&dec[..m as usize], orig);
            }
        }
    }

    // C `strnvis` with siz == 0 (vis.c:174 `end = start - 1`, :199 `if (siz>0)`):
    // writes NO bytes (not even a NUL) but still returns the length the full
    // encoding would occupy, so callers can size a buffer. dst is untouched.
    #[test]
    fn test_strnvis_dlen_zero_writes_nothing() {
        unsafe {
            let mut dst = [0xAAu8; 8];
            let ret = strnvis(dst.as_mut_ptr(), crate::c!("abc"), 0, NONE);
            assert_eq!(ret, 3, "returns full would-be length");
            // Nothing was written: the buffer keeps its sentinel bytes.
            assert!(dst.iter().all(|&b| b == 0xAA), "dst must be untouched");
        }
    }

    // VIS_NL escapes a newline to the C-style "\n" in strvis; without the flag it
    // passes through raw (vis.rs:78, 95). Pins the whole-string path, not just
    // single-char vis_.
    #[test]
    fn test_strvis_newline_flag() {
        unsafe {
            let mut dst = [0u8; 32];
            let ret = strvis(dst.as_mut_ptr(), crate::c!("a\nb"), vis_flags::VIS_NL);
            assert_eq!(ret, 4);
            assert_eq!(&dst[..5], b"a\\nb\0");

            let mut dst2 = [0u8; 32];
            let ret2 = strvis(dst2.as_mut_ptr(), crate::c!("a\nb"), NONE);
            assert_eq!(ret2, 3);
            assert_eq!(&dst2[..4], b"a\nb\0");
        }
    }

    // Exact-fit boundary: "abc" needs 3 visible bytes + NUL = 4. With dlen == 4
    // the whole string fits, NUL lands at index 3, and the return is the visible
    // length 3 (vis.c:174 `dst < end` with end == start+3).
    #[test]
    fn test_strnvis_exact_fit() {
        unsafe {
            let mut dst = [0u8; 8];
            let ret = strnvis(dst.as_mut_ptr(), crate::c!("abc"), 4, NONE);
            assert_eq!(ret, 3);
            assert_eq!(&dst[..4], b"abc\0");
            // One byte short (dlen == 3): only "ab" fits, NUL at index 2, still
            // returns the full 3.
            let mut dst2 = [0u8; 8];
            let ret2 = strnvis(dst2.as_mut_ptr(), crate::c!("abc"), 3, NONE);
            assert_eq!(ret2, 3);
            assert_eq!(&dst2[..3], b"ab\0");
        }
    }
}
