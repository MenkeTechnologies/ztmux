//! ztmux-original: render tmux's server-drawn UI surfaces with ratatui.
//!
//! tmux paints its interactive surfaces (menus, clock-mode, display-panes) by
//! the *server* writing cells into a screen or the tty, not by a program that
//! owns a terminal. ratatui renders into a [`Buffer`] of styled cells. So this
//! is a small **backend bridge**: lay a ratatui widget tree into a `Buffer`,
//! then translate every `Cell` into a tmux `grid_cell` and emit it via either
//! `screen_write_cell` ([`blit`], for overlay/mode screens) or `tty_cell`
//! ([`blit_tty`], for direct-to-tty overlays like display-panes).
//!
//! Surfaces bridged so far: overlay menus ([`draw`]), clock-mode
//! ([`draw_clock`]), display-panes ([`draw_pane_number`]).
//!
//! Opt-in via `ZTMUX_RATATUI` (alias `ZTMUX_RATATUI_MENU`) so the default path
//! and the byte-for-byte parity suite are untouched.
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, List, ListItem, ListState, StatefulWidget, Widget};

use crate::*;

/// True if the ratatui UI renderer is enabled for this server (`ZTMUX_RATATUI`;
/// `ZTMUX_RATATUI_MENU` kept as an alias for the earlier menu-only flag).
pub(crate) fn enabled() -> bool {
    std::env::var_os("ZTMUX_RATATUI").is_some() || std::env::var_os("ZTMUX_RATATUI_MENU").is_some()
}

/// 3x5 block-font rows for the clock glyphs (`0`-`9`, `:`, space, AM/PM `A P M`).
fn big_digit(c: char) -> [&'static str; 5] {
    match c {
        '0' => ["███", "█ █", "█ █", "█ █", "███"],
        '1' => ["  █", "  █", "  █", "  █", "  █"],
        '2' => ["███", "  █", "███", "█  ", "███"],
        '3' => ["███", "  █", "███", "  █", "███"],
        '4' => ["█ █", "█ █", "███", "  █", "  █"],
        '5' => ["███", "█  ", "███", "  █", "███"],
        '6' => ["███", "█  ", "███", "█ █", "███"],
        '7' => ["███", "  █", "  █", "  █", "  █"],
        '8' => ["███", "█ █", "███", "█ █", "███"],
        '9' => ["███", "█ █", "███", "  █", "███"],
        ':' => ["   ", " █ ", "   ", " █ ", "   "],
        'A' => ["███", "█ █", "███", "█ █", "█ █"],
        'P' => ["███", "█ █", "███", "█  ", "█  "],
        'M' => ["█ █", "███", "█ █", "█ █", "█ █"],
        _ => ["   ", "   ", "   ", "   ", "   "],
    }
}

