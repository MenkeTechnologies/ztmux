// Copyright (c) 2010 Nicholas Marriott <nicholas.marriott@gmail.com>
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
use crate::libc::sscanf;
use crate::*;

/// C `vendor/tmux/layout-custom.c:37`: `static struct layout_cell *layout_find_bottomright(struct layout_cell *lc)`
pub unsafe fn layout_find_bottomright(mut lc: *mut layout_cell) -> *mut layout_cell {
    unsafe {
        if (*lc).type_ == layout_type::LAYOUT_WINDOWPANE {
            return lc;
        }
        lc = tailq_last(&raw mut (*lc).cells);
        layout_find_bottomright(lc)
    }
}

/// C `vendor/tmux/layout-custom.c:47`: `static u_short layout_checksum(const char *layout)`
pub unsafe fn layout_checksum(mut layout: *const u8) -> u16 {
    unsafe {
        let mut csum = 0u16;
        while *layout != b'\0' {
            csum = (csum >> 1) + ((csum & 1) << 15);
            // C: `csum += *layout` on a u_short relies on unsigned wraparound;
            // use wrapping_add so debug builds match instead of panicking.
            csum = csum.wrapping_add(*layout as u16);
            layout = layout.add(1);
        }
        csum
    }
}

/// Dump layout as a string.
/// C `vendor/tmux/layout-custom.c:61`: `char *layout_dump(struct window *w, struct layout_cell *root)`
pub unsafe fn layout_dump(w: *mut window, root: *mut layout_cell) -> Option<String> {
    unsafe {
        let mut layout: MaybeUninit<[u8; 8192]> = MaybeUninit::<[u8; 8192]>::uninit();
        let layout = layout.as_mut_ptr() as *mut u8;

        *layout = b'\0' as _;
        if layout_append(root, layout, 8192) != 0 {
            return None;
        }

        // Floating panes are appended after the tiled tree as `<cell,cell,…>`.
        // They sit at the head of the z-index list, so stop at the first
        // non-floating pane.
        let mut bracket = false;
        for wp in tailq_foreach::<_, discr_zentry>(&raw mut (*w).z_index).map(NonNull::as_ptr) {
            if window_pane_is_floating(wp) == 0 {
                break;
            }
            if !bracket {
                strlcat(layout, c!("<"), 8192);
                bracket = true;
            }
            if layout_append((*wp).layout_cell, layout, 8192) != 0 {
                return None;
            }
            strlcat(layout, c!(","), 8192);
        }
        if bracket {
            *layout.add(strlen(layout) - 1) = b'>';
        }

        Some(format!("{:04x},{}", layout_checksum(layout), _s(layout)))
    }
}

/// C `vendor/tmux/layout-custom.c:91`: `static int layout_append(struct layout_cell *lc, char *buf, size_t len)`
pub unsafe fn layout_append(lc: *mut layout_cell, buf: *mut u8, len: usize) -> i32 {
    unsafe {
        let sizeof_tmp = 64;
        let mut tmp = MaybeUninit::<[u8; 64]>::uninit();
        let tmp = tmp.as_mut_ptr() as *mut u8;

        let mut brackets = c!("][");

        if len == 0 {
            return -1;
        }

        let tmplen = if !(*lc).wp.is_null() {
            xsnprintf_!(
                tmp,
                sizeof_tmp,
                "{}x{},{},{},{}",
                (*lc).sx,
                (*lc).sy,
                (*lc).xoff,
                (*lc).yoff,
                (*(*lc).wp).id,
            )
            .unwrap()
        } else {
            xsnprintf_!(
                tmp,
                sizeof_tmp,
                "{}x{},{},{}",
                (*lc).sx,
                (*lc).sy,
                (*lc).xoff,
                (*lc).yoff,
            )
            .unwrap()
        };

        if tmplen > sizeof_tmp - 1 {
            return -1;
        }
        if strlcat(buf, tmp, len) >= len {
            return -1;
        }

        if ((*lc).type_) == layout_type::LAYOUT_LEFTRIGHT {
            brackets = c!("}{");
        }

        match (*lc).type_ {
            layout_type::LAYOUT_LEFTRIGHT | layout_type::LAYOUT_TOPBOTTOM => {
                if strlcat(buf, brackets.add(1), len) >= len {
                    return -1;
                }
                for lcchild in tailq_foreach(&raw mut (*lc).cells) {
                    if layout_append(lcchild.as_ptr(), buf, len) != 0 {
                        return -1;
                    }
                    if strlcat(buf, c!(","), len) >= len {
                        return -1;
                    }
                }
                *buf.add(strlen(buf) - 1) = *brackets;
            }
            layout_type::LAYOUT_WINDOWPANE => (),
        }
    }
    0
}

