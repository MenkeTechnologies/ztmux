# A command table that carries an explicit on, an explicit off AND a toggle for
# one flag has three ways to be wired and only one of them is exercised by a
# test that toggles. Each pair is driven here in the order on, on, off, off,
# toggle, toggle — the sequence that separates "sets the flag" from "flips it",
# since a toggle wired into the on entry survives every other ordering.
# (The refresh-* and scroll-exit-* triples of next-3.7 are unported; they are
# pinned in parity/known_gaps/cmd_copy_mode_missing.sh instead.)
$TM new-window -d -n tog 'printf "0123456789\nabcdefghij\nABCDEFGHIJ\n"; sleep 300'
sleep 1
$TM copy-mode -t tog
$TM send-keys -X -t tog history-top
$TM send-keys -X -t tog start-of-line
$TM send-keys -X -t tog begin-selection
$TM send-keys -X -t tog cursor-right
$TM send-keys -X -t tog cursor-down
st() {
  $TM display-message -p -t tog \
    "$1 rect=#{rectangle_toggle} s=#{selection_start_y},#{selection_start_x} e=#{selection_end_y},#{selection_end_x}"
}
for c in rectangle-on rectangle-on rectangle-off rectangle-off rectangle-toggle rectangle-toggle; do
  $TM send-keys -X -t tog "$c"; st "$c"
done
# toggle-position moves the position indicator between the corners; it must be
# accepted repeatedly and leave the cursor and the selection untouched.
for c in toggle-position toggle-position toggle-position; do
  $TM send-keys -X -t tog "$c"
  $TM display-message -p -t tog "$c cur=#{copy_cursor_y},#{copy_cursor_x} present=#{selection_present}"
done
