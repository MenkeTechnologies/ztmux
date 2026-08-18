# `#[range=control|N]`, the click target inside a drawn format.
#
# next-3.7 added STYLE_RANGE_CONTROL (tmux.h:939) so a format can mark a region
# as a numbered click target; a press there resolves to the CONTROL0-9 mouse
# locations (server-client.c:963), which is how the default pane-border-format
# gets its Zoom and Kill buttons. The port had neither the range type nor the
# locations, so the directive failed to parse and the two default bindings
# MouseDown1Control8 / MouseDown1Control9 had nothing to attach to.
#
# This is reachable only with `pane-border-status` on -- it defaults to off in
# both binaries -- so it is pinned here by parsing and round-tripping the
# directive directly rather than by clicking a border.
echo "== parses and round-trips, every valid N =="
for n in 0 1 5 8 9; do
  $TM set -g status-left "#[range=control|$n]x#[norange]" 2>&1
  printf 'N=%s rc=%s value=%s\n' "$n" "$?" "$($TM show-options -gv status-left)"
done

# Invalid forms are NOT checked here: `range=` is parsed by style_parse at RENDER
# time (format_draw), not when the option is set, so `set -g status-left` accepts
# any string and display-message passes styles through untouched. Both binaries
# agree on that, but it discriminates nothing. The rejection path is pinned as a
# unit test instead, where style_parse is directly callable
# (src/ported/style.rs, range_control_parses_and_bounds_its_argument).

echo "== the range survives a full style round trip alongside other directives =="
$TM set -g status-left '#[fg=red,range=control|4,bold]z#[norange,default]'
$TM show-options -gv status-left

echo "== and it does not disturb the other range types =="
for r in 'range=left' 'range=right' 'range=window|5' 'range=user|abc'; do
  $TM set -g status-left "#[$r]q#[norange]" 2>&1 >/dev/null
  printf '%-18s -> %s\n' "$r" "$($TM show-options -gv status-left)"
done

echo "== the default pane-border-format still carries its control ranges =="
$TM show-options -gwv pane-border-format

$TM set -gu status-left
