# Every mouse key NAME: 65 event families x 19 locations, bound and listed back.
#
# tmux.h:268 KEYC_MOUSE_STRING expands each mouse event family into 19 named
# keys -- Pane, Status, StatusLeft, StatusRight, StatusDefault, ScrollbarUp,
# ScrollbarSlider, ScrollbarDown, Border and Control0..Control9 -- and tmux.h:233
# KEYC_MOUSE_KEYS gives each of those its own key code. The families are
# MouseDown/MouseUp/MouseDrag/MouseDragEnd/SecondClick/DoubleClick/TripleClick
# over buttons 1,2,3,6,7,8,9,10,11 plus WheelUp and WheelDown: 65 in all, so the
# full name space is 65 * 19 = 1235 keys.
#
# The scrollbar and control locations were ported only recently, and a port can
# get this wrong in ways that no single spot check notices: a location missing
# from one family but present in another, a location whose parse and whose
# to-string disagree (binds fine, lists back as a DIFFERENT name), two locations
# colliding on the same key code (the second bind silently overwrites the first,
# so the table ends up short), or the location list appearing in a different
# order in one family than another. This case walks the whole matrix, so all
# four of those show up as a diff.
#
# Purely server side and free of any host/time/pid input, so it is exhaustive
# without being expensive: the 1235 binds go in through one source-file.
locs="Pane Status StatusLeft StatusRight StatusDefault Border ScrollbarUp
      ScrollbarSlider ScrollbarDown Control0 Control1 Control2 Control3 Control4
      Control5 Control6 Control7 Control8 Control9"
fams=""
for fm in MouseDown MouseUp MouseDrag MouseDragEnd SecondClick DoubleClick TripleClick; do
  for b in 1 2 3 6 7 8 9 10 11; do fams="$fams $fm$b"; done
done
fams="$fams WheelUp WheelDown"

# Bind every name in a private table. The bound command carries the name we
# ASKED for, so each list-keys line holds both the canonical name tmux printed
# and the name we requested -- a per-key round trip in one line.
F="$(mktemp "${TMUX_TMPDIR:-/tmp}/mouseloc.XXXXXX")"
want=0
for fa in $fams; do
  for l in $locs; do
    printf 'bind-key -T zparity %s%s display-message %s%s\n' "$fa" "$l" "$fa" "$l" >> "$F"
    want=$((want+1))
  done
done
echo "names generated: $want"
$TM source-file "$F" 2>&1
rm -f "$F"

# Every name must have parsed, and each must have its OWN key code: a collision
# would make two binds land on one entry and the count would come up short.
echo "keys in table:   $($TM list-keys -T zparity | wc -l | tr -d ' ')"

# Round trip: canonical name (field 4) must equal the requested name (field 6).
echo "round-trip mismatches:"
$TM list-keys -T zparity | awk '$4 != $6 { print "  " $4 " != " $6 }'
echo "(end)"

# The location order inside one family, verbatim. This pins the location enum
# order, not just the set: reordering KEYC_MOUSE_LOCATION_* changes every key
# code and would show here even though the count stayed 1235.
echo "MouseDown1 locations in table order:"
$TM list-keys -T zparity | awk '$4 ~ /^MouseDown1[A-Z]/ { sub(/^MouseDown1/, "", $4); print "  " $4 }'

# ... and that the same 19 appear, in the same order, for a wheel family (no
# button number in the name) and for the highest button number.
echo "WheelDown locations in table order:"
$TM list-keys -T zparity | awk '$4 ~ /^WheelDown[A-Z]/ { sub(/^WheelDown/, "", $4); print "  " $4 }'
echo "TripleClick11 locations in table order:"
$TM list-keys -T zparity | awk '$4 ~ /^TripleClick11[A-Z]/ { sub(/^TripleClick11/, "", $4); print "  " $4 }'

# Per-family census: any family that lost a location drops below 19 here.
echo "families with a location count other than 19:"
$TM list-keys -T zparity |
  awk '{ n = $4; sub(/(Pane|Status|StatusLeft|StatusRight|StatusDefault|Border|ScrollbarUp|ScrollbarSlider|ScrollbarDown|Control[0-9])$/, "", n); c[n]++ }
       END { for (k in c) if (c[k] != 19) print "  " k " " c[k] }' | sort
echo "(end)"

# Single-key lookup by name for the three scrollbar locations and both ends of
# the control range -- the newly ported ones -- through the list-keys argument
# path rather than the whole-table dump.
for k in MouseDown1ScrollbarUp MouseDrag1ScrollbarSlider MouseUp3ScrollbarDown \
         WheelUpControl0 DoubleClick2Control9; do
  $TM list-keys -T zparity "$k"
done

# Unbinding by name resolves to the same key code it was bound with.
$TM unbind-key -T zparity MouseDrag1ScrollbarSlider
$TM list-keys -T zparity MouseDrag1ScrollbarSlider 2>&1
echo "after unbind: $($TM list-keys -T zparity | wc -l | tr -d ' ')"

# Names that must NOT parse: a location that does not exist, a button tmux has
# no key for (4 and 5 are the wheel), and a bare family with no location.
for bad in MouseDown1Scrollbar MouseDown1Control10 MouseDown4Pane MouseDown5Pane \
           MouseDown1 WheelUp WheelLeftPane MouseDown1PANE; do
  printf '%s -> %s\n' "$bad" "$($TM bind-key -T zparity "$bad" display-message x 2>&1)"
done
