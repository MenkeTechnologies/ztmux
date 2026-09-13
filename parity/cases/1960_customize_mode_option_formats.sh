# customize-mode's eight format variables (window-customize.c:296-313 for the
# option rows, :463-464 for the key rows), none of which any case had expanded:
#
#   is_option / is_key      which KIND of row this is -- the two halves of
#                           WINDOW_CUSTOMIZE_DEFAULT_FORMAT (:36-43)
#   option_name             including the "[n]" suffix on an array element
#   option_value            absent on an array row: the C only adds it when the
#                           option is not an array (:309-313)
#   option_scope            window_customize_scope_text(): empty for a global
#                           entry, the session/window/pane it belongs to otherwise
#   option_unit             options-table's .unit, which the default format
#                           appends after the value
#   option_is_array / option_is_global
#
# They reach a user format through `customize-mode -F` and the `-f` filter, which
# is what this case drives -- the filter is itself expanded with the same tree
# (:316), so filtering on #{option_name} proves the variables exist at BUILD time
# and not merely at draw time.
#
# Like 1504/1507/1508/1959 this needs a client, because the mode only builds for
# one. M-+ is mode-tree's expand-all (mode-tree.c:1761); without it the option
# rows stay folded under their category and nothing is drawn to read back.
set -- $TM
BIN="$1"
ISOCK="cuf_$$_inner"

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the PORTED
# rendering, so disable it. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off
# A window-scope and a pane-scope override, so option_is_global is 0 somewhere
# and option_scope has something to name.
$BIN -L "$ISOCK" set -w -t alpha:one automatic-rename off
$BIN -L "$ISOCK" set -p -t alpha:one allow-rename on

wait_mode() {   # $1 = expected mode name, or "" for no mode
  local want="$1" i=0 got
  while [ $i -lt 100 ]; do
    got=$($BIN -L "$ISOCK" display-message -p -t alpha:one '#{pane_mode}' 2>/dev/null)
    [ "$got" = "$want" ] && { sleep 0.2; return 0; }
    i=$((i+1)); sleep 0.1
  done
  echo "wait_mode: timed out waiting for [$want], last=[$got]"
  return 1
}

# Rows are tagged with a leading R so the categories, the box and the status line
# cannot drift into the comparison, and the 80-column pane truncates anything
# longer -- so each pass prints few, short fields.
rows() { $TM capture-pane -p -t client | cat -v | perl -ne 'print if /R\|/' | perl -pe 's/[ \t]+$//'; }

show() { # show <filter> <format> [extra keys to press before reading]
  local filter="$1" format="$2"; shift 2
  $BIN -L "$ISOCK" customize-mode -t alpha:one -f "$filter" -F "$format"
  wait_mode options-mode
  # M-+ expands the TOP-LEVEL nodes only -- mode-tree.c:1761 walks mtd->children,
  # not the whole tree -- so a row nested deeper needs its own keys.
  $TM send-keys -t client M-+
  sleep 0.2
  for k in "$@"; do $TM send-keys -t client "$k"; sleep 0.2; done
  # Fence on the rows appearing rather than sleeping a fixed amount: every pass
  # here expects at least one row, and under suite load the client's repaint can
  # land after any sleep short enough to be worth waiting.
  local i=0
  while [ $i -lt 60 ] && [ -z "$(rows)" ]; do i=$((i+1)); sleep 0.1; done
  rows
  $TM send-keys -t client q
  wait_mode ''
}

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"
# Wait for the inner client to be attached instead of sleeping a fixed two
# seconds: every case here must finish inside the runner's 15s budget
# (run_parity.sh:172), and a blind sleep spends an eighth of it doing nothing.
wait_client() {
  local i=0
  while [ $i -lt 300 ]; do
    [ -n "$($BIN -L "$ISOCK" list-clients -F "#{client_tty}" 2>/dev/null)" ] && { sleep 0.2; return 0; }
    i=$((i+1)); sleep 0.05
  done
  echo "wait_client: TIMEOUT"
}
wait_client

echo "== a global option with a unit, and the server option that shares its name =="
show '#{m:*history-limit*,#{option_name}}' \
     'R|#{?is_option,OPT,}#{?is_key,KEY,}|#{option_name}|scope=[#{option_scope}]|unit=[#{option_unit}]|a#{option_is_array}g#{option_is_global}'

echo "== an array: the elements carry the [n] suffix and a value, the parent none =="
# Down lands on the command-alias row under Server Options; Right expands it.
show '#{m:command-alias*,#{option_name}}' \
     'R|#{option_name}|v=[#{option_value}]|a#{option_is_array}' Down Right

echo "== a scalar option does have option_value =="
show '#{==:#{option_name},history-limit}' \
     'R|#{option_name}|v=[#{option_value}]|a#{option_is_array}'

echo "== a window and a pane override: not global, and the scope is named =="
show '#{||:#{==:#{option_name},automatic-rename},#{==:#{option_name},allow-rename}}' \
     'R|#{option_name}|scope=[#{option_scope}]|g#{option_is_global}'

echo "== a key row is the other branch: is_key 1, is_option 0, no option_name =="
show '#{==:#{key},c}' \
     'R|#{?is_option,OPT,}#{?is_key,KEY,}|key=[#{key}]|name=[#{option_name}]|note=[#{key_note}]'

$BIN -L "$ISOCK" kill-server 2>/dev/null
