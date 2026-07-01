// Both strtonum variants below classify a base-10 parse the same way C
// `strtonum` (vendor/tmux/compat/strtonum.c:52-58) does: C uses `strtoll`,
// which saturates to `LLONG_MIN`/`LLONG_MAX` and sets `errno == ERANGE` on
// overflow, then reports "too small" / "too large" rather than "invalid".
// Rust's `i64` parse surfaces the same distinction via
// `IntErrorKind::NegOverflow` / `PosOverflow` (strtonum.c:55-57); genuine
// non-numeric input (strtonum.c:53) stays "invalid". The mapping is inlined at
// each call site rather than extracted into a helper, because a free `fn` with
// no tmux C counterpart trips the ported-fn-names anti-drift gate.

/// C `vendor/tmux/compat/strtonum.c:31`: `long long strtonum(const char *numstr, long long minval, long long maxval, const char **errstrp)`
pub unsafe fn strtonum<T>(
    nptr: *const u8,
    minval: T,
    maxval: T,
) -> Result<T, &'static core::ffi::CStr>
where
    T: Into<i64>,
    i64: TryInto<T>,
{
    let minval: i64 = minval.into();
    let maxval: i64 = maxval.into();

    if minval > maxval {
        return Err(c"invalid");
    }

    let buf = unsafe { std::slice::from_raw_parts(nptr, crate::libc::strlen(nptr)) };
    let s = std::str::from_utf8(buf).map_err(|_| c"invalid")?;
    let n = s.trim_start().parse::<i64>().map_err(|e| match e.kind() {
        core::num::IntErrorKind::PosOverflow => c"too large",
        core::num::IntErrorKind::NegOverflow => c"too small",
        _ => c"invalid",
    })?;

    if n < minval {
        return Err(c"too small");
    }

    if n > maxval {
        return Err(c"too large");
    }

    match n.try_into() {
        Ok(value) => Ok(value),
        Err(_) => unreachable!("range check above should prevent this case"),
    }
}