/// Render clock-mode (`window_clock`) with ratatui: a rounded `Block` framed
/// with today's date, and a big-digit HH:MM (3x5 block font) centred in it.
pub(crate) unsafe fn draw_clock(wp: *mut window_pane, s: *mut screen) {
    unsafe {
        let w = crate::screen_size_x(s) as u16;
        let h = crate::screen_size_y(s) as u16;
        if w < 4 || h < 3 {
            return;
        }

        let style_24h =
            crate::options_::options_get_number_((*(*wp).window).options, "clock-mode-style") != 0;
        let mut t = libc::time(std::ptr::null_mut());
        let tm = libc::localtime(&raw mut t);
        let read = |fmt: &std::ffi::CStr| -> String {
            let mut b = [0u8; 64];
            libc::strftime(b.as_mut_ptr(), b.len(), fmt.as_ptr().cast(), tm);
            std::ffi::CStr::from_ptr(b.as_ptr().cast())
                .to_string_lossy()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        };
        let time_str = read(if style_24h { c"%H:%M" } else { c"%l:%M %p" });
        let date = read(c"%a %d %b %Y");

        let area = Rect::new(0, 0, w, h);
        let mut buf = Buffer::empty(area);

        Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Line::from(format!(" {date} ")).centered())
            .render(area, &mut buf);

        // Centre the big-digit time inside the border.
        let glyph_w = 4u16; // 3 wide + 1 gap
        let total_w = time_str.chars().count() as u16 * glyph_w;
        let inner_w = w.saturating_sub(2);
        let x0 = area.x + 1 + inner_w.saturating_sub(total_w) / 2;
        let y0 = area.y + h.saturating_sub(5) / 2;
        for (gi, ch) in time_str.chars().enumerate() {
            let rows = big_digit(ch.to_ascii_uppercase());
            for (ry, row) in rows.iter().enumerate() {
                for (rx, cc) in row.chars().enumerate() {
                    if cc == ' ' {
                        continue;
                    }
                    let px = x0 + gi as u16 * glyph_w + rx as u16;
                    let py = y0 + ry as u16;
                    if px < area.right().saturating_sub(1) && py < area.bottom().saturating_sub(1) {
                        buf[(px, py)].set_symbol("█").set_fg(Color::Green);
                    }
                }
            }
        }

        let mut ctx = std::mem::MaybeUninit::<screen_write_ctx>::uninit();
        screen_write::screen_write_start(ctx.as_mut_ptr(), s);
        screen_write::screen_write_clearscreen(ctx.as_mut_ptr(), 8);
        blit(ctx.as_mut_ptr(), &buf);
        screen_write::screen_write_stop(ctx.as_mut_ptr());
    }
}

/// Map a ratatui [`Color`] to a tmux colour int (`grid_cell.fg`/`.bg`).
/// `None`/`Reset` -> 8, tmux's "default" sentinel.
fn map_color(c: Option<Color>) -> i32 {
    match c {
        None | Some(Color::Reset) => 8,
        Some(Color::Black) => 0,
        Some(Color::Red) => 1,
        Some(Color::Green) => 2,
        Some(Color::Yellow) => 3,
        Some(Color::Blue) => 4,
        Some(Color::Magenta) => 5,
        Some(Color::Cyan) => 6,
        Some(Color::Gray) => 7,
        Some(Color::DarkGray) => 90,
        Some(Color::LightRed) => 91,
        Some(Color::LightGreen) => 92,
        Some(Color::LightYellow) => 93,
        Some(Color::LightBlue) => 94,
        Some(Color::LightMagenta) => 95,
        Some(Color::LightCyan) => 96,
        Some(Color::White) => 97,
        Some(Color::Rgb(r, g, b)) => crate::colour::colour_join_rgb(r, g, b),
        Some(Color::Indexed(n)) => {
            let n = n as i32;
            if n < 8 {
                n
            } else if n < 16 {
                90 + (n - 8)
            } else {
                n | 0x0100_0000 // COLOUR_FLAG_256
            }
        }
    }
}

/// Map ratatui [`Modifier`] bits to tmux `grid_attr` flags.
fn map_modifier(m: Modifier) -> grid_attr {
    let mut a = grid_attr::empty();
    if m.contains(Modifier::BOLD) {
        a |= grid_attr::GRID_ATTR_BRIGHT;
    }
    if m.contains(Modifier::DIM) {
        a |= grid_attr::GRID_ATTR_DIM;
    }
    if m.contains(Modifier::ITALIC) {
        a |= grid_attr::GRID_ATTR_ITALICS;
    }
    if m.contains(Modifier::UNDERLINED) {
        a |= grid_attr::GRID_ATTR_UNDERSCORE;
    }
    if m.intersects(Modifier::SLOW_BLINK | Modifier::RAPID_BLINK) {
        a |= grid_attr::GRID_ATTR_BLINK;
    }
    if m.contains(Modifier::REVERSED) {
        a |= grid_attr::GRID_ATTR_REVERSE;
    }
    if m.contains(Modifier::HIDDEN) {
        a |= grid_attr::GRID_ATTR_HIDDEN;
    }
    if m.contains(Modifier::CROSSED_OUT) {
        a |= grid_attr::GRID_ATTR_STRIKETHROUGH;
    }
    a
}

