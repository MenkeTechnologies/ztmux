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

/// C `vendor/tmux/image.c:28`: `#define MAX_IMAGE_COUNT 20`
const MAX_IMAGE_COUNT: u32 = 20;

static mut ALL_IMAGES: images = TAILQ_HEAD_INITIALIZER!(ALL_IMAGES);

static mut ALL_IMAGES_COUNT: u32 = 0;

/// C `vendor/tmux/image.c:54`: `static void image_free(struct image *im)`
unsafe fn image_free(im: NonNull<image>) {
    unsafe {
        let im = im.as_ptr();
        let s = (*im).s;

        tailq_remove::<_, discr_all_entry>(&raw mut ALL_IMAGES, im);
        ALL_IMAGES_COUNT -= 1;

        tailq_remove::<_, discr_entry>(&raw mut (*s).images, im);
        crate::image_sixel::sixel_free((*im).data);
        // Reclaim the boxed image; its owned `fallback` CString drops with it.
        drop(Box::from_raw(im));
    }
}

/// C `vendor/tmux/image.c:68`: `int image_free_all(struct screen *s)`
pub unsafe fn image_free_all(s: *mut screen) -> bool {
    unsafe {
        let redraw = !tailq_empty(&raw mut (*s).images);

        for im in tailq_foreach::<_, discr_entry>(&raw mut (*s).images) {
            image_free(im);
        }
        redraw
    }
}

/// Create text placeholder for an image.
/// C `vendor/tmux/image.c:82`: `static void image_fallback(char **ret, u_int sx, u_int sy)`
pub fn image_fallback(sx: u32, sy: u32) -> CString {
    let sx = sx as usize;
    let sy = sy as usize;

    let label = CString::new(format!("SIXEL IMAGE ({sx}x{sy})\r\n")).unwrap();

    // Allocate first line.
    let lsize = label.to_bytes_with_nul().len();
    let size = if sx < lsize - 3 { lsize - 1 } else { sx + 2 };
    // Remaining lines. Every placeholder line has \r\n at the end.
    let size = size + (sx + 2) * (sy - 1) + 1;

    let mut buf: Vec<u8> = Vec::with_capacity(size);

    // Render first line.
    if sx < lsize - 3 {
        buf.extend_from_slice(label.as_bytes());
    } else {
        buf.extend_from_slice(&label.as_bytes()[..(lsize - 3)]);
        buf.extend(std::iter::repeat_n(b'+', sx - lsize + 3));
        buf.extend_from_slice("\r\n".as_bytes());
    }

    // Remaining lines.
    for _ in 1..sy {
        buf.extend(std::iter::repeat_n(b'+', sx));
        buf.extend_from_slice("\r\n".as_bytes());
    }

    CString::new(buf).unwrap()
}

/// C `vendor/tmux/image.c:123`: `struct image*image_store(struct screen *s, struct sixel_image *si)`
pub unsafe fn image_store(s: *mut screen, si: *mut sixel_image) -> *mut image {
    unsafe {
        let mut im = Box::new(image {
            s,
            data: si,
            px: (*s).cx,
            py: (*s).cy,
            sx: 0,
            sy: 0,
            fallback: None,
            all_entry: zeroed(),
            entry: zeroed(),
        });

        (im.sx, im.sy) = crate::image_sixel::sixel_size_in_cells(&*si);

        im.fallback = Some(image_fallback(im.sx, im.sy));

        tailq_insert_tail::<image, discr_entry>(&raw mut (*s).images, &mut *im);
        tailq_insert_tail::<image, discr_all_entry>(&raw mut ALL_IMAGES, &mut *im);
        ALL_IMAGES_COUNT += 1;
        if ALL_IMAGES_COUNT == MAX_IMAGE_COUNT {
            image_free(NonNull::new(tailq_first::<image>(&raw mut ALL_IMAGES)).unwrap());
        }

        Box::leak(im)
    }
}

/// C `vendor/tmux/image.c:149`: `int image_check_line(struct screen *s, u_int py, u_int ny)`
pub unsafe fn image_check_line(s: *mut screen, py: u32, ny: u32) -> bool {
    unsafe {
        let mut redraw = false;

        for im in tailq_foreach::<_, discr_entry>(&raw mut (*s).images) {
            if py + ny > (*im.as_ptr()).py && py < (*im.as_ptr()).py + (*im.as_ptr()).sy {
                image_free(im);
                redraw = true;
            }
        }
        redraw
    }
}

