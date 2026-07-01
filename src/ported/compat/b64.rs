use core::mem::MaybeUninit;

// https://www.rfc-editor.org/rfc/rfc4648

/// C `vendor/tmux/compat/base64.c:126`: `int b64_ntop(unsigned char const *src, size_t srclength, char *target, size_t targsize)`
pub unsafe fn b64_ntop(src: *const u8, srclength: usize, target: *mut u8, targsize: usize) -> i32 {
    let src = unsafe { std::slice::from_raw_parts(src, srclength) };
    let dst = unsafe { std::slice::from_raw_parts_mut(target.cast::<MaybeUninit<u8>>(), targsize) };

    // C `base64.c:176-177`: returns `datalength`, the count of encoded chars
    // excluding the trailing NUL. `ntop` returns a slice of exactly that length
    // (the NUL is written past the slice end), so return `out.len()` directly.
    // Empty input -> len 0 -> returns 0 and writes a lone NUL, matching C.
    match ntop(src, dst) {
        Ok(out) => out.len() as i32,
        Err(()) => -1,
    }
}

/// skips all whitespace anywhere.
/// converts characters, four at a time, starting at (or after)
/// src from base - 64 numbers into three 8 bit bytes in the target area.
/// it returns the number of data bytes stored at the target, or -1 on error.
/// C `vendor/tmux/compat/base64.c:187`: `int b64_pton(char const *src, unsigned char *target, size_t targsize)`
pub unsafe fn b64_pton(src: *const u8, target: *mut u8, targsize: usize) -> i32 {
    let srclength: usize = unsafe { crate::libc::strlen(src) };
    let src = unsafe { std::slice::from_raw_parts(src.cast::<u8>(), srclength) };
    let dst = unsafe { std::slice::from_raw_parts_mut(target.cast::<MaybeUninit<u8>>(), targsize) };

    match pton(src, dst) {
        Ok(out) => out.len() as i32,
        Err(()) => -1,
    }
}

/// minimum ascii value used in encoded format
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
/// Reverse map, one slot per possible byte value (0..=255). A slot holds the
/// 6-bit base64 value for that ASCII char, or `u8::MAX` for non-base64 bytes.
/// This is the equivalent of C's `strchr(Base64, ch)` (base64.c:203): the whole
/// alphabet is present, digits included.
const REVERSE: [u8; 256] = const {
    let mut tmp = [u8::MAX; 256];

    let mut i: u8 = 0;
    while i < ALPHABET.len() as u8 {
        tmp[ALPHABET[i as usize] as usize] = i;
        i += 1;
    }

    tmp
};