/// Check layout sizes fit.
/// C `vendor/tmux/layout-custom.c:138`: `static int layout_check(struct layout_cell *lc)`
pub unsafe fn layout_check(lc: *mut layout_cell) -> bool {
    unsafe {
        let mut n = 0u32;

        match (*lc).type_ {
            layout_type::LAYOUT_WINDOWPANE => (),
            layout_type::LAYOUT_LEFTRIGHT => {
                for lcchild in tailq_foreach(&raw mut (*lc).cells).map(NonNull::as_ptr) {
                    if (*lcchild).sy != (*lc).sy {
                        return false;
                    }
                    if !layout_check(lcchild) {
                        return false;
                    }
                    n += (*lcchild).sx + 1;
                }
                if n - 1 != (*lc).sx {
                    return false;
                }
            }
            layout_type::LAYOUT_TOPBOTTOM => {
                for lcchild in tailq_foreach(&raw mut (*lc).cells).map(NonNull::as_ptr) {
                    if (*lcchild).sx != (*lc).sx {
                        return false;
                    }
                    if !layout_check(lcchild) {
                        return false;
                    }
                    n += (*lcchild).sy + 1;
                }
                if n - 1 != (*lc).sy {
                    return false;
                }
            }
        }
    }
    true
}

/// C `vendor/tmux/layout-custom.c:174`: `int layout_parse(struct window *w, const char *layout, char **cause)`
pub unsafe fn layout_parse(w: *mut window, mut layout: *const u8, cause: *mut *mut u8) -> i32 {
    let __func__ = c!("layout_parse");
    unsafe {
        let mut lc: *mut layout_cell;
        let mut csum: u16 = 0;

        'fail: {
            // Check validity.
            if sscanf(layout.cast(), c"%hx,".as_ptr(), &raw mut csum) != 1 {
                *cause = xstrdup_(c"invalid layout").as_ptr();
                return -1;
            }
            layout = layout.add(5);
            if csum != layout_checksum(layout) {
                *cause = xstrdup_(c"invalid layout").as_ptr();
                return -1;
            }

            // Build the layout.
            lc = layout_construct(null_mut(), &raw mut layout);
            if lc.is_null() {
                *cause = xstrdup_(c"invalid layout").as_ptr();
                return -1;
            }
            if *layout != b'\0' {
                *cause = xstrdup_(c"invalid layout").as_ptr();
                break 'fail;
            }

            // Check this window will fit into the layout.
            loop {
                let npanes = window_count_panes(w);
                let ncells = layout_count_cells(lc);
                if npanes > ncells {
                    *cause = format_nul!("have {} panes but need {}", npanes, ncells);
                    break 'fail;
                }
                if npanes == ncells {
                    break;
                }

                // Fewer panes than cells - close the bottom right.
                let lcchild = layout_find_bottomright(lc);
                layout_destroy_cell(w, lcchild, &raw mut lc);
            }

            // It appears older versions of tmux were able to generate layouts with
            // an incorrect top cell size - if it is larger than the top child then
            // correct that (if this is still wrong the check code will catch it).
            let mut sy = 0;
            let mut sx = 0;
            match (*lc).type_ {
                layout_type::LAYOUT_WINDOWPANE => (),
                layout_type::LAYOUT_LEFTRIGHT => {
                    for lcchild in tailq_foreach(&raw mut (*lc).cells).map(NonNull::as_ptr) {
                        sy = (*lcchild).sy + 1;
                        sx += (*lcchild).sx + 1;
                        continue;
                    }
                }
                layout_type::LAYOUT_TOPBOTTOM => {
                    for lcchild in tailq_foreach(&raw mut (*lc).cells).map(NonNull::as_ptr) {
                        sx = (*lcchild).sx + 1;
                        sy += (*lcchild).sy + 1;
                        continue;
                    }
                }
            }
            if (*lc).type_ != layout_type::LAYOUT_WINDOWPANE && ((*lc).sx != sx || (*lc).sy != sy) {
                log_debug!("fix layout {},{} to {},{}", (*lc).sx, (*lc).sy, sx, sy);
                layout_print_cell(lc, __func__, 0);
                (*lc).sx = sx - 1;
                (*lc).sy = sy - 1;
            }

            // Check the new layout.
            if !layout_check(lc) {
                *cause = xstrdup_(c"size mismatch after applying layout").as_ptr();
                break 'fail;
            }

            // Resize to the layout size.
            window_resize(w, (*lc).sx, (*lc).sy, -1, -1);

            // Destroy the old layout and swap to the new.
            layout_free_cell((*w).layout_root, 0);
            (*w).layout_root = lc;

            // Assign the panes into the cells.
            let mut wp = tailq_first(&raw mut (*w).panes);
            layout_assign(&raw mut wp, lc);

            // Update pane offsets and sizes.
            layout_fix_offsets(w);
            layout_fix_panes(w, null_mut());
            recalculate_sizes();

            layout_print_cell(lc, __func__, 0);

            notify_window(c"window-layout-changed", w);

            return 0;
        }
        // fail:
        layout_free_cell(lc, 0);
        -1
    }
}