/// Copy a one-grapheme ratatui symbol into a tmux `utf8_data` cell.
unsafe fn set_char(gc: *mut grid_cell, sym: &str) {
    unsafe {
        let bytes = sym.as_bytes();
        let n = bytes.len().min(UTF8_SIZE);
        if n == 0 {
            (*gc).data.data[0] = b' ';
            (*gc).data.have = 1;
            (*gc).data.size = 1;
            (*gc).data.width = 1;
            return;
        }
        let dst = (&raw mut (*gc).data.data).cast::<u8>();
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, n);
        (*gc).data.have = n as u8;
        (*gc).data.size = n as u8;
        // Menu cells are single-width (box glyphs + text); ratatui guarantees one
        // grapheme per cell, so width 1 is correct for everything we draw here.
        (*gc).data.width = 1;
    }
}

/// Blit a rendered ratatui [`Buffer`] into the overlay screen via `screen_write`.
unsafe fn blit(ctx: *mut screen_write_ctx, buf: &Buffer) {
    unsafe {
        let area = buf.area;
        for y in 0..area.height {
            for x in 0..area.width {
                let cell = &buf[(x, y)];
                let st = cell.style();

                let mut gc = std::mem::MaybeUninit::<grid_cell>::uninit();
                memcpy__(gc.as_mut_ptr(), &raw const GRID_DEFAULT_CELL);
                let gc = gc.as_mut_ptr();

                set_char(gc, cell.symbol());
                (*gc).fg = map_color(st.fg);
                (*gc).bg = map_color(st.bg);
                (*gc).attr = map_modifier(st.add_modifier);

                screen_write::screen_write_cursormove(ctx, x as i32, y as i32, 0);
                screen_write::screen_write_cell(ctx, gc);
            }
        }
    }
}

