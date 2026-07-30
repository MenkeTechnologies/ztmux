# selection-mode sets what a selection covers: characters, words or whole
# lines. It takes the mode as an optional argument, matched case-insensitively
# with a one-letter abbreviation, and an unrecognised value leaves the current
# mode alone rather than erroring. #{selection_mode} reports the result, and the
# selection extents show it actually applied.
$TM new-window -d -n sm 'printf "alpha bravo charlie\ndelta echo foxtrot\n"; sleep 300'
sleep 1
$TM copy-mode -t sm
$TM send-keys -X -t sm history-top
$TM send-keys -X -t sm start-of-line
mode() { $TM display-message -p -t sm "$1 mode=#{selection_mode}"; }
for arg in word line char WORD Line CHAR w l c W L C; do
  $TM send-keys -X -t sm selection-mode "$arg"
  mode "$arg"
done
# No argument at all resets to char; an unknown one is ignored.
$TM send-keys -X -t sm selection-mode line
$TM send-keys -X -t sm selection-mode nonsense
mode unknown
$TM send-keys -X -t sm selection-mode
mode none
# Word mode selects whole words as the cursor moves; line mode whole lines.
$TM send-keys -X -t sm selection-mode word
$TM send-keys -X -t sm begin-selection
$TM send-keys -X -t sm next-word
$TM display-message -p -t sm "word s=#{selection_start_y},#{selection_start_x} e=#{selection_end_y},#{selection_end_x}"
$TM send-keys -X -t sm copy-selection-no-clear
$TM show-buffer | perl -pe "s{^(.*)\$}{[\$1]}"
$TM send-keys -X -t sm clear-selection
$TM send-keys -X -t sm selection-mode line
$TM send-keys -X -t sm begin-selection
$TM send-keys -X -t sm cursor-down
$TM display-message -p -t sm "line s=#{selection_start_y},#{selection_start_x} e=#{selection_end_y},#{selection_end_x}"
$TM send-keys -X -t sm copy-selection-no-clear
$TM show-buffer | perl -pe "s{^(.*)\$}{[\$1]}"
# Too many arguments is an arity error.
$TM send-keys -X -t sm selection-mode word extra 2>&1
$TM display-message -p -t sm "alive #{pane_in_mode} mode=#{selection_mode}"
