# Invalid UTF-8 becomes U+FFFD, and does not vanish.
#
# tmux enters a replacement character for a byte sequence that cannot open or
# append as UTF-8 (input.c:783 input_stop_utf8, called from input_top_bit_set at
# :2844 and :2852 and from the two "can't be valid UTF-8" sites at :1212, :1292).
# The port used to DROP those bytes instead, which is not a cosmetic difference:
# every column after an invalid byte shifted, so wrapping, cursor position and
# capture-pane output all desynchronised from the reference. Any `cat` of a
# binary, a latin-1 log or mixed-encoding output reaches it.
#
# The ordering in input_top_bit_set is the load-bearing part and the reason this
# case also asserts VALID multi-byte input: the C sets utf8started = 1 BEFORE the
# utf8_open check, so a byte that fails to open still emits U+FFFD. Set it only
# on success and invalid bytes disappear; set it in the wrong place relative to
# the append path and VALID CJK, emoji and box-drawing characters start emitting
# U+FFFD instead. Both directions are checked here.

# A run of invalid bytes interleaved with ASCII: 0xFF (never valid), 0x80 (a
# continuation byte with no lead) and 0xE4 0xFF (a 3-byte lead followed by a
# byte that cannot continue it) -- the open-fail, the stray-continuation and the
# append-fail paths respectively.
$TM new-window -d -n bad 'printf "x\377y\200z\344\377w"; sleep 300'
sleep 1
echo "invalid:"
$TM capture-pane -p -t bad | head -1 | cat -v
echo "bytes=$($TM capture-pane -p -t bad | head -1 | tr -d '\n' | wc -c | tr -d ' ')"
$TM display-message -p -t bad 'cursor=#{cursor_x},#{cursor_y}'

# Valid multi-byte must be completely unaffected: 2-byte, 3-byte and 4-byte
# sequences, including a double-width CJK pair and an emoji.
$TM new-window -d -n good 'printf "CJK:\344\270\255\346\226\207 emoji:\360\237\232\200 box:\342\224\214\342\224\200\342\224\220 acc:\303\251"; sleep 300'
sleep 1
echo "valid:"
$TM capture-pane -p -t good | head -1 | cat -v
echo "bytes=$($TM capture-pane -p -t good | head -1 | tr -d '\n' | wc -c | tr -d ' ')"
$TM display-message -p -t good 'cursor=#{cursor_x},#{cursor_y}'

# A truncated sequence at end of output: the lead byte started a sequence that
# never completes, so it must still produce exactly one replacement character.
$TM new-window -d -n trunc 'printf "end\344\270"; sleep 300'
sleep 1
echo "truncated:"
$TM capture-pane -p -t trunc | head -1 | cat -v
$TM display-message -p -t trunc 'cursor=#{cursor_x},#{cursor_y}'

# Invalid bytes adjacent to valid ones, to pin the boundary between the two
# paths rather than each in isolation.
$TM new-window -d -n mix 'printf "\303\251\377\344\270\255\200ok"; sleep 300'
sleep 1
echo "mixed:"
$TM capture-pane -p -t mix | head -1 | cat -v
$TM display-message -p -t mix 'cursor=#{cursor_x},#{cursor_y}'
