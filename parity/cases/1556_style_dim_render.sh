# `dim=` in a style: parsed, stored, and actually applied to the drawn colours.
#
# This was the last style directive the two binaries disagreed on. It was
# recorded as open rather than half-ported, because accepting it is trivial and
# HONOURING it is not: the C carries the percentage in `struct tty_style_ctx`
# (tmux.h:1686) and tty_attributes dims the resolved fg and bg through
# colour_dim (tty.c:2649-2659). This port passed `defaults`, `palette` and
# `hyperlinks` as three separate parameters, so there was nowhere for `dim` to
# live; a parse-only version would have stored a value nothing read, rendering
# undimmed while looking applied.
#
# Ordering is the load-bearing part, and the reason the render half is checked
# here rather than only the parse. colour_dim returns a THEME colour untouched
# (it has no RGB to scale) and returns a DEFAULT colour untouched for the same
# reason, so the C resolves theme colours and substitutes a concrete colour for
# the default BEFORE dimming (tty.c:2646-2652). Get that order wrong and
# `dim=` silently does nothing for exactly the colours a default config uses.
set -- $TM
BIN="$1"
ISOCK="dim_$$_inner"

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 \
  'printf "AAAA\nBBBB\n"; sleep 300'
$BIN -L "$ISOCK" set -g status off
$BIN -L "$ISOCK" set -g @ztmux-ratatui off
$BIN -L "$ISOCK" split-window -d -t alpha:one 'printf "CCCC\n"; sleep 300'

echo "== accept/reject and the stored value =="
for d in 'dim=0' 'dim=30' 'dim=30%' 'dim=100' 'dim=101' 'dim=-1' 'dim=abc' 'dim=' \
         'fg=red,dim=50' 'dim=50,fg=red' 'fill=red,dim=40,fg=blue' dim nodim; do
  out=$($BIN -L "$ISOCK" set -g status-style "$d" 2>&1); rc=$?
  printf '%-22s rc=%s out=[%s] val=[%s]\n' "$d" "$rc" "$out" \
    "$($BIN -L "$ISOCK" show -gv status-style)"
  $BIN -L "$ISOCK" set -gu status-style
done

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"
sleep 2

echo "== the drawn colours, dimmed =="
# 256-palette colours are forced to RGB by the dim, so the emitted sequence
# changes shape as well as value -- 38;5;196 becomes 38;2;R;G;B.
for spec in 'fg=colour196,bg=colour21' \
            'fg=colour196,bg=colour21,dim=50' \
            'fg=colour196,bg=colour21,dim=100' \
            'fg=red,bg=blue,dim=25'; do
  $BIN -L "$ISOCK" set -g window-style "$spec"
  $BIN -L "$ISOCK" set -g window-active-style "$spec"
  $BIN -L "$ISOCK" refresh-client -S 2>/dev/null
  sleep 0.9
  printf '=== %s\n' "$spec"
  $TM capture-pane -p -e -t client | sed -n '1p' | cat -v | perl -pe 's/\s+$//'
done

echo "== active and inactive panes dim independently =="
# window-style and window-active-style each cache their own percentage
# (tty.c:3120 tty_style_changed), so the two panes must differ.
$BIN -L "$ISOCK" set -g window-style 'fg=colour196,bg=colour21,dim=80'
$BIN -L "$ISOCK" set -g window-active-style 'fg=colour196,bg=colour21,dim=0'
$BIN -L "$ISOCK" refresh-client -S 2>/dev/null
sleep 0.9
$TM capture-pane -p -e -t client | sed -n '1,14p' | cat -v | perl -pe 's/\s+$//' \
  | grep -o 'AAAA\|CCCC\|38;2;[0-9;]*m\|38;5;[0-9]*m' | head -8

echo "== a default colour with nothing to substitute stays undimmed =="
# colour_dim cannot scale colour 8, so tty_dim_default_colour substitutes a
# concrete colour first (tty.c:2598) -- the terminal's reported fg/bg, or the
# client theme's black/white. Under this suite the inner client's terminal
# reports neither and has no theme, so the fallback returns the colour
# unchanged and the line draws with no colour sequence at all. That is the
# `return (c)` branch, and pinning it is what stops a future "just dim it
# anyway" from inventing a colour the reference does not emit.
$BIN -L "$ISOCK" set -g window-style 'dim=60'
$BIN -L "$ISOCK" set -g window-active-style 'dim=60'
$BIN -L "$ISOCK" refresh-client -S 2>/dev/null
sleep 0.9
$TM capture-pane -p -e -t client | sed -n '1p' | cat -v | perl -pe 's/\s+$//'
# Explicitly: no SGR colour was emitted for that line on either binary.
$TM capture-pane -p -e -t client | sed -n '1p' | grep -c '38;[25];' || true

echo "== and dim=0 draws exactly what no dim at all draws =="
$BIN -L "$ISOCK" set -g window-style 'fg=colour196,bg=colour21,dim=0'
$BIN -L "$ISOCK" set -g window-active-style 'fg=colour196,bg=colour21,dim=0'
$BIN -L "$ISOCK" refresh-client -S 2>/dev/null
sleep 0.9
$TM capture-pane -p -e -t client | sed -n '1p' | cat -v | perl -pe 's/\s+$//'

$BIN -L "$ISOCK" kill-server 2>/dev/null
