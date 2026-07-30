# The copy commands are where a wrong selection boundary finally becomes
# visible: each one turns a pair of grid coordinates into buffer text, so an
# off-by-one that the coordinate formats hide (because both ends moved) shows up
# as a missing or extra character here. copy-line, copy-end-of-line and their
# -no-clear/-and-cancel variants are separate table entries with separate
# boundary maths.
$TM new-window -d -n cp 'printf "first line text\nsecond line text\nthird\n"; sleep 300'
sleep 1
buf() { $TM show-buffer 2>&1 | perl -pe "s{^(.*)\$}{[\$1]}"; }
$TM copy-mode -t cp
$TM send-keys -X -t cp history-top
$TM send-keys -X -t cp start-of-line
# A three-cell char selection.
$TM send-keys -X -t cp begin-selection
$TM send-keys -X -t cp cursor-right
$TM send-keys -X -t cp cursor-right
$TM send-keys -X -t cp copy-selection-no-clear
echo "sel3:"; buf
# A selection that spans a line break.
$TM send-keys -X -t cp clear-selection
$TM send-keys -X -t cp start-of-line
$TM send-keys -X -t cp begin-selection
$TM send-keys -X -t cp cursor-down
$TM send-keys -X -t cp cursor-right
$TM send-keys -X -t cp copy-selection-no-clear
echo "spanning:"; buf
# Whole-line and to-end-of-line copies from a mid-line cursor.
$TM send-keys -X -t cp clear-selection
$TM send-keys -X -t cp start-of-line
$TM send-keys -X -t cp cursor-right
$TM send-keys -X -t cp cursor-right
$TM send-keys -X -t cp cursor-right
$TM send-keys -X -t cp copy-line
echo "copy-line:"; buf
$TM send-keys -X -t cp copy-end-of-line
echo "copy-end-of-line:"; buf
# append-selection concatenates onto the most recent buffer instead of adding
# another one, so the buffer count is part of the assertion.
$TM send-keys -X -t cp start-of-line
$TM send-keys -X -t cp begin-selection
$TM send-keys -X -t cp cursor-right
$TM send-keys -X -t cp append-selection
echo "appended:"; buf
$TM list-buffers -F '#{buffer_name} #{buffer_size}'
$TM display-message -p -t cp "mode=#{pane_mode} in=#{pane_in_mode}"
