# #{=N:...} and #{=-N:...} over strings containing double-width characters,
# combining marks and a ZWJ sequence -- then the same truncation PAINTED into a
# status bar.
#
# The limit is a COLUMN budget, not a byte or character count: format_trim_left
# and format_trim_right (format-draw.c) walk the string with utf8_open/append and
# add ud.width per character. Two consequences that a byte- or char-based
# truncation cannot reproduce, and that cases 1060-1062 (ASCII only) cannot see:
#
#   * A character that would straddle the limit is DROPPED, not half-emitted and
#     not replaced by a space -- format_trim_left copies only when
#     `width + ud.width <= limit` -- so an odd budget in front of a wide char
#     yields a result one column SHORT of the budget (=3 and =2 agree, =5 and =4
#     agree, ...).
#   * Because `width += ud.width` runs even when the character was dropped, the
#     loop then exits, so nothing further is appended even if a narrow character
#     would still have fitted.
#
# The marker form is checked alongside, because the marker is appended only when
# the trim actually changed the string -- with a wide char the "changed" test can
# trigger at a budget where a naive char-count implementation would think nothing
# was cut. #{p N:} is included as the paired width-aware padding (utf8_padcstr),
# since a truncation that is column-correct and a padding that is byte-correct
# still misalign a status bar.
#
# The last section is the rendering: the same modifier drives status-left with
# status-left-length left wide open, so the format is the only thing cutting the
# string, and the column the window list starts at is what proves the painted
# result is the trimmed one.
#
# Deliberately absent: #{=-N:...} applied to a string whose FIRST kept character
# is zero-width. format_trim_right keeps characters from `width >= skip` onward,
# and when that first kept character is a combining mark or a ZWJ both binaries
# abort the server -- `#{=-4:<U+1F468 U+200D U+1F4BB>xy}`, `#{=-1:e<U+0301>Q}`,
# `#{=-3:e<U+0301><U+4E2D>Q}` all do it. Identical on both sides, so not a parity
# difference, but a dead server would put a socket path into the compared output.
# The right-hand sweep below therefore runs only over the two strings that have
# no zero-width characters in them.
set -- $TM
BIN="$1"
ISOCK="uft_$$_inner"
# Kill the inner server even if the harness timeout cuts this script short, so a
# slow run cannot leave a server (and its sleeps) behind for the rest of the suite.
trap '$BIN -L "$ISOCK" kill-server >/dev/null 2>&1' EXIT INT TERM

W='\344\270\255'                                    # U+4E2D wide
WEN='\346\226\207'                                  # U+6587 wide
ACU='\314\201'                                      # U+0301 combining acute
ZWJ='\360\237\221\250\342\200\215\360\237\222\273'  # U+1F468 U+200D U+1F4BB
ELL=$(printf '\342\200\246')                        # U+2026, the usual marker
S1=$(printf "${W}${WEN}${W}${WEN}${W}${WEN}")       # 6 wide chars, 12 columns
S2=$(printf "a${W}b${WEN}c")                        # narrow/wide mix, 7 columns
S3=$(printf "${ZWJ}xy")                             # ZWJ sequence then ASCII
S4=$(printf "e${ACU}${W}Q")                         # combining mark then wide

$TM set -g @s1 "$S1"; $TM set -g @s2 "$S2"
$TM set -g @s3 "$S3"; $TM set -g @s4 "$S4"

for k in 1 2 3 4; do
  # #{w:} is the column width the trimming works in, #{n:} the byte length: they
  # differ for every one of these strings, which is the whole point.
  echo "== s$k [$($TM display-message -p "#{@s$k}" | cat -v)] width=$($TM display-message -p "#{w:#{@s$k}}") bytes=$($TM display-message -p "#{n:#{@s$k}}")"
  for n in 1 2 3 4 5 6 7 12; do
    printf '   =%-2s [%s]\n' "$n" "$($TM display-message -p "#{=$n:#{@s$k}}" | cat -v)"
  done
  for n in 3 4 5; do
    printf '   =/%s/marker [%s]\n' "$n" "$($TM display-message -p "#{=/$n/$ELL:#{@s$k}}" | cat -v)"
  done
  for n in 8 9; do
    printf '   p%-2s [%s]  p-%-2s [%s]\n' "$n" "$($TM display-message -p "#{p$n:#{@s$k}}" | cat -v)" "$n" "$($TM display-message -p "#{p-$n:#{@s$k}}" | cat -v)"
  done
done

# Right-hand trimming, on the strings that do not hit the shared abort noted
# above. This is format_trim_right, which computes skip = total - limit and keeps
# characters from `width >= skip` onwards -- a different straddle rule from the
# left-hand trim, so the two sweeps do not mirror each other.
for k in 1 2; do
  echo "== s$k right trim"
  for n in 1 2 3 4 5 6 7 12; do
    printf '   =-%-2s [%s]\n' "$n" "$($TM display-message -p "#{=-$n:#{@s$k}}" | cat -v)"
  done
  for n in 3 4 5; do
    printf '   =/-%s/marker [%s]\n' "$n" "$($TM display-message -p "#{=/-$n/$ELL:#{@s$k}}" | cat -v)"
  done
done

# The rendered half. A second server inside a pane of the first with a client
# attached (cases 1504/1507/1508), so capture-pane on the OUTER server returns
# what the inner client actually painted.
$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status-interval 0
$BIN -L "$ISOCK" set -g status-left-length 60
$BIN -L "$ISOCK" set -g status-right-length 20
# ztmux's floating overlay is an intentional extension; this case pins the
# PORTED rendering, so disable it. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off
$BIN -L "$ISOCK" set -g @s1 "$S1"

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"
i=0
while [ $i -lt 150 ]; do
  g=$($BIN -L "$ISOCK" display-message -p -t alpha:one '#{window_width}x#{window_height}' 2>/dev/null)
  [ "$g" = "80x23" ] && break
  i=$((i+1)); sleep 0.1
done
echo "geometry: $g"

# An ASCII tag on the right identifies each redraw: consecutive budgets can paint
# IDENTICAL left-hand text (a wide char dropped at =3 leaves the same two columns
# as =2), so "wait until the line changes" would never return.
echo "status-left = #{=N:...} with status-left-length wide open:"
for n in 2 3 4 5 12; do
  $BIN -L "$ISOCK" set -g status-left "[#{=$n:#{@s1}}]"
  $BIN -L "$ISOCK" set -g status-right "F$n"
  i=0
  while [ $i -lt 200 ]; do
    line=$($TM capture-pane -p -t client | tail -1)
    case "$line" in *"F$n") break ;; esac
    i=$((i+1)); sleep 0.1
  done
  printf 'trunc=%-2s ' "$n"
  printf '%s\n' "$line" | cat -v
done

$BIN -L "$ISOCK" kill-server 2>/dev/null