/// decode — faithful port of the C state machine (base64.c:187-309).
///
/// Skips whitespace anywhere (base64.c:197), breaks on the Pad64 `=`
/// (base64.c:200-201), and decodes with the 0/1/2/3 state machine
/// (base64.c:207-253), writing into `dst` with the per-write
/// `tarindex >= targsize` bounds checks. After the loop it validates the pad
/// (base64.c:261-306): `=` in state 0/1 is an error; state 2 requires a second
/// trailing `=`; only whitespace may follow the final `=`; and the "extra bits"
/// that slop past the last full byte must be zero (subliminal-channel check).
/// Returns the number of decoded bytes (`tarindex`), or `Err` (-1) on error.
/// Unlike the encoder, C does not NUL-terminate the output, so neither do we.
fn pton<'out>(src: &'_ [u8], dst: &'out mut [MaybeUninit<u8>]) -> Result<&'out mut [u8], ()> {
    let targsize = dst.len();
    let mut tarindex: usize = 0;
    let mut state: u8 = 0;
    let mut idx = 0;
    let mut got_pad = false;

    // Read a value already written into `dst[i]` so we can OR into it, mirroring
    // C's `target[tarindex] |= ...`. Every index we read here was written by an
    // earlier state, so it is initialized.
    macro_rules! cur {
        ($i:expr) => {
            unsafe { dst[$i].assume_init() }
        };
    }

    while idx < src.len() {
        let ch = src[idx];
        idx += 1;

        if ch.is_ascii_whitespace() {
            continue; // base64.c:197-198
        }
        if ch == b'=' {
            got_pad = true; // base64.c:200-201
            break;
        }

        let pos = REVERSE[ch as usize];
        if pos == u8::MAX {
            return Err(()); // non-base64 char, base64.c:204-205
        }

        match state {
            0 => {
                // base64.c:208-215
                if tarindex >= targsize {
                    return Err(());
                }
                dst[tarindex] = MaybeUninit::new(pos << 2);
                state = 1;
            }
            1 => {
                // base64.c:216-229
                if tarindex >= targsize {
                    return Err(());
                }
                dst[tarindex] = MaybeUninit::new(cur!(tarindex) | pos >> 4);
                let nextbyte = (pos & 0x0f) << 4;
                if tarindex + 1 < targsize {
                    dst[tarindex + 1] = MaybeUninit::new(nextbyte);
                } else if nextbyte != 0 {
                    return Err(());
                }
                tarindex += 1;
                state = 2;
            }
            2 => {
                // base64.c:230-243
                if tarindex >= targsize {
                    return Err(());
                }
                dst[tarindex] = MaybeUninit::new(cur!(tarindex) | pos >> 2);
                let nextbyte = (pos & 0x03) << 6;
                if tarindex + 1 < targsize {
                    dst[tarindex + 1] = MaybeUninit::new(nextbyte);
                } else if nextbyte != 0 {
                    return Err(());
                }
                tarindex += 1;
                state = 3;
            }
            _ => {
                // state == 3, base64.c:244-252
                if tarindex >= targsize {
                    return Err(());
                }
                dst[tarindex] = MaybeUninit::new(cur!(tarindex) | pos);
                tarindex += 1;
                state = 0;
            }
        }
    }

    if got_pad {
        // We got a pad char (base64.c:261). `idx` points just past the first `=`.
        match state {
            0 | 1 => return Err(()), // invalid `=` position, base64.c:264-266
            2 => {
                // base64.c:268-278: skip whitespace, require a second trailing `=`.
                let mut second_pad = false;
                while idx < src.len() {
                    let c = src[idx];
                    idx += 1;
                    if !c.is_ascii_whitespace() {
                        second_pad = c == b'=';
                        break;
                    }
                }
                if !second_pad {
                    return Err(());
                }
                // Fall through to the state-3 tail check below.
            }
            _ => {} // state == 3, fall through to the tail check
        }

        // base64.c:280-297 (case 3): only whitespace may follow the final `=`,
        // and the extra bits past the last full byte must be zero.
        while idx < src.len() {
            let c = src[idx];
            idx += 1;
            if !c.is_ascii_whitespace() {
                return Err(());
            }
        }
        if tarindex < targsize && cur!(tarindex) != 0 {
            return Err(());
        }
    } else if state != 0 {
        // Ended at end-of-string with a partial group, base64.c:304-305.
        return Err(());
    }

    Ok(unsafe { std::slice::from_raw_parts_mut(dst.as_mut_ptr().cast::<u8>(), tarindex) })
}

