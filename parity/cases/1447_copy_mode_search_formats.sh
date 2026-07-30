# The search bookkeeping formats, which are what a status line shows while a
# search is live: how many matches were marked, whether the count is partial,
# which match the cursor is sitting on, and whether the mark set exists at all.
# The search that produced them was silently finding nothing until recently
# (1436), and these counters come from a second pass over the grid, so they can
# be wrong even once the cursor lands correctly.
$TM new-window -d -n hits 'printf "needle one\nhay\nneedle two\nhay\nneedle three\n"; sleep 300'
sleep 1
$TM copy-mode -t hits
state() {
  $TM display-message -p -t hits \
    "$1 present=#{search_present} count=#{search_count} partial=#{search_count_partial} timedout=#{search_timed_out} match=[#{search_match}] cur=#{copy_cursor_y},#{copy_cursor_x}"
}
state before
$TM send-keys -X -t hits search-backward needle; state after-backward
$TM send-keys -X -t hits search-again; state again
$TM send-keys -X -t hits search-reverse; state reverse
# The literal-text variants set the same state as the regex ones.
$TM send-keys -X -t hits history-top
$TM send-keys -X -t hits search-forward-text hay; state forward-text
# A regex-only pattern proves search-forward is regex and -text is not.
$TM send-keys -X -t hits history-top
$TM send-keys -X -t hits search-forward 'ne+dle'; state regex
$TM send-keys -X -t hits history-top
$TM send-keys -X -t hits search-forward-text 'ne+dle'; state literal
# #{pane_search_string} carries the last term back out to the caller.
$TM display-message -p -t hits "term=[#{pane_search_string}]"