/// Assign panes into cells.
/// C `vendor/tmux/layout-custom.c:300`: `static void layout_assign(struct window_pane **wp, struct layout_cell *lc, int flags)`
unsafe fn layout_assign(wp: *mut *mut window_pane, lc: *mut layout_cell) {
    unsafe {
        match (*lc).type_ {
            layout_type::LAYOUT_WINDOWPANE => {
                layout_make_leaf(lc, *wp);
                *wp = tailq_next::<_, _, discr_entry>(*wp);
            }
            layout_type::LAYOUT_LEFTRIGHT | layout_type::LAYOUT_TOPBOTTOM => {
                for lcchild in tailq_foreach(&raw mut (*lc).cells).map(NonNull::as_ptr) {
                    layout_assign(wp, lcchild);
                }
            }
        }
    }
}

/// Construct a cell from all or part of a layout tree.
/// C `vendor/tmux/layout-custom.c:376`: `static int layout_construct(struct layout_cell *lcparent, const char **layout, struct layout_cell **lc)`
unsafe fn layout_construct(lcparent: *mut layout_cell, layout: *mut *const u8) -> *mut layout_cell {
    unsafe {
        let lc;
        let mut sx = 0u32;
        let mut sy = 0u32;
        let mut xoff = 0u32;
        let mut yoff = 0u32;

        'fail: {
            if !(**layout).is_ascii_digit() {
                return null_mut();
            }
            if sscanf(
                (*layout).cast(),
                c"%ux%u,%u,%u".as_ptr(),
                &raw mut sx,
                &raw mut sy,
                &raw mut xoff,
                &raw mut yoff,
            ) != 4
            {
                return null_mut();
            }

            while isdigit(**layout as i32) != 0 {
                (*layout) = (*layout).add(1);
            }
            if **layout != b'x' {
                return null_mut();
            }
            (*layout) = (*layout).add(1);
            while isdigit(**layout as i32) != 0 {
                (*layout) = (*layout).add(1);
            }
            if **layout != b',' {
                return null_mut();
            }
            (*layout) = (*layout).add(1);
            while isdigit(**layout as i32) != 0 {
                (*layout) = (*layout).add(1);
            }
            if **layout != b',' {
                return null_mut();
            }
            (*layout) = (*layout).add(1);
            while isdigit(**layout as i32) != 0 {
                (*layout) = (*layout).add(1);
            }
            if **layout == b',' {
                let saved = *layout;
                (*layout) = (*layout).add(1);
                while isdigit(**layout as i32) != 0 {
                    (*layout) = (*layout).add(1);
                }
                if **layout == b'x' {
                    *layout = saved;
                }
            }

            lc = layout_create_cell(lcparent);
            (*lc).sx = sx;
            (*lc).sy = sy;
            (*lc).xoff = xoff;
            (*lc).yoff = yoff;

            match **layout {
                b',' | b'}' | b']' | b'\0' => return lc,
                b'{' => (*lc).type_ = layout_type::LAYOUT_LEFTRIGHT,
                b'[' => (*lc).type_ = layout_type::LAYOUT_TOPBOTTOM,
                _ => break 'fail,
            }

            loop {
                (*layout) = (*layout).add(1);
                let lcchild = layout_construct(lc, layout);
                if lcchild.is_null() {
                    break 'fail;
                }
                tailq_insert_tail(&raw mut (*lc).cells, lcchild);
                if **layout != b',' {
                    break;
                }
            }

            match (*lc).type_ {
                layout_type::LAYOUT_LEFTRIGHT => {
                    if **layout != b'}' {
                        break 'fail;
                    }
                }
                layout_type::LAYOUT_TOPBOTTOM => {
                    if **layout != b']' {
                        break 'fail;
                    }
                }
                _ => break 'fail,
            }
            (*layout) = (*layout).add(1);

            return lc;
        }
        // fail:
        layout_free_cell(lc, 0);
        null_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Build a leaf (WINDOWPANE) cell. Mirrors the helper in src/layout.rs
    // tests. No pane is attached, so layout_append emits the 4-field
    // "SXxSY,XOFF,YOFF" form (vendor/tmux/layout-custom.c:100).
    unsafe fn leaf(sx: u32, sy: u32, xoff: u32, yoff: u32) -> *mut layout_cell {
        unsafe {
            let lc = layout_create_cell(null_mut());
            (*lc).type_ = layout_type::LAYOUT_WINDOWPANE;
            layout_set_size(lc, sx, sy, xoff, yoff);
            lc
        }
    }

    // Build a node cell of `type_` with the given children linked in order.
    unsafe fn node(
        type_: layout_type,
        sx: u32,
        sy: u32,
        xoff: u32,
        yoff: u32,
        children: &[*mut layout_cell],
    ) -> *mut layout_cell {
        unsafe {
            let p = layout_create_cell(null_mut());
            (*p).type_ = type_;
            layout_set_size(p, sx, sy, xoff, yoff);
            for &c in children {
                (*c).parent = p;
                tailq_insert_tail(&raw mut (*p).cells, c);
            }
            p
        }
    }

    // Dump a tiled tree with a zeroed window (empty z-index → no floating suffix).
    unsafe fn dump(root: *mut layout_cell) -> Option<String> {
        unsafe {
            let mut w: window = std::mem::zeroed();
            layout_dump(&raw mut w, root)
        }
    }

    // layout_checksum: csum = (csum >> 1) + ((csum & 1) << 15); csum += *c
    // over the string, u16 arithmetic (vendor/tmux/layout-custom.c:47).
    #[test]
    fn checksum_matches_c_algorithm() {
        unsafe {
            // Empty string never enters the loop -> 0.
            assert_eq!(layout_checksum(c!("")), 0);
            // Values precomputed by running the exact C loop.
            assert_eq!(layout_checksum(c!("80x24,0,0")), 0xc85e);
            assert_eq!(layout_checksum(c!("1x1,0,0")), 0x145f);
            // Rotate-and-add is order sensitive: "ab" != "ba".
            assert_ne!(layout_checksum(c!("ab")), layout_checksum(c!("ba")));
        }
    }

    // A single windowpane cell: layout_append emits "SXxSY,XOFF,YOFF" and
    // layout_dump prepends the %04x checksum (vendor/tmux/layout-custom.c:61).
    #[test]
    fn dump_single_windowpane() {
        unsafe {
            let lc = leaf(80, 24, 0, 0);
            assert_eq!(dump(lc).unwrap(), "c85e,80x24,0,0");
            layout_free_cell(lc, 0);
        }
    }

    // Non-zero offsets are dumped verbatim.
    #[test]
    fn dump_windowpane_with_offsets() {
        unsafe {
            let lc = leaf(80, 24, 3, 7);
            assert_eq!(dump(lc).unwrap(), "8866,80x24,3,7");
            layout_free_cell(lc, 0);
        }
    }

    // LEFTRIGHT uses "{...}" brackets and joins children with commas
    // (vendor/tmux/layout-custom.c:102).
    #[test]
    fn dump_leftright_split() {
        unsafe {
            let a = leaf(80, 24, 0, 0);
            let b = leaf(80, 24, 81, 0);
            let root = node(layout_type::LAYOUT_LEFTRIGHT, 160, 24, 0, 0, &[a, b]);
            assert_eq!(
                dump(root).unwrap(),
                "b198,160x24,0,0{80x24,0,0,80x24,81,0}"
            );
            layout_free_cell(root, 0);
        }
    }

    // TOPBOTTOM uses "[...]" brackets.
    #[test]
    fn dump_topbottom_split() {
        unsafe {
            let a = leaf(80, 24, 0, 0);
            let b = leaf(80, 23, 0, 25);
            let root = node(layout_type::LAYOUT_TOPBOTTOM, 80, 48, 0, 0, &[a, b]);
            assert_eq!(
                dump(root).unwrap(),
                "8e60,80x48,0,0[80x24,0,0,80x23,0,25]"
            );
            layout_free_cell(root, 0);
        }
    }

    // Nested tree: a TOPBOTTOM subtree sitting alongside a leaf inside a
    // LEFTRIGHT root exercises the recursion in layout_append.
    #[test]
    fn dump_nested_tree() {
        unsafe {
            let inner_a = leaf(80, 24, 0, 0);
            let inner_b = leaf(80, 23, 0, 25);
            let top = node(
                layout_type::LAYOUT_TOPBOTTOM,
                80,
                48,
                0,
                0,
                &[inner_a, inner_b],
            );
            let right = leaf(79, 48, 81, 0);
            let root = node(layout_type::LAYOUT_LEFTRIGHT, 160, 48, 0, 0, &[top, right]);
            assert_eq!(
                dump(root).unwrap(),
                "d3c8,160x48,0,0{80x48,0,0[80x24,0,0,80x23,0,25],79x48,81,0}"
            );
            layout_free_cell(root, 0);
        }
    }

    // layout_append into a zero-length buffer returns -1 immediately
    // (vendor/tmux/layout-custom.c:96).
    #[test]
    fn append_zero_len_fails() {
        unsafe {
            let lc = leaf(80, 24, 0, 0);
            let mut buf = [0u8; 1];
            assert_eq!(layout_append(lc, buf.as_mut_ptr(), 0), -1);
            layout_free_cell(lc, 0);
        }
    }

    // layout_append succeeds writing into a buffer and NUL-terminates
    // the "SXxSY,XOFF,YOFF" form for a leaf.
    #[test]
    fn append_leaf_writes_string() {
        unsafe {
            let lc = leaf(12, 34, 5, 6);
            let mut buf = [0u8; 64];
            buf[0] = b'\0';
            assert_eq!(layout_append(lc, buf.as_mut_ptr(), buf.len()), 0);
            assert_eq!(format!("{}", _s(buf.as_ptr())), "12x34,5,6");
            layout_free_cell(lc, 0);
        }
    }

    // layout_find_bottomright walks TAILQ_LAST recursively down to a
    // windowpane (vendor/tmux/layout-custom.c:37).
    #[test]
    fn find_bottomright_returns_leaf() {
        unsafe {
            // A plain leaf is its own bottom-right.
            let solo = leaf(80, 24, 0, 0);
            assert_eq!(layout_find_bottomright(solo), solo);
            layout_free_cell(solo, 0);

            // LEFTRIGHT { a, TOPBOTTOM { x, y } } -> deepest last child y.
            let x = leaf(80, 24, 0, 0);
            let y = leaf(80, 23, 0, 25);
            let inner = node(layout_type::LAYOUT_TOPBOTTOM, 80, 48, 80, 0, &[x, y]);
            let a = leaf(79, 48, 0, 0);
            let root = node(layout_type::LAYOUT_LEFTRIGHT, 160, 48, 0, 0, &[a, inner]);
            assert_eq!(layout_find_bottomright(root), y);
            layout_free_cell(root, 0);
        }
    }

    // layout_check: a windowpane always fits (vendor/tmux/layout-custom.c:134).
    #[test]
    fn check_windowpane_always_true() {
        unsafe {
            let lc = leaf(80, 24, 0, 0);
            assert!(layout_check(lc));
            layout_free_cell(lc, 0);
        }
    }

    // A consistent LEFTRIGHT: each child sy == parent sy and the widths plus
    // one border each sum to parent sx + 1 (n - 1 == lc->sx).
    #[test]
    fn check_valid_leftright() {
        unsafe {
            // 80 + 1 + 80 = 161 columns; children share sy = 24.
            let a = leaf(80, 24, 0, 0);
            let b = leaf(80, 24, 81, 0);
            let root = node(layout_type::LAYOUT_LEFTRIGHT, 161, 24, 0, 0, &[a, b]);
            assert!(layout_check(root));
            layout_free_cell(root, 0);
        }
    }

    // Mismatched child height fails the LEFTRIGHT check.
    #[test]
    fn check_leftright_height_mismatch_fails() {
        unsafe {
            let a = leaf(80, 24, 0, 0);
            let b = leaf(80, 23, 81, 0); // sy != parent sy
            let root = node(layout_type::LAYOUT_LEFTRIGHT, 161, 24, 0, 0, &[a, b]);
            assert!(!layout_check(root));
            layout_free_cell(root, 0);
        }
    }

    // Wrong total width fails the LEFTRIGHT check (n - 1 != lc->sx).
    #[test]
    fn check_leftright_width_mismatch_fails() {
        unsafe {
            let a = leaf(80, 24, 0, 0);
            let b = leaf(80, 24, 81, 0);
            // Parent claims 200 cols but children only account for 161.
            let root = node(layout_type::LAYOUT_LEFTRIGHT, 200, 24, 0, 0, &[a, b]);
            assert!(!layout_check(root));
            layout_free_cell(root, 0);
        }
    }

    // A consistent TOPBOTTOM: each child sx == parent sx and heights plus one
    // border each sum to parent sy + 1.
    #[test]
    fn check_valid_topbottom() {
        unsafe {
            // 24 + 1 + 23 = 48 rows; children share sx = 80.
            let a = leaf(80, 24, 0, 0);
            let b = leaf(80, 23, 0, 25);
            let root = node(layout_type::LAYOUT_TOPBOTTOM, 80, 48, 0, 0, &[a, b]);
            assert!(layout_check(root));
            layout_free_cell(root, 0);
        }
    }

    // Mismatched child width fails the TOPBOTTOM check (child sx != parent sx,
    // vendor/tmux/layout-custom.c:150).
    #[test]
    fn check_topbottom_width_mismatch_fails() {
        unsafe {
            let a = leaf(80, 24, 0, 0);
            let b = leaf(79, 23, 0, 25); // sx != parent sx
            let root = node(layout_type::LAYOUT_TOPBOTTOM, 80, 48, 0, 0, &[a, b]);
            assert!(!layout_check(root));
            layout_free_cell(root, 0);
        }
    }

    // Wrong total height fails the TOPBOTTOM check (n - 1 != lc->sy).
    #[test]
    fn check_topbottom_height_mismatch_fails() {
        unsafe {
            let a = leaf(80, 24, 0, 0);
            let b = leaf(80, 23, 0, 25);
            // Parent claims 60 rows but children only account for 48.
            let root = node(layout_type::LAYOUT_TOPBOTTOM, 80, 60, 0, 0, &[a, b]);
            assert!(!layout_check(root));
            layout_free_cell(root, 0);
        }
    }

    // layout_construct parses a single "SXxSY,XOFF,YOFF" leaf and consumes the
    // whole string (vendor/tmux/layout-custom.c:376).
    #[test]
    fn construct_single_leaf() {
        unsafe {
            let mut p: *const u8 = c!("80x24,3,7");
            let lc = layout_construct(null_mut(), &raw mut p);
            assert!(!lc.is_null());
            assert!((*lc).type_ == layout_type::LAYOUT_WINDOWPANE);
            assert_eq!((*lc).sx, 80);
            assert_eq!((*lc).sy, 24);
            assert_eq!((*lc).xoff, 3);
            assert_eq!((*lc).yoff, 7);
            // Parser advanced to the terminating NUL.
            assert_eq!(*p, b'\0');
            layout_free_cell(lc, 0);
        }
    }

    // A trailing pane id ("...,ID") is consumed by the parser but not stored on
    // the cell (vendor/tmux/layout-custom.c:399 rewind logic).
    #[test]
    fn construct_leaf_with_pane_id() {
        unsafe {
            let mut p: *const u8 = c!("80x24,0,0,5");
            let lc = layout_construct(null_mut(), &raw mut p);
            assert!(!lc.is_null());
            assert!((*lc).type_ == layout_type::LAYOUT_WINDOWPANE);
            assert!((*lc).wp.is_null());
            assert_eq!(*p, b'\0');
            layout_free_cell(lc, 0);
        }
    }

    // layout_construct builds a LEFTRIGHT node with two child leaves of the
    // parsed sizes (vendor/tmux/layout-custom.c:406, '{' -> LEFTRIGHT).
    #[test]
    fn construct_leftright_split() {
        unsafe {
            let mut p: *const u8 = c!("160x24,0,0{80x24,0,0,79x24,81,0}");
            let lc = layout_construct(null_mut(), &raw mut p);
            assert!(!lc.is_null());
            assert!((*lc).type_ == layout_type::LAYOUT_LEFTRIGHT);
            assert_eq!((*lc).sx, 160);

            let c0 = tailq_first(&raw mut (*lc).cells);
            let c1 = tailq_next(c0);
            assert_eq!((*c0).sx, 80);
            assert_eq!((*c1).sx, 79);
            assert_eq!((*c1).xoff, 81);
            assert!(tailq_next(c1).is_null());
            assert_eq!(*p, b'\0');
            layout_free_cell(lc, 0);
        }
    }

    // '[' produces a TOPBOTTOM node.
    #[test]
    fn construct_topbottom_split() {
        unsafe {
            let mut p: *const u8 = c!("80x48,0,0[80x24,0,0,80x23,0,25]");
            let lc = layout_construct(null_mut(), &raw mut p);
            assert!(!lc.is_null());
            assert!((*lc).type_ == layout_type::LAYOUT_TOPBOTTOM);
            let c0 = tailq_first(&raw mut (*lc).cells);
            let c1 = tailq_next(c0);
            assert_eq!((*c0).sy, 24);
            assert_eq!((*c1).sy, 23);
            assert_eq!((*c1).yoff, 25);
            layout_free_cell(lc, 0);
        }
    }

    // A layout that does not begin with a digit is rejected
    // (vendor/tmux/layout-custom.c:387).
    #[test]
    fn construct_rejects_non_digit() {
        unsafe {
            let mut p: *const u8 = c!("xyz");
            let lc = layout_construct(null_mut(), &raw mut p);
            assert!(lc.is_null());
        }
    }

    // Round trip: layout_construct then layout_dump reproduces the input body.
    // dump prepends a "%04x," checksum, so strip it and compare the tail.
    #[test]
    fn construct_dump_roundtrip() {
        unsafe {
            for body in [
                "80x24,0,0",
                "160x24,0,0{80x24,0,0,79x24,81,0}",
                "80x48,0,0[80x24,0,0,80x23,0,25]",
                "160x48,0,0{80x48,0,0[80x24,0,0,80x23,0,25],79x48,81,0}",
            ] {
                let mut s = body.to_string();
                s.push('\0');
                let mut p: *const u8 = s.as_bytes().as_ptr();
                let lc = layout_construct(null_mut(), &raw mut p);
                assert!(!lc.is_null(), "construct failed for {body}");
                let dumped = dump(lc).unwrap();
                let (_csum, tail) = dumped.split_once(',').unwrap();
                assert_eq!(tail, body, "roundtrip mismatch for {body}");
                layout_free_cell(lc, 0);
            }
        }
    }
}
