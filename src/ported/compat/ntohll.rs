use ::libc;
pub type __uint32_t = libc::c_uint;
pub type __uint64_t = libc::c_ulong;
pub type uint32_t = __uint32_t;
pub type uint64_t = __uint64_t;
#[inline]
unsafe fn __bswap_32(mut __bsx: __uint32_t) -> __uint32_t {
    return (__bsx & 0xff000000 as libc::c_uint) >> 24 as libc::c_int
        | (__bsx & 0xff0000 as libc::c_uint) >> 8 as libc::c_int
        | (__bsx & 0xff00 as libc::c_uint) << 8 as libc::c_int
        | (__bsx & 0xff as libc::c_uint) << 24 as libc::c_int;
}

/// C `vendor/tmux/compat/ntohll.c:23`: `uint64_t ntohll(uint64_t v)`
pub unsafe fn ntohll(mut v: uint64_t) -> uint64_t {
    let mut b: uint32_t = 0;
    let mut t: uint32_t = 0;
    b = __bswap_32((v & 0xffffffff as libc::c_uint as libc::c_ulong) as __uint32_t);
    t = __bswap_32((v >> 32 as libc::c_int) as __uint32_t);
    return (b as uint64_t) << 32 as libc::c_int | t as libc::c_ulong;
}

#[cfg(test)]
mod tests {
    use super::*;

    // The C source (vendor/tmux/compat/ntohll.c) computes:
    //   b = ntohl(v & 0xffffffff); t = ntohl(v >> 32); return ((uint64_t)b << 32 | t);
    // On a little-endian host ntohl is a 32-bit byte swap, so ntohll is a full
    // 64-bit byte reversal. This Rust port applies __bswap_32 unconditionally,
    // matching little-endian behavior (all supported test hosts are LE).

    #[test]
    fn test_ntohll_known_constant() {
        // 0x0123456789ABCDEF byte-reversed is 0xEFCDAB8967452301.
        // low32  = 0x89ABCDEF -> bswap -> 0xEFCDAB89 (b)
        // high32 = 0x01234567 -> bswap -> 0x67452301 (t)
        // result = (b << 32) | t = 0xEFCDAB8967452301
        unsafe {
            assert_eq!(ntohll(0x0123456789ABCDEF), 0xEFCDAB8967452301);
        }
    }

    #[test]
    fn test_ntohll_matches_swap_bytes() {
        // On little-endian hosts ntohll is exactly a 64-bit byte swap.
        for &v in &[
            0u64,
            1,
            0xFF,
            0xFF00000000000000,
            0xDEADBEEFCAFEBABE,
            u64::MAX,
            0x0000_0001_0000_0000,
        ] {
            unsafe {
                assert_eq!(ntohll(v), v.swap_bytes());
            }
        }
    }

    #[test]
    fn test_ntohll_roundtrip() {
        // Applying the byte swap twice restores the original value.
        for &v in &[
            0u64,
            0x1,
            0xABCDEF0123456789,
            u64::MAX,
            0x8000_0000_0000_0001,
        ] {
            unsafe {
                assert_eq!(ntohll(ntohll(v)), v);
            }
        }
    }

    #[test]
    fn test_ntohll_edge_values() {
        unsafe {
            assert_eq!(ntohll(0), 0);
            // A single low byte moves to the most-significant byte.
            assert_eq!(ntohll(0xFF), 0xFF00000000000000);
            // The 32-bit halves are swapped as whole units (plus inner swap).
            assert_eq!(ntohll(0x0000_0000_FFFF_FFFF), 0xFFFF_FFFF_0000_0000);
        }
    }

    #[test]
    fn test_ntohll_each_byte_position() {
        // Byte i (value 1 << (8*i)) must land at position 7-i after a full
        // 64-bit reversal: __bswap_32 on each half then the halves are swapped
        // (vendor/tmux/compat/ntohll.c:23).
        unsafe {
            for i in 0u32..8 {
                let v: u64 = 1u64 << (8 * i);
                let expected: u64 = 1u64 << (8 * (7 - i));
                assert_eq!(ntohll(v), expected, "byte position {i}");
            }
        }
    }

    #[test]
    fn test_ntohll_low_word_reverses_into_high() {
        // 0x0000_0000_1234_5678: only the low 32 bits are set. The port swaps
        // that half (0x78563412) and shifts it into the high word.
        unsafe {
            assert_eq!(ntohll(0x0000_0000_1234_5678), 0x7856_3412_0000_0000);
        }
    }

    #[test]
    fn test_ntohll_high_word_reverses_into_low() {
        // Mirror of the previous case: only the high 32 bits are set.
        unsafe {
            assert_eq!(ntohll(0x1234_5678_0000_0000), 0x0000_0000_7856_3412);
        }
    }

    #[test]
    fn test_ntohll_alternating_and_palindrome() {
        unsafe {
            // 0xAA55... reversed is still a byte-wise swap.
            assert_eq!(ntohll(0xAA55_AA55_AA55_AA55), 0xAA55_AA55_AA55_AA55u64.swap_bytes());
            // 0x00FF repeated swaps to 0xFF00 repeated.
            assert_eq!(ntohll(0x00FF_00FF_00FF_00FF), 0xFF00_FF00_FF00_FF00);
        }
    }
}