/// Strip tmux `#[...]` style markup from a menu label (the ratatui path styles
/// items itself; we only want the visible text).
fn strip_markup(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut chars = name.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '#' && chars.peek() == Some(&'[') {
            // consume until matching ']'
            chars.next();
            for d in chars.by_ref() {
                if d == ']' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Render `md`'s menu into the overlay screen `md.s` with ratatui: a rounded
/// bordered `Block` titled with the menu title, a `List` of items with the
/// current `choice` reverse-highlighted, and separators drawn as rule lines.
pub(crate) unsafe fn draw(md: *mut menu_data, ctx: *mut screen_write_ctx) {
    unsafe {
        let s = &raw mut (*md).s;
        let w = crate::screen_size_x(s) as u16;
        let h = crate::screen_size_y(s) as u16;
        if w == 0 || h == 0 {
            return;
        }
        let area = Rect::new(0, 0, w, h);
        let mut buf = Buffer::empty(area);

        let menu = (*md).menu;
        let inner_w = w.saturating_sub(2) as usize;

        let mut items: Vec<ListItem> = Vec::with_capacity((*menu).items.len());
        for it in &(*menu).items {
            let stripped = strip_markup(&it.name);
            let name = stripped.trim();
            if name.is_empty() {
                // Empty name -> separator rule line.
                items.push(
                    ListItem::new(Line::from("─".repeat(inner_w)))
                        .style(Style::default().fg(Color::DarkGray)),
                );
            } else if let Some(rest) = name.strip_prefix('-') {
                // A leading '-' marks a disabled item (dimmed, not selectable).
                items.push(
                    ListItem::new(Line::from(format!(" {}", rest.trim_start())))
                        .style(Style::default().add_modifier(Modifier::DIM)),
                );
            } else {
                items.push(ListItem::new(Line::from(format!(" {name}"))));
            }
        }

        // Strip the tmux style markup (e.g. `#[align=centre]`) from the title;
        // ratatui centres it itself via `.centered()`.
        let title = strip_markup(&(*menu).title);
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Line::from(format!(" {title} ")).centered());

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let mut state = ListState::default();
        if (*md).choice >= 0 {
            state.select(Some((*md).choice as usize));
        }

        StatefulWidget::render(list, area, &mut buf, &mut state);
        blit(ctx, &buf);
    }
}

/// Blit a rendered ratatui [`Buffer`] straight to the tty at (`px`,`py`) - used
/// by overlays that draw to the terminal directly (display-panes) rather than
/// into a screen buffer.
unsafe fn blit_tty(tty: *mut tty, buf: &Buffer, px: u32, py: u32) {
    unsafe {
        let area = buf.area;
        for y in 0..area.height {
            for x in 0..area.width {
                let cell = &buf[(x, y)];
                let st = cell.style();

                let mut gc = std::mem::MaybeUninit::<grid_cell>::uninit();
                memcpy__(gc.as_mut_ptr(), &raw const GRID_DEFAULT_CELL);
                let gc = gc.as_mut_ptr();
                set_char(gc, cell.symbol());
                (*gc).fg = map_color(st.fg);
                (*gc).bg = map_color(st.bg);
                (*gc).attr = map_modifier(st.add_modifier);

                tty_cursor(tty, px + x as u32, py + y as u32);
                tty_cell(
                    tty,
                    gc,
                    &raw const GRID_DEFAULT_CELL,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                );
            }
        }
    }
}

/// Render one pane's display-panes (`prefix+q`) label with ratatui: a rounded
/// box tinted by active/inactive, titled with the pane size, wrapping the pane
/// index in the big-digit block font. Blitted straight to the tty at the pane.
pub(crate) unsafe fn draw_pane_number(ctx: *mut screen_redraw_ctx, wp: *mut window_pane) {
    unsafe {
        let c = (*ctx).c;
        let tty = &raw mut (*c).tty;

        // Prototype: draw for panes fully inside the viewport; skip partials.
        if (*wp).xoff < (*ctx).ox
            || (*wp).yoff < (*ctx).oy
            || (*wp).xoff + (*wp).sx > (*ctx).ox + (*ctx).sx
            || (*wp).yoff + (*wp).sy > (*ctx).oy + (*ctx).sy
        {
            return;
        }

        let mut idx: u32 = 0;
        window_pane_index(wp, &raw mut idx);
        let label = format!("{idx}");
        let digits = label.chars().count() as u16;

        let size = format!("{}x{}", (*wp).sx, (*wp).sy);
        let need_w = (digits * 4 + 1).max(size.len() as u16 + 2);
        let boxw = need_w.min((*wp).sx as u16);
        let boxh = 7u16.min((*wp).sy as u16);
        if boxw < 5 || boxh < 5 {
            return;
        }

        let area = Rect::new(0, 0, boxw, boxh);
        let mut buf = Buffer::empty(area);

        let active = (*(*wp).window).active == wp;
        let colour = if active { Color::Green } else { Color::Blue };
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(colour))
            .title(Line::from(format!(" {size} ")).centered())
            .render(area, &mut buf);

        let total_w = digits * 4;
        let x0 = 1 + (boxw.saturating_sub(2)).saturating_sub(total_w) / 2;
        let y0 = 1 + (boxh.saturating_sub(2)).saturating_sub(5) / 2;
        for (gi, ch) in label.chars().enumerate() {
            for (ry, row) in big_digit(ch).iter().enumerate() {
                for (rx, cc) in row.chars().enumerate() {
                    if cc == ' ' {
                        continue;
                    }
                    let bx = x0 + gi as u16 * 4 + rx as u16;
                    let by = y0 + ry as u16;
                    if bx < boxw - 1 && by < boxh - 1 {
                        buf[(bx, by)].set_symbol("█").set_fg(colour);
                    }
                }
            }
        }

        let yoff = (*wp).yoff - (*ctx).oy
            + if (*ctx).statustop != 0 {
                (*ctx).statuslines
            } else {
                0
            };
        let xoff = (*wp).xoff - (*ctx).ox;
        let px = xoff + ((*wp).sx.saturating_sub(boxw as u32)) / 2;
        let py = yoff + ((*wp).sy.saturating_sub(boxh as u32)) / 2;
        blit_tty(tty, &buf, px, py);
    }
}

/// Read the client prompt buffer (`utf8_data` array, terminated by a zero-size
/// entry) into per-grapheme (text, cell-width) pairs.
unsafe fn prompt_graphemes(buf: *const utf8_data) -> Vec<(String, u16)> {
    unsafe {
        let mut out = Vec::new();
        if buf.is_null() {
            return out;
        }
        let mut i = 0isize;
        loop {
            let ud = buf.offset(i);
            let size = (*ud).size as usize;
            if size == 0 {
                break;
            }
            let bytes = std::slice::from_raw_parts((&raw const (*ud).data).cast::<u8>(), size);
            let text = String::from_utf8_lossy(bytes).into_owned();
            out.push((text, ((*ud).width as u16).max(1)));
            i += 1;
        }
        out
    }
}

/// Register the floating ratatui command-prompt overlay (from
/// `status_prompt_set`): a rounded box floated in the upper area so the status
/// bar stays visible, with the prompt glyph, the input, a block cursor, and a
/// live tab-completion popup.
/// Sentinel stored as the overlay's data so we can recognise our own prompt
/// overlay reliably (fn-pointer identity isn't guaranteed unique).
static PROMPT_OVERLAY_MARK: u8 = 0;

fn prompt_mark() -> *mut core::ffi::c_void {
    (&raw const PROMPT_OVERLAY_MARK).cast_mut().cast()
}

pub(crate) unsafe fn set_prompt_overlay(c: *mut client) {
    unsafe {
        server_client_set_overlay(
            c,
            0,
            Some(prompt_check_cb),
            None,
            Some(prompt_overlay_draw),
            Some(prompt_overlay_key),
            None,
            None,
            prompt_mark(),
        );
    }
}

/// Tear down the prompt overlay if it is ours (from `status_prompt_clear`).
pub(crate) unsafe fn clear_prompt_overlay(c: *mut client) {
    unsafe {
        if (*c).overlay_data == prompt_mark() {
            server_client_clear_overlay(c);
        }
    }
}

/// Geometry + content of the floating prompt box, computed fresh from the
/// current prompt state so the draw and the pane-clipping check callback always
/// agree - no one-frame lag that would re-ghost a shrinking box.
struct PromptLayout {
    px: u16,
    py: u16,
    w: u16,
    h: u16,
    plabel: String,
    input: Vec<(String, u16)>,
    cursor_i: usize,
    cands: Vec<String>,
}

unsafe fn prompt_layout(c: *mut client) -> Option<PromptLayout> {
    unsafe {
        if (*c).prompt_string.is_null() {
            return None;
        }
        let sx = (*c).tty.sx as u16;
        let sy = (*c).tty.sy as u16;
        if sx < 12 || sy < 6 {
            return None;
        }

        let prompt = strip_markup(crate::cstr_to_str((*c).prompt_string));
        let input = prompt_graphemes((*c).prompt_buffer);
        let cursor_i = (*c).prompt_index;
        let input_w: u16 = input.iter().map(|(_, w)| *w).sum();
        let plabel = format!("{prompt} ");
        let plabel_w = plabel.chars().count() as u16;

        let max_rows = 8u16;
        let cands = prompt_completions(c);

        // Anchor the box TOP a quarter of the way up from the bottom so the
        // input row stays put, and let the completion list grow DOWNWARD from it
        // (adding/removing candidates never shifts the input or cursor).
        let py = sy.saturating_sub(sy / 4).saturating_sub(1);
        let space_below = sy.saturating_sub(py + 3); // rows for completions
        let n_rows = (cands.len() as u16).min(max_rows).min(space_below);
        let widest = cands
            .iter()
            .take(n_rows as usize)
            .map(|s| s.chars().count())
            .max()
            .unwrap_or(0) as u16;

        let inner = (plabel_w + input_w + 1)
            .max(widest + 2)
            .max(36)
            .min(sx.saturating_sub(4));
        let w = inner + 2;
        let h = 3 + n_rows;
        let px = sx.saturating_sub(w) / 2;

        Some(PromptLayout {
            px,
            py,
            w,
            h,
            plabel,
            input,
            cursor_i,
            cands,
        })
    }
}

/// Overlay check callback: report the box rect as covered so pane drawing is
/// clipped around it (live panes never overwrite the box - no freeze needed).
unsafe fn prompt_check_cb(
    c: *mut client,
    _data: *mut core::ffi::c_void,
    px: u32,
    py: u32,
    nx: u32,
    r: *mut overlay_ranges,
) {
    unsafe {
        if let Some(l) = prompt_layout(c) {
            server_client_overlay_range(
                l.px as u32,
                l.py as u32,
                l.w as u32,
                l.h as u32,
                px,
                py,
                nx,
                r,
            );
        } else {
            (*r).px[0] = px;
            (*r).nx[0] = nx;
            (*r).px[1] = 0;
            (*r).nx[1] = 0;
            (*r).px[2] = 0;
            (*r).nx[2] = 0;
        }
    }
}

/// Overlay key callback. Keys go to the normal prompt handler (which clears the
/// prompt - and hence this overlay - itself on Enter/Escape). A mouse *press*
/// outside the box dismisses the prompt (click-away); other mouse events are
/// swallowed so they don't leak to the pane underneath.
unsafe fn prompt_overlay_key(
    c: *mut client,
    _data: *mut core::ffi::c_void,
    event: *mut key_event,
) -> i32 {
    unsafe {
        let key = (*event).key;
        if KEYC_IS_MOUSE(key) {
            let m = &raw mut (*event).m;
            let inside = prompt_layout(c).is_some_and(|l| {
                (*m).x >= l.px as u32
                    && (*m).x < (l.px + l.w) as u32
                    && (*m).y >= l.py as u32
                    && (*m).y < (l.py + l.h) as u32
            });
            // A button press (not release/drag/wheel) outside the box = click-away.
            if !inside && !MOUSE_RELEASE((*m).b) && !MOUSE_WHEEL((*m).b) && !MOUSE_DRAG((*m).b) {
                status_prompt_key(c, b'\x1b' as key_code); // Escape -> cancel
            }
            return 0;
        }
        status_prompt_key(c, key);
        // Repaint the window (panes behind the box, so a shrinking completion
        // list doesn't ghost) plus the overlay itself, after each keystroke.
        (*c).flags |= client_flag::REDRAWWINDOW | client_flag::REDRAWOVERLAY;
        0
    }
}

/// Completion candidates for the word currently under the cursor.
unsafe fn prompt_completions(c: *mut client) -> Vec<String> {
    unsafe {
        let input = prompt_graphemes((*c).prompt_buffer);
        let cursor_i = (*c).prompt_index;
        let before: String = input[..cursor_i.min(input.len())]
            .iter()
            .map(|(t, _)| t.as_str())
            .collect();
        let at_start = !before.trim_end().contains(char::is_whitespace);
        let word = before.rsplit(char::is_whitespace).next().unwrap_or("");
        if word.is_empty() {
            return Vec::new();
        }
        let Ok(cword) = std::ffi::CString::new(word) else {
            return Vec::new();
        };
        crate::status::status_prompt_complete_list(cword.as_ptr().cast(), at_start as i32)
    }
}

/// Draw the floating command-prompt overlay onto the tty.
unsafe fn prompt_overlay_draw(
    c: *mut client,
    _data: *mut core::ffi::c_void,
    _rctx: *mut screen_redraw_ctx,
) {
    unsafe {
        let Some(l) = prompt_layout(c) else {
            return;
        };
        let tty = &raw mut (*c).tty;
        let boxw = l.w;
        let boxh = l.h;
        let plabel = l.plabel;
        let input = l.input;
        let cursor_i = l.cursor_i;
        let cands = l.cands;
        let n_rows = boxh - 3;

        let area = Rect::new(0, 0, boxw, boxh);
        let mut buf = Buffer::empty(area);
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .title(Line::from(" command ").centered())
            .render(area, &mut buf);

        // Input row.
        let row = 1u16;
        let mut col = 1u16;
        for ch in plabel.chars() {
            if col >= boxw - 1 {
                break;
            }
            buf[(col, row)].set_char(ch).set_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );
            col += 1;
        }
        let start_col = col;
        let avail = (boxw - 1).saturating_sub(start_col);
        let cursor_col: u16 = input[..cursor_i.min(input.len())]
            .iter()
            .map(|(_, w)| *w)
            .sum();
        let off = if cursor_col >= avail {
            cursor_col - avail + 1
        } else {
            0
        };
        let mut x = 0u16;
        for (gi, (text, w)) in input.iter().enumerate() {
            if x >= off {
                let cx = start_col + (x - off);
                if cx < boxw - 1 {
                    let st = if gi == cursor_i {
                        Style::default().add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    };
                    buf[(cx, row)].set_symbol(text).set_style(st);
                }
            }
            x += *w;
        }
        if cursor_i >= input.len() {
            let cx = start_col + x.saturating_sub(off);
            if cx < boxw - 1 {
                buf[(cx, row)]
                    .set_symbol(" ")
                    .set_style(Style::default().add_modifier(Modifier::REVERSED));
            }
        }

        // Completion rows.
        for (i, cand) in cands.iter().take(n_rows as usize).enumerate() {
            let ry = 2 + i as u16;
            let mut cx = 2u16;
            for ch in cand.chars() {
                if cx >= boxw - 1 {
                    break;
                }
                buf[(cx, ry)]
                    .set_char(ch)
                    .set_style(Style::default().fg(Color::Gray));
                cx += 1;
            }
        }

        blit_tty(tty, &buf, l.px as u32, l.py as u32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_mapping_covers_the_palette() {
        assert_eq!(map_color(None), 8); // default sentinel
        assert_eq!(map_color(Some(Color::Reset)), 8);
        assert_eq!(map_color(Some(Color::Red)), 1);
        assert_eq!(map_color(Some(Color::White)), 97); // bright white
        assert_eq!(map_color(Some(Color::DarkGray)), 90); // bright black
        assert_eq!(map_color(Some(Color::Indexed(5))), 5); // base 16 -> 0..7
        assert_eq!(map_color(Some(Color::Indexed(12))), 94); // base 16 -> 90..97
        assert_eq!(map_color(Some(Color::Indexed(200))), 200 | 0x0100_0000); // 256
        assert_eq!(
            map_color(Some(Color::Rgb(0x11, 0x22, 0x33))),
            crate::colour::colour_join_rgb(0x11, 0x22, 0x33)
        );
    }

    #[test]
    fn modifier_mapping() {
        assert!(map_modifier(Modifier::REVERSED).contains(grid_attr::GRID_ATTR_REVERSE));
        assert!(map_modifier(Modifier::BOLD).contains(grid_attr::GRID_ATTR_BRIGHT));
        let both = map_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        assert!(both.contains(grid_attr::GRID_ATTR_BRIGHT));
        assert!(both.contains(grid_attr::GRID_ATTR_UNDERSCORE));
        assert!(map_modifier(Modifier::empty()).is_empty());
    }

    #[test]
    fn strip_markup_removes_style_runs() {
        assert_eq!(strip_markup("#[align=centre]Kill"), "Kill");
        assert_eq!(strip_markup("Copy #[underscore]word"), "Copy word");
        assert_eq!(strip_markup("plain"), "plain");
    }

    // The widget path must produce a bordered box with the item text - proves
    // the ratatui side renders before it's ever blitted into an overlay screen.
    #[test]
    fn renders_a_bordered_menu_into_a_buffer() {
        let area = Rect::new(0, 0, 20, 5);
        let mut buf = Buffer::empty(area);
        let items = vec![ListItem::new(Line::from(" Horizontal Split"))];
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Line::from(" pane ").centered());
        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        let mut state = ListState::default();
        state.select(Some(0));
        StatefulWidget::render(list, area, &mut buf, &mut state);

        // Rounded top-left corner from the Block border.
        assert_eq!(buf[(0, 0)].symbol(), "╭");
        // The item text landed inside the box.
        let row1: String = (1..19).map(|x| buf[(x, 1)].symbol()).collect();
        assert!(row1.contains("Horizontal Split"), "got row: {row1:?}");
    }
}