/// encode
fn ntop<'out>(src: &'_ [u8], dst: &'out mut [MaybeUninit<u8>]) -> Result<&'out mut [u8], ()> {
    if dst.len() < src.len().div_ceil(3) * 4 + 1 {
        return Err(());
    }

    let mut i = 0;
    let mut it = src.chunks_exact(3);

    macro_rules! enc {
        ($e:expr) => {
            MaybeUninit::new(ALPHABET[($e & 0b00111111) as usize])
        }
    }

    for chunk in &mut it {
        dst[i] = enc!(chunk[0] >> 2);
        dst[i + 1] = enc!(chunk[0] << 4 | chunk[1] >> 4);
        dst[i + 2] = enc!(chunk[1] << 2 | chunk[2] >> 6);
        dst[i + 3] = enc!(chunk[2]);
        i += 4;
    }

    let chunk = it.remainder();
    match chunk.len() {
        0 => (),
        1 => {
            dst[i] = enc!(chunk[0] >> 2);
            dst[i + 1] = enc!(chunk[0] << 4);
            dst[i + 2] = MaybeUninit::new(b'=');
            dst[i + 3] = MaybeUninit::new(b'=');
            i += 4;
        }
        2 => {
            dst[i] = enc!(chunk[0] >> 2);
            dst[i + 1] = enc!(chunk[0] << 4 | chunk[1] >> 4);
            dst[i + 2] = enc!(chunk[1] << 2);
            dst[i + 3] = MaybeUninit::new(b'=');
            i += 4;
        }
        _ => unreachable!(),
    }

    dst[i] = MaybeUninit::new(b'\0');
    Ok(unsafe { std::slice::from_raw_parts_mut(dst.as_mut_ptr().cast::<u8>(), i) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_b64_pton_valid() {
        let input = crate::c!("TWFu");
        let mut output = [0u8; 4];
        let expected = [b'M', b'a', b'n', 0];

        unsafe {
            let result = b64_pton(input, output.as_mut_ptr(), output.len());
            assert_eq!(&output, &expected);
            assert_eq!(result, 3);
        }
    }

    #[test]
    fn test_b64_pton_invalid() {
        let input = crate::c!("****");
        let mut output = [0u8; 3];

        unsafe {
            let result = b64_pton(input, output.as_mut_ptr(), output.len());
            assert_eq!(result, -1);
        }
    }

    // Single trailing '=' (16-bit final quantum): "TWE=" decodes to "Ma"
    // (base64.c:280-297). Two decoded bytes, tarindex 2.
    #[test]
    fn test_b64_pton_partial() {
        let input = crate::c!("TWE=");
        let mut output = [0u8; 2];

        unsafe {
            let result = b64_pton(input, output.as_mut_ptr(), output.len());
            assert_eq!(result, 2);
            assert_eq!(&output, b"Ma");
        }
    }

    // Encode a whole 3-byte quantum ("Man" -> "TWFu"). The RFC 4648 vector.
    // C b64_ntop (base64.c:176-177) returns `datalength` = 4, the number of
    // encoded chars excluding the trailing NUL. Pins the exact encoded bytes,
    // the NUL terminator, and the return value.
    #[test]
    fn test_b64_ntop_full_quantum() {
        let mut out = [0xffu8; 8];
        let ret = unsafe { b64_ntop(b"Man".as_ptr(), 3, out.as_mut_ptr(), out.len()) };
        assert_eq!(&out[..5], b"TWFu\0");
        assert_eq!(ret, 4);
    }

    // One leftover byte -> two chars + "==" padding.
    #[test]
    fn test_b64_ntop_one_pad() {
        let mut out = [0xffu8; 8];
        let ret = unsafe { b64_ntop(b"M".as_ptr(), 1, out.as_mut_ptr(), out.len()) };
        assert_eq!(&out[..5], b"TQ==\0");
        assert_eq!(ret, 4);
    }

    // Two leftover bytes -> three chars + "=" padding.
    #[test]
    fn test_b64_ntop_two_pad() {
        let mut out = [0xffu8; 8];
        let ret = unsafe { b64_ntop(b"Ma".as_ptr(), 2, out.as_mut_ptr(), out.len()) };
        assert_eq!(&out[..5], b"TWE=\0");
        assert_eq!(ret, 4);
    }

    // Multi-quantum, no padding: "foobar" (6 bytes) -> "Zm9vYmFy".
    #[test]
    fn test_b64_ntop_multi_quantum() {
        let mut out = [0xffu8; 16];
        let ret = unsafe { b64_ntop(b"foobar".as_ptr(), 6, out.as_mut_ptr(), out.len()) };
        assert_eq!(&out[..9], b"Zm9vYmFy\0");
        assert_eq!(ret, 8);
    }

    // Empty input: C b64_ntop writes a lone NUL and returns datalength 0
    // (base64.c:176-177). The port now returns `out.len()` (== 0) with no
    // underflow, matching C: no panic, returns 0, out[0] == NUL.
    #[test]
    fn test_b64_ntop_empty_returns_zero() {
        let mut out = [0xffu8; 4];
        let ret = unsafe { b64_ntop(b"".as_ptr(), 0, out.as_mut_ptr(), out.len()) };
        assert_eq!(ret, 0);
        assert_eq!(out[0], 0);
    }

    // Target too small: "Man" needs 4 chars + NUL = 5 bytes; give 4 -> error.
    #[test]
    fn test_b64_ntop_target_too_small() {
        let mut out = [0xffu8; 4];
        let ret = unsafe { b64_ntop(b"Man".as_ptr(), 3, out.as_mut_ptr(), 4) };
        assert_eq!(ret, -1);
    }

    // High-bit bytes exercise the full 6-bit alphabet including '+' and '/'.
    // 0xFB 0xFF 0xBF -> "+/+/".
    #[test]
    fn test_b64_ntop_plus_slash() {
        let mut out = [0xffu8; 8];
        let ret = unsafe { b64_ntop([0xFB, 0xFF, 0xBFu8].as_ptr(), 3, out.as_mut_ptr(), out.len()) };
        assert_eq!(&out[..5], b"+/+/\0");
        assert_eq!(ret, 4);
    }

    // Round trip on a padding-free (multiple-of-3) buffer. "Man" encodes to
    // "TWFu", decoded back to the original bytes.
    #[test]
    fn test_b64_roundtrip_no_padding() {
        let src = b"Man"; // 3 bytes -> "TWFu", no padding
        let mut enc = [0u8; 16];
        unsafe {
            b64_ntop(src.as_ptr(), src.len(), enc.as_mut_ptr(), enc.len());
        }
        assert_eq!(&enc[..5], b"TWFu\0");
        // enc is NUL-terminated; feed it back to b64_pton.
        let mut dec = [0u8; 16];
        let n = unsafe { b64_pton(enc.as_ptr(), dec.as_mut_ptr(), dec.len()) };
        assert_eq!(n, 3);
        assert_eq!(&dec[..3], src);
    }

    // Digits are part of the base64 alphabet (values 52-61) and decode via the
    // reverse table just like C's strchr over the whole table (base64.c:203).
    // "Zm9vYmFy" is base64 for "foobar" and contains '9'; it decodes to 6 bytes.
    #[test]
    fn test_b64_pton_decodes_digits() {
        let mut out = [0u8; 16];
        let n = unsafe { b64_pton(crate::c!("Zm9vYmFy"), out.as_mut_ptr(), out.len()) };
        assert_eq!(n, 6);
        assert_eq!(&out[..6], b"foobar");
    }

    // Whitespace is skipped anywhere in the input (base64.c:197). "TW Fu"
    // decodes the same as "TWFu".
    #[test]
    fn test_b64_pton_skips_whitespace() {
        let input = crate::c!("TW Fu");
        let mut out = [0u8; 8];
        let n = unsafe { b64_pton(input, out.as_mut_ptr(), out.len()) };
        assert_eq!(n, 3);
        assert_eq!(&out[..3], b"Man");
    }

    // A length that is not a multiple of 4 after filtering has an incomplete
    // final group -> error.
    #[test]
    fn test_b64_pton_incomplete_group() {
        let input = crate::c!("TWF");
        let mut out = [0u8; 8];
        let n = unsafe { b64_pton(input, out.as_mut_ptr(), out.len()) };
        assert_eq!(n, -1);
    }

    // Double trailing '=' (8-bit final quantum): "TQ==" decodes to one byte "M"
    // (base64.c:268-297). tarindex 1.
    #[test]
    fn test_b64_pton_two_pad() {
        let mut out = [0u8; 8];
        let n = unsafe { b64_pton(crate::c!("TQ=="), out.as_mut_ptr(), out.len()) };
        assert_eq!(n, 1);
        assert_eq!(out[0], b'M');
    }

    // Subliminal-channel check (base64.c:289-297): for a "==" (one-byte) quantum,
    // the second base64 char's low 4 bits must be zero. "TZ==" has Z (value 25,
    // low nibble 0x9 != 0), so the extra bits are non-zero and decoding fails.
    #[test]
    fn test_b64_pton_subliminal_bits_rejected() {
        let mut out = [0u8; 8];
        let n = unsafe { b64_pton(crate::c!("TZ=="), out.as_mut_ptr(), out.len()) };
        assert_eq!(n, -1);
    }

    // A single '=' where two pads are required (8-bit quantum stopped in state 2
    // with only one '=') is an error: no second trailing '=' (base64.c:274-275).
    #[test]
    fn test_b64_pton_missing_second_pad() {
        let mut out = [0u8; 8];
        let n = unsafe { b64_pton(crate::c!("TQ="), out.as_mut_ptr(), out.len()) };
        assert_eq!(n, -1);
    }

    // Output buffer too small for the decoded length: "TWFuTWFu" -> 6 bytes,
    // needs div_ceil(8,4)*3+1 = 7 bytes; give 4 -> error.
    #[test]
    fn test_b64_pton_output_too_small() {
        let mut out = [0u8; 4];
        let n = unsafe { b64_pton(crate::c!("TWFuTWFu"), out.as_mut_ptr(), out.len()) };
        assert_eq!(n, -1);
    }

    // ntop then pton is the identity for every input length 0..=9, exercising all
    // three residue classes of the final quantum (0/1/2 leftover bytes ->
    // ""/"=="/"=" padding, base64.c:126/187). The ntop return value equals the
    // encoded-char count (session fix: datalength, not the NUL-inclusive size).
    #[test]
    fn test_b64_roundtrip_all_lengths() {
        let src: &[u8] = b"abcdefghi";
        for len in 0..=src.len() {
            let mut enc = [0u8; 32];
            let ncoded =
                unsafe { b64_ntop(src.as_ptr(), len, enc.as_mut_ptr(), enc.len()) };
            // Encoded length is a multiple of 4 (with padding) except empty -> 0.
            assert_eq!(ncoded as usize, if len == 0 { 0 } else { len.div_ceil(3) * 4 });
            assert_eq!(enc[ncoded as usize], 0, "NUL terminator at len {len}");

            let mut dec = [0u8; 32];
            let ndec = unsafe { b64_pton(enc.as_ptr(), dec.as_mut_ptr(), dec.len()) };
            assert_eq!(ndec, len as i32, "decoded length at len {len}");
            assert_eq!(&dec[..len], &src[..len], "roundtrip at len {len}");
        }
    }

    // Decoding an empty string yields zero bytes (base64.c:187: no quanta, no
    // pad, tarindex 0). The port returns out.len() == 0 with no panic.
    #[test]
    fn test_b64_pton_empty_returns_zero() {
        let mut out = [0u8; 8];
        let n = unsafe { b64_pton(crate::c!(""), out.as_mut_ptr(), out.len()) };
        assert_eq!(n, 0);
    }

    // All-zero input bytes encode to the first alphabet char 'A' repeated: three
    // 0x00 bytes -> 24 zero bits -> "AAAA" (ALPHABET[0] == 'A').
    #[test]
    fn test_b64_ntop_all_zero_bytes() {
        let mut out = [0xffu8; 8];
        let ret = unsafe { b64_ntop([0u8, 0, 0].as_ptr(), 3, out.as_mut_ptr(), out.len()) };
        assert_eq!(&out[..5], b"AAAA\0");
        assert_eq!(ret, 4);
        // And it decodes straight back to the zero bytes.
        let mut dec = [0xffu8; 8];
        let n = unsafe { b64_pton(crate::c!("AAAA"), dec.as_mut_ptr(), dec.len()) };
        assert_eq!(n, 3);
        assert_eq!(&dec[..3], &[0u8, 0, 0]);
    }

    // Interior whitespace of any run is skipped (base64.c:197 `isspace`): a tab
    // and newline between and after groups decode identically to the packed form.
    // "Zm9vYmFy" is base64 for "foobar".
    #[test]
    fn test_b64_pton_skips_interior_newlines() {
        let mut out = [0u8; 16];
        let n = unsafe { b64_pton(crate::c!("Zm9v\nYmFy\t"), out.as_mut_ptr(), out.len()) };
        assert_eq!(n, 6);
        assert_eq!(&out[..6], b"foobar");
    }
}