pub fn strtonum_<T>(s: &str, minval: T, maxval: T) -> Result<T, &'static core::ffi::CStr>
where
    T: Into<i64>,
    i64: TryInto<T>,
{
    let minval: i64 = minval.into();
    let maxval: i64 = maxval.into();

    if minval > maxval {
        return Err(c"invalid");
    }

    let n = s.trim_start().parse::<i64>().map_err(|e| match e.kind() {
        core::num::IntErrorKind::PosOverflow => c"too large",
        core::num::IntErrorKind::NegOverflow => c"too small",
        _ => c"invalid",
    })?;

    if n < minval {
        return Err(c"too small");
    }

    if n > maxval {
        return Err(c"too large");
    }

    match n.try_into() {
        Ok(value) => Ok(value),
        Err(_) => unreachable!("range check above should prevent this case"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strtonum_valid() {
        assert_eq!(strtonum_("42", 0i32, 100i32), Ok(42));
    }

    #[test]
    fn test_strtonum_leading_whitespace() {
        // trim_start allows leading spaces.
        assert_eq!(strtonum_("   7", 0i32, 100i32), Ok(7));
    }

    #[test]
    fn test_strtonum_too_small() {
        assert_eq!(strtonum_("-5", 0i32, 100i32), Err(c"too small"));
    }

    #[test]
    fn test_strtonum_too_large() {
        assert_eq!(strtonum_("200", 0i32, 100i32), Err(c"too large"));
    }

    #[test]
    fn test_strtonum_non_numeric() {
        assert_eq!(strtonum_("abc", 0i32, 100i32), Err(c"invalid"));
    }

    #[test]
    fn test_strtonum_inverted_range() {
        // minval > maxval is rejected before parsing.
        assert_eq!(strtonum_("5", 100i32, 0i32), Err(c"invalid"));
    }

    #[test]
    fn test_strtonum_inclusive_boundaries() {
        // Both bounds are inclusive (C strtonum.c:55-58 uses `< minval` /
        // `> maxval`, so equality is accepted).
        assert_eq!(strtonum_("100", 0i32, 100i32), Ok(100));
        assert_eq!(strtonum_("0", 0i32, 100i32), Ok(0));
        assert_eq!(strtonum_("101", 0i32, 100i32), Err(c"too large"));
        assert_eq!(strtonum_("-1", 0i32, 100i32), Err(c"too small"));
    }

    #[test]
    fn test_strtonum_single_point_range() {
        // minval == maxval is a valid one-value window.
        assert_eq!(strtonum_("5", 5i32, 5i32), Ok(5));
        assert_eq!(strtonum_("6", 5i32, 5i32), Err(c"too large"));
        assert_eq!(strtonum_("4", 5i32, 5i32), Err(c"too small"));
    }

    #[test]
    fn test_strtonum_negative_range() {
        assert_eq!(strtonum_("-50", -100i32, 100i32), Ok(-50));
        assert_eq!(strtonum_("-100", -100i32, 100i32), Ok(-100));
        assert_eq!(strtonum_("-101", -100i32, 100i32), Err(c"too small"));
    }

    #[test]
    fn test_strtonum_leading_plus() {
        // Rust's i64 parse accepts an explicit '+' sign, like C strtoll.
        assert_eq!(strtonum_("+42", 0i32, 100i32), Ok(42));
    }

    #[test]
    fn test_strtonum_trailing_garbage() {
        // C strtonum.c:53 rejects a non-empty `*ep`; the Rust port relies on
        // i64 parse, which likewise rejects any trailing byte (digits, letters,
        // or a trailing space, since only leading whitespace is trimmed).
        assert_eq!(strtonum_("42x", 0i32, 100i32), Err(c"invalid"));
        assert_eq!(strtonum_("42 ", 0i32, 100i32), Err(c"invalid"));
        assert_eq!(strtonum_("4 2", 0i32, 100i32), Err(c"invalid"));
    }

    #[test]
    fn test_strtonum_empty_string() {
        // No digits at all -> invalid (C: numstr == ep).
        assert_eq!(strtonum_("", 0i32, 100i32), Err(c"invalid"));
        assert_eq!(strtonum_("   ", 0i32, 100i32), Err(c"invalid"));
    }

    #[test]
    fn test_strtonum_hex_not_accepted() {
        // strtonum parses base 10 only (C strtonum.c:52 passes 10), so "0x1f"
        // stops after the '0' and the trailing "x1f" makes it invalid.
        assert_eq!(strtonum_("0x1f", 0i32, 100i32), Err(c"invalid"));
    }

    #[test]
    fn test_strtonum_full_i64_range() {
        // The generic accumulator is i64; exercise the extreme bounds.
        assert_eq!(strtonum_("9223372036854775807", i64::MIN, i64::MAX), Ok(i64::MAX));
        assert_eq!(strtonum_("-9223372036854775808", i64::MIN, i64::MAX), Ok(i64::MIN));
    }

    #[test]
    fn test_strtonum_overflow_beyond_i64() {
        // A value past LLONG_MAX makes C strtonum return "too large"
        // (strtonum.c:57, `ll == LLONG_MAX && errno == ERANGE`). The Rust port
        // maps `IntErrorKind::PosOverflow` to the same string, so it now
        // matches C instead of falling through to "invalid".
        assert_eq!(strtonum_("99999999999999999999999", i64::MIN, i64::MAX), Err(c"too large"));
    }

    #[test]
    fn test_strtonum_overflow_positive_small_max() {
        // Overflowing the i64 accumulator classifies as "too large" even when
        // the requested max is small (strtonum.c:57).
        assert_eq!(strtonum_("99999999999999999999", 0i32, 100i32), Err(c"too large"));
    }

    #[test]
    fn test_strtonum_overflow_negative_small_max() {
        // Underflowing past LLONG_MIN classifies as "too small"
        // (strtonum.c:55, `ll == LLONG_MIN && errno == ERANGE`).
        assert_eq!(strtonum_("-99999999999999999999", 0i32, 100i32), Err(c"too small"));
    }

    #[test]
    fn test_strtonum_overflow_garbage_still_invalid() {
        // Non-numeric input is not an overflow; it stays "invalid"
        // (strtonum.c:53-54).
        assert_eq!(strtonum_("abc", 0i32, 100i32), Err(c"invalid"));
    }

    #[test]
    fn test_strtonum_ptr_overflow() {
        // The pointer variant uses the same inlined overflow classification, so
        // it is identical to the &str helper.
        unsafe {
            assert_eq!(
                strtonum(crate::c!("99999999999999999999"), i64::MIN, i64::MAX),
                Err(c"too large")
            );
            assert_eq!(
                strtonum(crate::c!("-99999999999999999999"), i64::MIN, i64::MAX),
                Err(c"too small")
            );
        }
    }

    #[test]
    fn test_strtonum_ptr_variant() {
        // The pointer-taking `strtonum` (used by the ported C call sites) must
        // agree with the &str helper.
        unsafe {
            assert_eq!(strtonum(crate::c!("55"), 0i32, 100i32), Ok(55));
            assert_eq!(strtonum(crate::c!("999"), 0i32, 100i32), Err(c"too large"));
            assert_eq!(strtonum(crate::c!("nope"), 0i32, 100i32), Err(c"invalid"));
        }
    }

    // The generic result type narrows the i64 accumulator back to T via
    // `try_into` (strtonum.rs:44). A u8 window must round-trip the extreme in-band
    // value 255 without the unreachable panic, and reject out-of-band values with
    // the range strings.
    #[test]
    fn test_strtonum_u8_narrowing() {
        assert_eq!(strtonum_("255", 0u8, 255u8), Ok(255u8));
        assert_eq!(strtonum_("0", 0u8, 255u8), Ok(0u8));
        assert_eq!(strtonum_("256", 0u8, 255u8), Err(c"too large"));
        // A tighter window still bounded by u8's own range.
        assert_eq!(strtonum_("200", 0u8, 100u8), Err(c"too large"));
    }

    // An i16 window classifies below/above as too small/too large at the
    // requested bounds, not the type extremes (strtonum.rs:68-73).
    #[test]
    fn test_strtonum_i16_window() {
        assert_eq!(strtonum_("-100", -100i16, 100i16), Ok(-100i16));
        assert_eq!(strtonum_("100", -100i16, 100i16), Ok(100i16));
        assert_eq!(strtonum_("-101", -100i16, 100i16), Err(c"too small"));
        assert_eq!(strtonum_("101", -100i16, 100i16), Err(c"too large"));
    }

    // "-0" parses to 0 (i64 parse accepts the sign, value is zero), which is in
    // band for a 0-based window.
    #[test]
    fn test_strtonum_negative_zero() {
        assert_eq!(strtonum_("-0", 0i32, 100i32), Ok(0));
        assert_eq!(strtonum_("+0", 0i32, 100i32), Ok(0));
    }

    // A lone sign or a sign followed by only whitespace has no digits, so the
    // i64 parse fails as invalid (C strtonum.c:53, numstr == ep).
    #[test]
    fn test_strtonum_lone_sign_invalid() {
        assert_eq!(strtonum_("-", 0i32, 100i32), Err(c"invalid"));
        assert_eq!(strtonum_("+", 0i32, 100i32), Err(c"invalid"));
        // Leading whitespace is trimmed, but "  -" still has no digit.
        assert_eq!(strtonum_("  -", 0i32, 100i32), Err(c"invalid"));
    }
}
