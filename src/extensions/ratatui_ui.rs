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
//! On by default; opt OUT with the global option `@ztmux-ratatui off` for a
//! classic plain-tmux server (see [`enabled`]).
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, List, ListItem, ListState, StatefulWidget, Widget};

use crate::*;

/// True if the ratatui UI renderer is enabled for this server. It is ON by
/// default (the `:` command palette, ratatui menus, clock, hint bar); opt OUT
/// with the global option `@ztmux-ratatui off` (or `0`/`false`/`no`) for a
/// classic plain-tmux server. Read live from the option table (no env, no cache)
/// so `set -g @ztmux-ratatui off` takes effect on the next redraw — the reader
/// is allocation-free because `enabled()` is on hot paths (per border cell, per
/// pane redraw).
///
/// Note this only turns on the ratatui *chrome*; the more invasive zellij look
/// (pane frames, inset, stacks) is a separate opt-in via `@ztmux-zellij-mode`.
pub(crate) fn enabled() -> bool {
    unsafe { global_user_flag("@ztmux-ratatui", true) }
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

/// Convert a tmux colour integer (as stored in `grid_cell.fg`/`.bg`) into a
/// ratatui [`Color`]. Returns `None` for the terminal default (`8`/`-1`) so the
/// caller can leave that channel unset. The inverse of [`map_color`].
fn tmux_colour_to_ratatui(c: i32) -> Option<Color> {
    const COLOUR_FLAG_256: i32 = 0x0100_0000;
    const COLOUR_FLAG_RGB: i32 = 0x0200_0000;
    if c == -1 || c == 8 {
        return None; // terminal default
    }
    if c & COLOUR_FLAG_RGB != 0 {
        let (r, g, b) = crate::colour::colour_split_rgb(c);
        return Some(Color::Rgb(r, g, b));
    }
    if c & COLOUR_FLAG_256 != 0 {
        return Some(Color::Indexed((c & 0xff) as u8));
    }
    match c {
        0..=7 => Some(Color::Indexed(c as u8)),
        90..=97 => Some(Color::Indexed((c - 90 + 8) as u8)), // aixterm bright
        100..=107 => Some(Color::Indexed((c - 100 + 8) as u8)),
        _ => None,
    }
}

/// Resolve the client's `message-style` option into `(fg, bg)` ratatui colours,
/// honouring a `reverse` attribute (swaps fg/bg). Both the floating status
/// message and the `:` command prompt are drawn with `message-style` in tmux, so
/// both overlays tint themselves from this - a user's `message-style bg=purple`
/// recolours the boxes instead of them being hardcoded. Falls back to `(None,
/// None)` when there is no session.
unsafe fn message_style_colors(c: *mut client) -> (Option<Color>, Option<Color>) {
    unsafe {
        let s = (*c).session;
        if s.is_null() {
            return (None, None);
        }
        let ft = format_create_defaults(null_mut(), c, null_mut(), null_mut(), null_mut());
        let mut gc = std::mem::MaybeUninit::<grid_cell>::uninit();
        style_apply(gc.as_mut_ptr(), (*s).options, c!("message-style"), ft);
        let gc = gc.assume_init();
        format_free(ft);

        let (mut fg, mut bg) = (gc.fg, gc.bg);
        if gc.attr.contains(grid_attr::GRID_ATTR_REVERSE) {
            std::mem::swap(&mut fg, &mut bg);
        }
        (tmux_colour_to_ratatui(fg), tmux_colour_to_ratatui(bg))
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

/// Highlighted completion candidate in the palette (-1 == none). Navigated with
/// Up/Down, applied with Tab. Reset when the prompt opens or the input changes.
static PROMPT_SEL: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

fn prompt_mark() -> *mut core::ffi::c_void {
    (&raw const PROMPT_OVERLAY_MARK).cast_mut().cast()
}

pub(crate) unsafe fn set_prompt_overlay(c: *mut client) {
    unsafe {
        PROMPT_SEL.store(-1, std::sync::atomic::Ordering::Relaxed);
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

// ---------------------------------------------------------------------------
// Floating status *message* (errors, notices). Classic tmux paints these over
// the status row, hiding it. ztmux instead floats the message as a rounded
// overlay box - the same treatment as the `:` command prompt - so the status
// bar stays visible underneath. It carries a check callback (panes clip around
// it, no freeze) but NO key callback: the message-key handling in
// `server_client_handle_key` already clears the message (and hence this
// overlay) on the next key, and the `display-time` timer auto-dismisses it
// otherwise - exactly the transient toast behaviour. The draw reads
// `c.message_string` live each frame, so no owned data is needed; the sentinel
// mark just lets `clear_message_overlay` recognise our own overlay.
// ---------------------------------------------------------------------------

static MESSAGE_OVERLAY_MARK: u8 = 0;

fn message_mark() -> *mut core::ffi::c_void {
    (&raw const MESSAGE_OVERLAY_MARK).cast_mut().cast()
}

/// Float the status message as an overlay box (from `status_message_set`).
pub(crate) unsafe fn set_message_overlay(c: *mut client) {
    unsafe {
        server_client_set_overlay(
            c,
            0,
            Some(message_check_cb),
            None,
            Some(message_overlay_draw),
            None,
            None,
            None,
            message_mark(),
        );
    }
}

/// Tear down the message overlay if it is ours (from `status_message_clear`).
pub(crate) unsafe fn clear_message_overlay(c: *mut client) {
    unsafe {
        if (*c).overlay_data == message_mark() {
            server_client_clear_overlay(c);
        }
    }
}

/// Geometry + wrapped lines of the floating message box, computed fresh from the
/// current `message_string` so the draw and the pane-clipping check callback
/// always agree.
struct MessageLayout {
    px: u16,
    py: u16,
    w: u16,
    h: u16,
    lines: Vec<String>,
}

unsafe fn message_layout(c: *mut client) -> Option<MessageLayout> {
    unsafe {
        if (*c).message_string.is_null() {
            return None;
        }
        let sx = (*c).tty.sx as u16;
        let sy = (*c).tty.sy as u16;
        if sx < 12 || sy < 4 {
            return None;
        }

        // Strip status markup (`#[...]`) - the overlay renders plain text - and
        // fold any embedded newlines / tabs into spaces so wrapping is clean.
        let text = strip_markup(crate::cstr_to_str((*c).message_string));
        let text = text.replace(['\n', '\r', '\t'], " ");
        let text = text.trim();
        if text.is_empty() {
            return None;
        }

        // Wrap to an inner width bounded by the terminal, word-first with a
        // hard break for over-long tokens, at most 6 rows.
        let inner = (sx.saturating_sub(6) as usize).clamp(8, 100);
        let mut lines: Vec<String> = Vec::new();
        let mut cur = String::new();
        for word in text.split(' ').filter(|w| !w.is_empty()) {
            if word.chars().count() > inner {
                if !cur.is_empty() {
                    lines.push(std::mem::take(&mut cur));
                }
                let mut chunk = String::new();
                for ch in word.chars() {
                    if chunk.chars().count() >= inner {
                        lines.push(std::mem::take(&mut chunk));
                    }
                    chunk.push(ch);
                }
                cur = chunk;
                continue;
            }
            let extra = if cur.is_empty() { 0 } else { 1 };
            if !cur.is_empty() && cur.chars().count() + extra + word.chars().count() > inner {
                lines.push(std::mem::take(&mut cur));
            }
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(word);
        }
        if !cur.is_empty() {
            lines.push(cur);
        }
        if lines.len() > 6 {
            lines.truncate(6);
            if let Some(last) = lines.last_mut() {
                let mut t: String = last.chars().take(inner.saturating_sub(1)).collect();
                t.push('\u{2026}');
                *last = t;
            }
        }

        let widest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
        let title_w = " message ".chars().count() as u16;
        let w = (widest + 4).max(title_w + 2).min(sx.saturating_sub(2));
        let h = lines.len() as u16 + 2;

        // Anchor a quarter of the way up from the bottom - the same height the
        // `:` command-prompt box floats at - horizontally centred, but never
        // let a tall box run off the bottom edge.
        let py = sy
            .saturating_sub(sy / 4)
            .saturating_sub(1)
            .min(sy.saturating_sub(h));
        let px = sx.saturating_sub(w) / 2;

        Some(MessageLayout {
            px,
            py,
            w,
            h,
            lines,
        })
    }
}

/// Overlay check callback: report the box rect as covered so live panes are
/// clipped around it (no freeze - the window keeps drawing underneath).
unsafe fn message_check_cb(
    c: *mut client,
    _data: *mut core::ffi::c_void,
    px: u32,
    py: u32,
    nx: u32,
    r: *mut overlay_ranges,
) {
    unsafe {
        if let Some(l) = message_layout(c) {
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

/// Draw the floating status-message overlay onto the tty: a rounded amber box
/// titled " message " with the wrapped message text inside.
unsafe fn message_overlay_draw(
    c: *mut client,
    _data: *mut core::ffi::c_void,
    _rctx: *mut screen_redraw_ctx,
) {
    unsafe {
        let Some(l) = message_layout(c) else {
            return;
        };
        let tty = &raw mut (*c).tty;
        let area = Rect::new(0, 0, l.w, l.h);
        let mut buf = Buffer::empty(area);

        // Tint from `message-style` (e.g. the user's `bg=purple`); fall back to
        // amber when the style leaves a channel at the terminal default.
        let (mfg, mbg) = message_style_colors(c);
        let fg = mfg.unwrap_or(Color::Yellow);
        let mut box_style = Style::default().fg(fg);
        if let Some(bg) = mbg {
            box_style = box_style.bg(bg);
        }
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(fg))
            .style(box_style)
            .title(Line::from(" message ").centered())
            .render(area, &mut buf);

        let mut text_style = Style::default().fg(fg).add_modifier(Modifier::BOLD);
        if let Some(bg) = mbg {
            text_style = text_style.bg(bg);
        }
        for (ri, line) in l.lines.iter().enumerate() {
            let ry = 1 + ri as u16;
            let mut cx = 2u16;
            for ch in line.chars() {
                if cx >= l.w - 1 {
                    break;
                }
                buf[(cx, ry)].set_char(ch).set_style(text_style);
                cx += 1;
            }
        }
        blit_tty(tty, &buf, l.px as u32, l.py as u32);
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
        use std::sync::atomic::Ordering::Relaxed;
        let raw = (*event).key;
        if KEYC_IS_MOUSE(raw) {
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

        let key = raw & !KEYC_MASK_FLAGS; // mirror status_prompt_key's masking
        let cands = prompt_completions(c);
        let n = cands.len() as i32;
        let redraw = |c: *mut client| {
            (*c).flags |= client_flag::REDRAWWINDOW | client_flag::REDRAWOVERLAY;
        };

        // Up/Down navigate the completion list (falling through to the normal
        // prompt's history when there is nothing to complete).
        if n > 0 && (key == keyc::KEYC_UP as key_code || key == keyc::KEYC_DOWN as key_code) {
            let cur = PROMPT_SEL.load(Relaxed);
            let next = if key == keyc::KEYC_DOWN as key_code {
                (cur + 1).min(n - 1)
            } else {
                (cur - 1).max(0)
            };
            PROMPT_SEL.store(next, Relaxed);
            redraw(c);
            return 0;
        }

        // Tab applies the highlighted (or first) candidate INLINE - never letting
        // tmux's own completion open its menu at the status line (the bug where
        // Tab jumped to the bottom command box). Always swallowed.
        if key == 0x09 {
            if n > 0 {
                let idx = PROMPT_SEL.load(Relaxed).max(0) as usize;
                if let Some(cand) = cands.get(idx) {
                    crate::status::status_prompt_replace_complete(c, Some(cand));
                }
            }
            PROMPT_SEL.store(-1, Relaxed);
            redraw(c);
            return 0;
        }

        // Everything else (typing, Left/Right cursor, Backspace, Enter to run,
        // Escape to cancel, history) goes to the normal prompt handler; the
        // candidate list is about to change, so drop the highlight.
        PROMPT_SEL.store(-1, Relaxed);
        status_prompt_key(c, raw);
        redraw(c);
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

        // Tint from `message-style` (the same option tmux draws the prompt with)
        // so a user's `bg=purple` recolours the box; fall back to cyan for the
        // "command" identity when the style leaves a channel at the default.
        let (mfg, mbg) = message_style_colors(c);
        let accent = mfg.unwrap_or(Color::Cyan);
        let with_bg = |s: Style| match mbg {
            Some(bg) => s.bg(bg),
            None => s,
        };

        let area = Rect::new(0, 0, boxw, boxh);
        let mut buf = Buffer::empty(area);
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(accent))
            .style(with_bg(Style::default().fg(accent)))
            .title(Line::from(" command ").centered())
            .render(area, &mut buf);

        // Input row.
        let row = 1u16;
        let mut col = 1u16;
        for ch in plabel.chars() {
            if col >= boxw - 1 {
                break;
            }
            buf[(col, row)].set_char(ch).set_style(with_bg(
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ));
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
                        with_bg(Style::default()).add_modifier(Modifier::REVERSED)
                    } else {
                        with_bg(Style::default())
                    };
                    buf[(cx, row)].set_symbol(text).set_style(st);
                }
            }
            x += *w;
        }
        if cursor_i >= input.len() {
            let cx = start_col + x.saturating_sub(off);
            if cx < boxw - 1 {
                buf[(cx, row)].set_symbol(" ").set_style(
                    with_bg(Style::default().fg(accent)).add_modifier(Modifier::REVERSED),
                );
            }
        }

        // Completion rows, with the navigable highlight (Up/Down) reverse-video.
        let sel = PROMPT_SEL.load(std::sync::atomic::Ordering::Relaxed);
        for (i, cand) in cands.iter().take(n_rows as usize).enumerate() {
            let ry = 2 + i as u16;
            let style = if i as i32 == sel {
                with_bg(Style::default().fg(accent))
                    .add_modifier(Modifier::REVERSED | Modifier::BOLD)
            } else {
                with_bg(Style::default().fg(Color::Gray))
            };
            let mut cx = 2u16;
            for ch in cand.chars() {
                if cx >= boxw - 1 {
                    break;
                }
                buf[(cx, ry)].set_char(ch).set_style(style);
                cx += 1;
            }
        }

        blit_tty(tty, &buf, l.px as u32, l.py as u32);
    }
}

// ---------------------------------------------------------------------------
// Keybinding hint bar (zellij-style "which-key"): when the client enters a
// non-root key table (e.g. after the prefix) float a rounded bar near the
// bottom listing the bindings available right now. It carries NO key callback,
// so the very next keypress falls straight through to normal dispatch and the
// generic key path tears the overlay down - exactly the transient which-key
// feel. Driven from `server_client_set_key_table`, the single funnel every key
// table change passes through, so it tracks prefix/custom tables precisely.
// ---------------------------------------------------------------------------

/// The hint overlay owns a heap-allocated [`HintLayout`] as its `data`, computed
/// ONCE when the overlay is set and freed by [`hint_free`]. The check and draw
/// callbacks just read it - enumerating ~100 bindings and packing rows on every
/// check call (the overlay check runs many times per redraw) was what made
/// pressing the prefix lag. We recognise our own overlay by the draw-callback
/// pointer rather than a data sentinel, since `data` now varies per overlay.
unsafe fn hint_is_ours(c: *mut client) -> bool {
    unsafe {
        (*c).overlay_draw
            .is_some_and(|f| std::ptr::fn_addr_eq(f, hint_draw as OverlayDrawFn))
    }
}

type OverlayDrawFn = unsafe fn(*mut client, *mut core::ffi::c_void, *mut screen_redraw_ctx);

/// Free the cached [`HintLayout`] when the overlay is torn down.
unsafe fn hint_free(_c: *mut client, data: *mut core::ffi::c_void) {
    unsafe {
        if !data.is_null() {
            drop(Box::from_raw(data.cast::<HintLayout>()));
        }
    }
}

/// Collect `(key, note)` pairs for the note-bearing, non-mouse bindings of the
/// client's current key table - the same curated set `list-keys -N` shows.
unsafe fn hint_bindings(c: *mut client) -> Vec<(String, String)> {
    unsafe {
        let table = (*c).keytable;
        let mut out = Vec::new();
        if table.is_null() {
            return out;
        }
        let mut bd = key_bindings_first(table);
        while !bd.is_null() {
            let key = (*bd).key;
            let note = (*bd).note;
            // Label: the binding's `-N` note if it has one, else the bound
            // command itself (like `list-keys`), so a key the user rebound
            // WITHOUT a note still shows up instead of vanishing.
            let (label, owned) = if !note.is_null() && *note != 0 {
                (note, false)
            } else if !(*bd).cmdlist.is_null() {
                (cmd_list_print(&*(*bd).cmdlist, 1), true)
            } else {
                (null_mut(), false)
            };
            if !KEYC_IS_MOUSE(key) && !label.is_null() && *label != 0 {
                let ks = key_string_lookup_key(key, 0);
                if !ks.is_null() {
                    // key_string_lookup_key hands back a reused static buffer, so
                    // copy both strings out before the next call clobbers it.
                    let keystr = crate::cstr_to_str(ks).to_string();
                    let mut notestr = crate::cstr_to_str(label).to_string();
                    // A command-derived label can be an arbitrarily long command
                    // line; keep chips compact (notes are already terse).
                    if owned && notestr.chars().count() > 24 {
                        notestr = notestr.chars().take(23).collect::<String>() + "\u{2026}";
                    }
                    out.push((keystr, notestr));
                }
            }
            if owned && !label.is_null() {
                crate::free_(label);
            }
            bd = key_bindings_next(table, bd);
        }
        out
    }
}

/// Geometry + greedily-packed rows of the hint bar, computed fresh each frame so
/// the draw and the pane-clipping check callback always agree.
struct HintLayout {
    px: u16,
    py: u16,
    w: u16,
    h: u16,
    table: String,
    rows: Vec<Vec<(String, String)>>,
}

unsafe fn hint_layout(c: *mut client) -> Option<HintLayout> {
    unsafe {
        if (*c).keytable.is_null() {
            return None;
        }
        let sx = (*c).tty.sx as u16;
        let sy = (*c).tty.sy as u16;
        if sx < 24 || sy < 6 {
            return None;
        }
        let chips = hint_bindings(c);
        if chips.is_empty() {
            return None;
        }
        let table = strip_markup(crate::cstr_to_str((*(*c).keytable).name));

        // Greedy-pack chips ("key note") into rows no wider than the inner box.
        let gap = 2usize;
        let inner = (sx.saturating_sub(6) as usize).min(120);
        let chip_w = |k: &str, n: &str| k.chars().count() + 1 + n.chars().count();
        let mut rows: Vec<Vec<(String, String)>> = Vec::new();
        let mut cur: Vec<(String, String)> = Vec::new();
        let mut curw = 0usize;
        for (k, n) in chips {
            let cw = chip_w(&k, &n);
            if !cur.is_empty() && curw + gap + cw > inner {
                rows.push(std::mem::take(&mut cur));
                curw = cw;
            } else {
                curw += if cur.is_empty() { cw } else { gap + cw };
            }
            cur.push((k, n));
        }
        if !cur.is_empty() {
            rows.push(cur);
        }
        rows.truncate(7);

        let row_w = |r: &Vec<(String, String)>| -> usize {
            r.iter().map(|(k, v)| chip_w(k, v)).sum::<usize>() + gap * r.len().saturating_sub(1)
        };
        let widest = rows.iter().map(row_w).max().unwrap_or(0) as u16;
        let title_w = table.chars().count() as u16 + 4;
        let w = (widest + 4).max(title_w).min(sx.saturating_sub(2));
        let h = rows.len() as u16 + 2;

        // Sit just above a bottom status line (or at the very bottom otherwise).
        let sa = status_at_line(c);
        let bottom = if sa > 0 { sa as u16 } else { sy };
        let py = bottom.saturating_sub(h);
        let px = sx.saturating_sub(w) / 2;

        Some(HintLayout {
            px,
            py,
            w,
            h,
            table,
            rows,
        })
    }
}

/// Overlay check callback: report the bar rect as covered so live panes are
/// clipped around it - no freeze, the window keeps drawing underneath.
unsafe fn hint_check_cb(
    _c: *mut client,
    data: *mut core::ffi::c_void,
    px: u32,
    py: u32,
    nx: u32,
    r: *mut overlay_ranges,
) {
    unsafe {
        if let Some(l) = data.cast::<HintLayout>().as_ref() {
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

/// Draw the hint bar onto the tty: a rounded box titled with the table name,
/// each binding rendered as a highlighted key followed by its note.
unsafe fn hint_draw(c: *mut client, data: *mut core::ffi::c_void, _rctx: *mut screen_redraw_ctx) {
    unsafe {
        let Some(l) = data.cast::<HintLayout>().as_ref() else {
            return;
        };
        let tty = &raw mut (*c).tty;
        let area = Rect::new(0, 0, l.w, l.h);
        let mut buf = Buffer::empty(area);
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Yellow))
            .title(Line::from(format!(" {} ", l.table)).centered())
            .render(area, &mut buf);

        let key_style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
        let note_style = Style::default().fg(Color::Gray);
        for (ri, row) in l.rows.iter().enumerate() {
            let ry = 1 + ri as u16;
            let mut cx = 2u16;
            for (k, n) in row {
                for ch in k.chars() {
                    if cx >= l.w - 1 {
                        break;
                    }
                    buf[(cx, ry)].set_char(ch).set_style(key_style);
                    cx += 1;
                }
                if cx < l.w - 1 {
                    cx += 1; // space between key and its note
                }
                for ch in n.chars() {
                    if cx >= l.w - 1 {
                        break;
                    }
                    buf[(cx, ry)].set_char(ch).set_style(note_style);
                    cx += 1;
                }
                cx += 2; // gap before the next chip
                if cx >= l.w - 1 {
                    break;
                }
            }
        }
        blit_tty(tty, &buf, l.px as u32, l.py as u32);
    }
}

/// True if the hint bar is suppressed. It is OFF by default (opt in with
/// `set -g @ztmux-hint on`); any other value, or unset, leaves it hidden. Checks
/// every global scope so the option is found wherever it lands.
unsafe fn hint_option_off(_c: *mut client) -> bool {
    unsafe {
        match global_user_opt("@ztmux-hint") {
            Some(v) => !matches!(v.trim(), "on" | "1" | "true" | "yes"),
            None => true, // unset -> default OFF
        }
    }
}

/// Show or hide the hint bar to match the client's current key table. Called at
/// the tail of `server_client_set_key_table`.
pub(crate) unsafe fn reconcile_hint(c: *mut client) {
    unsafe {
        if !enabled() || c.is_null() || (*c).session.is_null() {
            return;
        }
        // Runtime opt-out: `set -g @ztmux-hint off` hides just the hint bar and
        // leaves the other ratatui surfaces (menus, clock, palette) untouched.
        if hint_option_off(c) {
            if hint_is_ours(c) {
                server_client_clear_overlay(c);
            }
            return;
        }
        let kt = (*c).keytable;
        let want = !kt.is_null() && !server_client_is_default_key_table(c, kt);
        if want {
            // Claim the overlay slot only if it's free (never clobber a menu,
            // popup or the command prompt). Compute the layout ONCE here and hand
            // it to the overlay as owned data; the check/draw callbacks just read
            // it (recomputing per check call was the prefix-activation lag).
            if (*c).overlay_draw.is_none()
                && let Some(layout) = hint_layout(c)
            {
                let data = Box::into_raw(Box::new(layout)).cast::<core::ffi::c_void>();
                server_client_set_overlay(
                    c,
                    0,
                    Some(hint_check_cb),
                    None,
                    Some(hint_draw),
                    None,
                    Some(hint_free),
                    None,
                    data,
                );
            }
        } else if hint_is_ours(c) {
            server_client_clear_overlay(c);
        }
    }
}

// ---------------------------------------------------------------------------
// ztmux's own rounded ratatui frame for synced / sync-marked panes (a
// zellij-style box), the sole sync/mark indicator - the earlier tmux
// border-recolour has been dropped in favour of this. Only active when the
// ratatui UI is enabled, so the byte-for-byte parity path is untouched.
// ---------------------------------------------------------------------------

/// True if this pane is marked for sync (per-pane `@ztmux_sel` == "1").
unsafe fn pane_marked_for_sync(po: *mut options) -> bool {
    unsafe {
        if crate::options_::options_get_only_const(po, "@ztmux_sel").is_null() {
            return false;
        }
        crate::cstr_to_str(crate::options_::options_get_string_(po, "@ztmux_sel")) == "1"
    }
}

/// The `#{?synchronize-panes}`-style colour for a pane's sync state, or `None`
/// if it has none. Synced red, selected orange, trigger-armed cyan (the same
/// palette as the frames).
unsafe fn pane_state_colour(wp: *mut window_pane) -> Option<i32> {
    unsafe {
        let po = (*wp).options;
        if crate::options_::options_get_number_(po, "synchronize-panes") != 0 {
            Some(196)
        } else if pane_marked_for_sync(po) {
            Some(214)
        } else if (*wp).pipe_fd != -1 {
            Some(51)
        } else {
            None
        }
    }
}

/// Colour a pane's *border* for its sync state. Unlike an overlaid frame, the
/// border is never repainted by the pane's own output, so this is the robust
/// sync indicator in the default (no-frame) mode. Layered on the resolved
/// `pane-border-style`; a no-op unless the ratatui UI is enabled.
pub(crate) unsafe fn apply_sync_border(gc: *mut grid_cell, wp: *mut window_pane) {
    unsafe {
        if !enabled() || wp.is_null() {
            return;
        }
        if let Some(fg) = pane_state_colour(wp) {
            (*gc).fg = fg;
            (*gc).attr |= grid_attr::GRID_ATTR_BRIGHT;
        }
    }
}

/// Read a global user option (`set -g @name ...`) as a string, checking every
/// global scope so it is found wherever `-g` routed it (server/session/window),
/// never fataling on an unset option. `None` if unset everywhere.
unsafe fn global_user_opt(name: &str) -> Option<String> {
    unsafe {
        for oo in [GLOBAL_S_OPTIONS, GLOBAL_OPTIONS, GLOBAL_W_OPTIONS] {
            if !oo.is_null() && !crate::options_::options_get_only_const(oo, name).is_null() {
                return Some(
                    crate::cstr_to_str(crate::options_::options_get_string_(oo, name)).to_string(),
                );
            }
        }
        None
    }
}

/// Read a boolean-ish global user option without allocating (for hot paths). An
/// option that is set and holds a falsy value (`0`/`off`/`false`/`no`) reads
/// false; set to anything else reads true; unset falls back to `default`.
unsafe fn global_user_flag(name: &str, default: bool) -> bool {
    unsafe {
        for oo in [GLOBAL_S_OPTIONS, GLOBAL_OPTIONS, GLOBAL_W_OPTIONS] {
            if !oo.is_null() && !crate::options_::options_get_only_const(oo, name).is_null() {
                let s = crate::cstr_to_str(crate::options_::options_get_string_(oo, name)).trim();
                let off = s.eq_ignore_ascii_case("0")
                    || s.eq_ignore_ascii_case("off")
                    || s.eq_ignore_ascii_case("false")
                    || s.eq_ignore_ascii_case("no");
                return !off;
            }
        }
        default
    }
}

/// Whether "zellij mode" is on: every pane gets a named, inset rounded frame,
/// the active pane's frame is green, sync/select/trigger recolour it, and tmux's
/// own borders are hidden. Opt-in via `@ztmux-zellij-mode` (global), with
/// `@ztmux-pane-names` kept as a back-compat alias; `on`/`1`/`true`/`yes`
/// enables it, default OFF. When off there are no frames (sync shows on the
/// pane border colour instead).
unsafe fn zellij_mode_on() -> bool {
    unsafe {
        let v =
            global_user_opt("@ztmux-zellij-mode").or_else(|| global_user_opt("@ztmux-pane-names"));
        match v {
            Some(v) => matches!(v.trim(), "on" | "1" | "true" | "yes"),
            None => false, // default off
        }
    }
}

/// The 1-cell ring reserved around every pane for its frame in zellij mode, else
/// 0. Read by BOTH `layout_fix_panes` (to shrink the pane's content so a program
/// can never draw on the frame) and the frame draw (to place the ring), so the
/// two always agree.
pub(crate) unsafe fn frame_inset() -> u32 {
    unsafe { u32::from(enabled() && zellij_mode_on()) }
}

/// The name shown in a pane's frame. `@ztmux-pane-name-format` (global) is a
/// tmux format expanded per pane if set (e.g. `#{pane_index}: #{pane_current_command}`);
/// otherwise the pane's title, falling back to `pane N`.
unsafe fn pane_display_name(ctx: *mut screen_redraw_ctx, wp: *mut window_pane) -> String {
    unsafe {
        // Custom format if set, else a clean default of "index: command" (the
        // pane title is usually the tty/host, which reads badly in a frame).
        let fmt = global_user_opt("@ztmux-pane-name-format")
            .filter(|f| !f.trim().is_empty())
            .unwrap_or_else(|| "#{pane_index}: #{pane_current_command}".to_string());
        if let Ok(cfmt) = std::ffi::CString::new(fmt.trim()) {
            let c = (*ctx).c;
            let s = (*c).session;
            let wl = if s.is_null() { null_mut() } else { (*s).curw };
            let ft = format_create_defaults(null_mut(), c, s, wl, wp);
            let out = format_expand(ft, cfmt.as_ptr().cast());
            let name = crate::cstr_to_str(out).trim().to_string();
            crate::free_(out);
            format_free(ft);
            if !name.is_empty() {
                return name;
            }
        }
        let mut idx: u32 = 0;
        window_pane_index(wp, &raw mut idx);
        format!("pane {idx}")
    }
}

/// A zellij-style `SCROLL: off/total` indicator for a collapsed pane's title
/// bar: `total` is the scrollback depth (`history_size`), `off` the current
/// copy-mode scroll offset (0 when not scrolled). Empty if the pane has no
/// history yet, so a fresh shell reads as a clean bar.
unsafe fn scroll_indicator(ctx: *mut screen_redraw_ctx, wp: *mut window_pane) -> String {
    unsafe {
        // Expand scroll offset and history depth separated by a unit-separator
        // byte; both are per-pane. scroll_position is empty outside copy mode.
        let Ok(cfmt) = std::ffi::CString::new("#{scroll_position}\u{1f}#{history_size}") else {
            return String::new();
        };
        let c = (*ctx).c;
        let s = (*c).session;
        let wl = if s.is_null() { null_mut() } else { (*s).curw };
        let ft = format_create_defaults(null_mut(), c, s, wl, wp);
        let out = format_expand(ft, cfmt.as_ptr().cast());
        let raw = crate::cstr_to_str(out).to_string();
        crate::free_(out);
        format_free(ft);
        let (pos, total) = raw.split_once('\u{1f}').unwrap_or(("", "0"));
        let total = total.trim();
        if total.is_empty() || total == "0" {
            return String::new(); // fresh pane, no scrollback yet
        }
        let pos = pos.trim();
        let pos = if pos.is_empty() { "0" } else { pos };
        format!("SCROLL: {pos}/{total}")
    }
}

/// Draw ztmux's own rounded ratatui frame around a pane, with the pane's name in
/// the top border (zellij-style, so it costs no extra row). Synced (red),
/// selected (orange) and trigger-armed (cyan) panes get a loud state badge;
/// ordinary panes get a subtle name-only frame when `@ztmux-pane-names` is on.
/// Drawn at the end of every pane redraw (full or partial) so it survives the
/// pane's own output.
pub(crate) unsafe fn draw_pane_frame(ctx: *mut screen_redraw_ctx, wp: *mut window_pane) {
    unsafe {
        if !enabled() || wp.is_null() {
            return;
        }
        // Don't paint frames while an overlay (command palette, menu, popup,
        // display-panes, hint bar) owns the screen: our per-pane blit ignores the
        // overlay clip, so a pane redraw would scribble over the floating box and
        // wipe it. Frames come back when the overlay closes.
        if (*ctx).c.is_null() || (*(*ctx).c).overlay_draw.is_some() {
            return;
        }
        // Frames only exist in the always-on mode, where the layout has inset
        // every pane by this ring so the frame has reserved space (content can
        // never draw on it). Off => no frames; sync shows via the border colour.
        let inset = frame_inset();
        if inset == 0 {
            return;
        }
        let po = (*wp).options;
        // The pane states, most-urgent first: actively broadcasting (synced),
        // selected to be synced next (marked), or watched by an armed
        // content-trigger (`pipe_fd` is the `#{pane_pipe}` signal). State just
        // recolours the (always present) frame.
        let state = if crate::options_::options_get_number_(po, "synchronize-panes") != 0 {
            Some((Color::Indexed(196), "\u{27f2} SYNCED")) // bright red
        } else if pane_marked_for_sync(po) {
            Some((Color::Indexed(214), "\u{25c6} SELECTED")) // orange
        } else if (*wp).pipe_fd != -1 {
            Some((Color::Indexed(51), "\u{26a1} TRIGGER")) // cyan
        } else {
            None
        };

        // Collapsed pane in a stack (zellij's fixed(1) row): draw the box TOP
        // border with the name, so a stack reads as a set of stacked boxes
        // (╭─ name ─────╮) rather than a flat list.
        if (*wp).sy <= 2 {
            let width = (*wp).sx as usize;
            if width < 4
                || (*wp).yoff < (*ctx).oy
                || (*wp).yoff >= (*ctx).oy + (*ctx).sy
                || (*wp).xoff < (*ctx).ox
                || (*wp).xoff + (*wp).sx > (*ctx).ox + (*ctx).sx
            {
                return;
            }
            let color = match state {
                Some((col, _)) => col,
                None => Color::Indexed(244),
            };
            let mut row: Vec<char> = vec!['\u{2500}'; width]; // ─
            row[0] = '\u{256d}'; // ╭
            row[width - 1] = '\u{256e}'; // ╮
            for (i, ch) in format!(" {} ", pane_display_name(ctx, wp))
                .chars()
                .enumerate()
            {
                if 2 + i < width - 1 {
                    row[2 + i] = ch;
                }
            }
            // Right-aligned scroll indicator (zellij shows `SCROLL: 0/N`): the
            // pane's scrollback depth, and the current copy-mode offset if any.
            let scroll = scroll_indicator(ctx, wp);
            if !scroll.is_empty() {
                let label: Vec<char> = format!(" {scroll} ").chars().collect();
                // Sit just left of the ╮, but never overwrite the name or corner.
                let start = width.saturating_sub(1 + label.len());
                if start > 3 {
                    for (i, ch) in label.iter().enumerate() {
                        row[start + i] = *ch;
                    }
                }
            }
            let st = Style::default().fg(color);
            let area = Rect::new(0, 0, width as u16, 1);
            let mut buf = Buffer::empty(area);
            for (x, ch) in row.iter().enumerate() {
                buf[(x as u16, 0)].set_char(*ch).set_style(st);
            }
            let yoff = (*wp).yoff - (*ctx).oy
                + if (*ctx).statustop != 0 {
                    (*ctx).statuslines
                } else {
                    0
                };
            let xoff = (*wp).xoff - (*ctx).ox;
            let tty = &raw mut (*(*ctx).c).tty;
            blit_tty(tty, &buf, xoff, yoff);
            return;
        }

        // The frame occupies the reserved ring: the full layout cell, one inset
        // cell out from the content on each side.
        let cell_x = (*wp).xoff.saturating_sub(inset);
        let cell_y = (*wp).yoff.saturating_sub(inset);
        let cell_sx = ((*wp).sx + 2 * inset) as u16;
        let cell_sy = ((*wp).sy + 2 * inset) as u16;
        if cell_sx < 4 || cell_sy < 3 {
            return;
        }
        // Only frame cells fully inside the viewport (skip partials).
        if cell_x < (*ctx).ox
            || cell_y < (*ctx).oy
            || cell_x + cell_sx as u32 > (*ctx).ox + (*ctx).sx
            || cell_y + cell_sy as u32 > (*ctx).oy + (*ctx).sy
        {
            return;
        }

        let name = pane_display_name(ctx, wp);
        let (color, title, bold) = match state {
            Some((col, lbl)) => {
                let t = if name.is_empty() {
                    format!(" {lbl} ")
                } else {
                    format!(" {lbl} \u{b7} {name} ")
                };
                (col, t, true)
            }
            // No sync state: the active pane's frame is green (like zellij),
            // inactive panes a subtle grey so they read as labels.
            None => {
                let active = std::ptr::eq(wp, (*(*wp).window).active);
                let col = if active {
                    Color::Green
                } else {
                    Color::Indexed(244)
                };
                (col, format!(" {name} "), active)
            }
        };

        let c = (*ctx).c;
        let tty = &raw mut (*c).tty;
        let area = Rect::new(0, 0, cell_sx, cell_sy);
        let mut buf = Buffer::empty(area);
        let mut style = Style::default().fg(color);
        if bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(style)
            .title(Line::from(title))
            .render(area, &mut buf);

        let yoff = cell_y - (*ctx).oy
            + if (*ctx).statustop != 0 {
                (*ctx).statuslines
            } else {
                0
            };
        let xoff = cell_x - (*ctx).ox;
        blit_tty_frame(tty, &buf, xoff, yoff);
    }
}

/// Blit only the perimeter (border + title row) of a rendered buffer, leaving
/// the pane's interior content untouched.
unsafe fn blit_tty_frame(tty: *mut tty, buf: &Buffer, px: u32, py: u32) {
    unsafe {
        let area = buf.area;
        for y in 0..area.height {
            for x in 0..area.width {
                if x != 0 && x != area.width - 1 && y != 0 && y != area.height - 1 {
                    continue; // interior - leave the pane's content alone
                }
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

/// A zellij-style info bar on the last pane row (just above the status line):
/// context hints for the framed mode. Only in zellij mode - it overwrites the
/// bottommost frames' bottom edge, so it costs no reserved row and pane output
/// can't reach it (panes are inset). Drawn after the frame pass, on the same
/// window/border redraws.
pub(crate) unsafe fn draw_info_bar(ctx: *mut screen_redraw_ctx) {
    unsafe {
        if frame_inset() == 0 {
            return;
        }
        let c = (*ctx).c;
        if c.is_null() || (*c).overlay_draw.is_some() {
            return;
        }
        let width = (*ctx).sx as u16;
        if width < 24 || (*ctx).sy < 3 {
            return;
        }
        let tty = &raw mut (*c).tty;

        let hints = "  \u{2b21} zellij   C-s mark \u{b7} M sync \u{b7} : cmd \u{b7} ? keys \u{b7} d detach  ";
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        let bar = Style::default()
            .bg(Color::Indexed(236))
            .fg(Color::Indexed(250));
        for x in 0..width {
            buf[(x, 0)].set_char(' ').set_style(bar);
        }
        let mut cx = 0u16;
        for ch in hints.chars() {
            if cx >= width {
                break;
            }
            buf[(cx, 0)].set_char(ch).set_style(bar);
            cx += 1;
        }

        let srow = (*ctx).sy - 1
            + if (*ctx).statustop != 0 {
                (*ctx).statuslines
            } else {
                0
            };
        blit_tty(tty, &buf, 0, srow);
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
