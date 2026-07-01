#![no_main]

#[derive(arbitrary::Arbitrary, Debug)]
struct RgbInput {
    r: u8,
    g: u8,
    b: u8,
}

libfuzzer_sys::fuzz_target!(|input: RgbInput| {
    let RgbInput { r, g, b } = input;

    // Exercise the mapping for every RGB triple to catch panics / out-of-range
    // palette indices. The result carries the 256-colour flag and its low byte
    // must be a valid palette slot (16..=255: 6x6x6 cube or greyscale ramp).
    const COLOUR_FLAG_256: i32 = 0x01000000;
    let result = ztmux::colour::colour_find_rgb(r, g, b);
    assert!(
        result & COLOUR_FLAG_256 != 0 && (16..=255).contains(&(result & 0xff)),
        "colour_find_rgb out of range\nInput: r={r}, g={g}, b={b}\nResult: {result:#x}",
    );
});
