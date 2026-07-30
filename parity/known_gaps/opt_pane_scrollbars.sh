# GAP: pane scrollbars (screen-redraw.c scrollbar scene: redraw_draw_scrollbar_span,
# redraw_pane_scrollbar, redraw_mark_pane_scrollbar).
#
# The copy-mode command `scroll-to-mouse` belongs to this gap rather than to the
# command table: it drags the scrollbar slider, so it needs window_copy_scroll /
# window_copy_scroll1 reading wp->sb_slider_h (tmux.h:1301) and
# tty.mouse_slider_mpos (tmux.h:1769), none of which exist without the scrollbar
# subsystem. It is deliberately not driven here — the vendored tmux takes its own
# server down when it runs with no mouse event to read (verified directly against
# vendor/tmux/tmux next-3.7), so a case calling it would measure that upstream
# crash instead of this gap. Every other next-3.7 copy-mode command is ported.
$TM show-options -wg pane-scrollbars
$TM show-options -wg pane-scrollbars-position
$TM show-options -wg pane-scrollbars-style
$TM show-options -wg pane-scrollbars-timeout