/// C `vendor/tmux/image.c:166`: `int image_check_area(struct screen *s, u_int px, u_int py, u_int nx, u_int ny)`
pub unsafe fn image_check_area(s: *mut screen, px: u32, py: u32, nx: u32, ny: u32) -> bool {
    unsafe {
        let mut redraw = false;

        for im in tailq_foreach::<_, discr_entry>(&raw mut (*s).images) {
            if py + ny <= (*im.as_ptr()).py || py >= (*im.as_ptr()).py + (*im.as_ptr()).sy {
                continue;
            }
            if px + nx <= (*im.as_ptr()).px || px >= (*im.as_ptr()).px + (*im.as_ptr()).sx {
                continue;
            }
            image_free(im);
            redraw = true;
        }
        redraw
    }
}

/// C `vendor/tmux/image.c:186`: `int image_scroll_up(struct screen *s, u_int lines)`
pub unsafe fn image_scroll_up(s: *mut screen, lines: u32) -> bool {
    unsafe {
        let mut redraw = false;

        for im in tailq_foreach::<_, discr_entry>(&raw mut (*s).images) {
            if (*im.as_ptr()).py >= lines {
                (*im.as_ptr()).py -= lines;
                redraw = true;
                continue;
            }
            if (*im.as_ptr()).py + (*im.as_ptr()).sy <= lines {
                image_free(im);
                redraw = true;
                continue;
            }
            let sx = (*im.as_ptr()).sx;
            let sy = ((*im.as_ptr()).py + (*im.as_ptr()).sy) - lines;

            let new = crate::image_sixel::sixel_scale(
                (*im.as_ptr()).data,
                0,
                0,
                0,
                (*im.as_ptr()).sy - sy,
                sx,
                sy,
                1,
            );
            crate::image_sixel::sixel_free((*im.as_ptr()).data);
            (*im.as_ptr()).data = new;

            (*im.as_ptr()).py = 0;
            ((*im.as_ptr()).sx, (*im.as_ptr()).sy) =
                crate::image_sixel::sixel_size_in_cells(&*(*im.as_ptr()).data);

            // Assigning the new placeholder drops the old CString — no free.
            (*im.as_ptr()).fallback =
                Some(image_fallback((*im.as_ptr()).sx, (*im.as_ptr()).sy));
            redraw = true;
        }
        redraw
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options_::{options_create, options_default};
    use std::sync::Mutex;

    // image_store()/image_free() mutate the process-global ALL_IMAGES list and
    // ALL_IMAGES_COUNT counter (image.c:24-25). screen_init() -> screen_reinit()
    // also reads GLOBAL_OPTIONS (screen.c:107). cargo runs tests in parallel
    // threads sharing all of that, so every test that builds a screen holds this
    // mutex for its whole body and frees the screen (screen_free -> image_free_all)
    // before releasing it, so siblings never observe each other's images.
    static IMAGE_LOCK: Mutex<()> = Mutex::new(());

    struct Guard(#[expect(dead_code)] std::sync::MutexGuard<'static, ()>);

    // Populate GLOBAL_OPTIONS with the server-scope defaults the same way
    // tmux.rs does at startup, so screen_reinit's "extended-keys" lookup
    // (options_get_number_) does not fatal. Publish the fully-populated set
    // only after it is built. Mirrors src/paste.rs's ensure_global_options.
    unsafe fn ensure_global_options() {
        unsafe {
            if GLOBAL_OPTIONS.is_null() {
                let o = options_create(null_mut());
                for oe in &OPTIONS_TABLE {
                    if oe.scope & OPTIONS_TABLE_SERVER != 0 {
                        options_default(o, oe);
                    }
                }
                GLOBAL_OPTIONS = o;
            }
        }
    }

    // Lock, set up options, and build a real, empty screen via screen_init.
    unsafe fn setup() -> (Guard, *mut screen) {
        unsafe {
            let g = Guard(
                IMAGE_LOCK
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
            ensure_global_options();
            let s: *mut screen = Box::into_raw(Box::new(zeroed::<screen>()));
            screen_init(s, 80, 24, 100);
            (g, s)
        }
    }

    // screen_free calls image_free_all (screen.c:171), which frees any images
    // still attached and keeps the global list/counter balanced.
    unsafe fn teardown(s: *mut screen) {
        unsafe {
            screen_free(s);
            drop(Box::from_raw(s));
        }
    }

    // Build a minimal, valid sixel image whose size in cells is exactly
    // (cols/xpixel, 6*bands/ypixel). Each '~' (0x7e) writes a full 6-pixel-tall
    // sixel band and advances one pixel column; '-' starts the next band. See
    // sixel_parse (image-sixel.c:294) and sixel_size_in_cells (image-sixel.c:398).
    unsafe fn make_sixel(cols: u32, bands: u32, xpixel: u32, ypixel: u32) -> *mut sixel_image {
        unsafe {
            // Define colour register 0 (RGB type) up front so the parsed image
            // has a non-empty `colours` Vec; sixel_scale (image-sixel.c:463)
            // clones it, which would otherwise read an uninitialised Vec.
            let mut buf: Vec<u8> = Vec::new();
            buf.extend_from_slice(b"q#0;2;0;0;0");
            for b in 0..bands {
                if b > 0 {
                    buf.push(b'-');
                }
                buf.extend(std::iter::repeat_n(b'~', cols as usize));
            }
            let si = crate::image_sixel::sixel_parse(buf.as_ptr(), buf.len(), xpixel, ypixel);
            assert!(!si.is_null(), "sixel_parse returned null");
            si
        }
    }

    // Store an image of size (2,2) cells at the current cursor position.
    unsafe fn store_2x2(s: *mut screen) -> *mut image {
        unsafe {
            let si = make_sixel(4, 2, 2, 6);
            let (esx, esy) = crate::image_sixel::sixel_size_in_cells(&*si);
            assert_eq!((esx, esy), (2, 2), "sixel geometry helper sanity");
            image_store(s, si)
        }
    }

    unsafe fn image_count(s: *mut screen) -> usize {
        unsafe { tailq_foreach::<_, discr_entry>(&raw mut (*s).images).count() }
    }

    // --- image_fallback (image.c:82): pure, no screen needed ---------------

    #[test]
    fn test_image_fallback_short_single_line() {
        // sx=1,sy=1: "SIXEL IMAGE (1x1)\r\n" is 19 bytes so lsize=20 and
        // sx(1) < lsize-3(17), so the whole label is copied and there are no
        // extra lines (image.c:90-107).
        let out = image_fallback(1, 1);
        assert_eq!(out.to_str().unwrap(), "SIXEL IMAGE (1x1)\r\n");
    }

    #[test]
    fn test_image_fallback_short_multi_line() {
        // sx=1,sy=3: label fits, then (sy-1)=2 extra lines each "+\r\n" (sx=1).
        let out = image_fallback(1, 3);
        assert_eq!(out.to_str().unwrap(), "SIXEL IMAGE (1x3)\r\n+\r\n+\r\n");
    }

    #[test]
    fn test_image_fallback_wide_pads_with_plus() {
        // sx=30,sy=2: label = "SIXEL IMAGE (30x2)\r\n" (20 bytes) -> lsize=21.
        // sx(30) >= lsize-3(18), so the first line is the visible label
        // (lsize-3 = 18 bytes, dropping "\r\n") padded with sx-lsize+3 = 12 '+'
        // then "\r\n"; the remaining (sy-1)=1 line is sx=30 '+' then "\r\n"
        // (image.c:100-113).
        let out = image_fallback(30, 2);
        let mut exp = String::from("SIXEL IMAGE (30x2)");
        exp.push_str(&"+".repeat(12)); // sx - lsize + 3 = 30 - 21 + 3 = 12
        exp.push_str("\r\n");
        exp.push_str(&"+".repeat(30)); // sx
        exp.push_str("\r\n");
        assert_eq!(out.to_str().unwrap(), exp);
    }

    // --- empty-screen behaviour (no global mutation) -----------------------

    #[test]
    fn test_free_all_and_checks_on_empty_screen() {
        unsafe {
            let (_g, s) = setup();

            // image_free_all: redraw = !TAILQ_EMPTY(&s->images) (image.c:71).
            assert!(!image_free_all(s), "empty screen: nothing to free");
            // The check/scroll helpers all iterate an empty list -> no redraw.
            assert!(!image_check_line(s, 0, 24));
            assert!(!image_check_area(s, 0, 0, 80, 24));
            assert!(!image_scroll_up(s, 1));

            teardown(s);
        }
    }

    // --- image_store (image.c:123) -----------------------------------------

    #[test]
    fn test_image_store_sets_geometry() {
        unsafe {
            let (_g, s) = setup();
            (*s).cx = 7;
            (*s).cy = 3;

            let si = make_sixel(4, 2, 2, 6);
            let (esx, esy) = crate::image_sixel::sixel_size_in_cells(&*si);
            let im = image_store(s, si);

            // px/py come from the cursor; sx/sy from sixel_size_in_cells.
            assert_eq!((*im).px, 7);
            assert_eq!((*im).py, 3);
            assert_eq!((*im).sx, esx);
            assert_eq!((*im).sy, esy);
            assert_eq!(((*im).sx, (*im).sy), (2, 2));
            assert_eq!((*im).s, s);
            assert!((*im).fallback.is_some());
            assert_eq!(image_count(s), 1);

            // Now that an image is attached, image_free_all reports a redraw
            // and empties the list.
            assert!(image_free_all(s));
            assert_eq!(image_count(s), 0);
            assert!(!image_free_all(s));

            teardown(s);
        }
    }

    // --- image_check_line (image.c:149) ------------------------------------

    #[test]
    fn test_image_check_line() {
        unsafe {
            let (_g, s) = setup();
            (*s).cx = 0;
            (*s).cy = 0; // image occupies py rows [0,2)
            store_2x2(s);

            // Line well below the image: py+ny > im.py but py >= im.py+im.sy
            // -> not in, image survives (image.c:153).
            assert!(!image_check_line(s, 5, 1));
            assert_eq!(image_count(s), 1);

            // Line 1 intersects rows [0,2): freed, redraw.
            assert!(image_check_line(s, 1, 1));
            assert_eq!(image_count(s), 0);

            teardown(s);
        }
    }

    // --- image_check_area (image.c:166) ------------------------------------

    #[test]
    fn test_image_check_area() {
        unsafe {
            let (_g, s) = setup();
            (*s).cx = 0;
            (*s).cy = 0; // image occupies px [0,2), py [0,2)
            store_2x2(s);

            // Column 5 is right of the image (px >= im.px+im.sx): not in.
            assert!(!image_check_area(s, 5, 0, 1, 1));
            assert_eq!(image_count(s), 1);

            // Row 5 is below the image (py >= im.py+im.sy): not in.
            assert!(!image_check_area(s, 0, 5, 1, 1));
            assert_eq!(image_count(s), 1);

            // Overlapping the top-left cell: freed, redraw.
            assert!(image_check_area(s, 0, 0, 1, 1));
            assert_eq!(image_count(s), 0);

            teardown(s);
        }
    }

    // --- image_scroll_up (image.c:186) -------------------------------------

    #[test]
    fn test_image_scroll_up_shifts_when_below() {
        unsafe {
            let (_g, s) = setup();
            (*s).cx = 0;
            (*s).cy = 3; // im.py = 3
            let im = store_2x2(s);

            // im.py(3) >= lines(2): py decremented, image kept (image.c:191).
            assert!(image_scroll_up(s, 2));
            assert_eq!((*im).py, 1);
            assert_eq!(image_count(s), 1);

            teardown(s);
        }
    }

    #[test]
    fn test_image_scroll_up_frees_when_fully_scrolled_off() {
        unsafe {
            let (_g, s) = setup();
            (*s).cx = 0;
            (*s).cy = 0; // im.py = 0, sy = 2
            store_2x2(s);

            // im.py+im.sy(2) <= lines(5): image freed (image.c:196).
            assert!(image_scroll_up(s, 5));
            assert_eq!(image_count(s), 0);

            teardown(s);
        }
    }

    #[test]
    fn test_image_scroll_up_partial_rescales() {
        unsafe {
            let (_g, s) = setup();
            (*s).cx = 0;
            (*s).cy = 0; // im.py = 0, sy = 2
            let im = store_2x2(s);

            // im.py(0) < lines(1) and im.py+im.sy(2) > lines(1): the image is
            // rescaled via sixel_scale, py reset to 0, image kept (image.c:202).
            assert!(image_scroll_up(s, 1));
            assert_eq!(image_count(s), 1);
            assert_eq!((*im).py, 0);
            assert!((*im).fallback.is_some());

            teardown(s);
        }
    }

    // MAX_IMAGE_COUNT: C frees the oldest global image only once the count
    // reaches 20 (image.c:26 `#define MAX_IMAGE_COUNT 20`, used image.c:139).
    // ztmux hardcodes 10 (image.rs: `if ALL_IMAGES_COUNT == 10`), so storing
    // 10 images already evicts the first one. With the C constant, all 10 would
    // still be attached to the screen.
    #[test]
    fn test_max_image_count_matches_c() {
        unsafe {
            let (_g, s) = setup();
            for _ in 0..10 {
                let si = make_sixel(4, 2, 2, 6);
                image_store(s, si);
            }
            // C: no eviction below 20, so all 10 remain on the screen.
            assert_eq!(image_count(s), 10);
            teardown(s);
        }
    }
}
