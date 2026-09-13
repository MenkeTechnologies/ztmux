# list-keys installs five format variables that describe the LISTING rather than
# any one binding, and no case had ever expanded one (cmd-list-keys.c:236-240):
#
#   notes_only        1 when -N was given
#   key_has_repeat    1 when ANY key in the set being listed is -r
#   key_repeat        per binding: is THIS key -r
#   key_string_width  max utf8_cstrwidth() over the listed key strings
#   key_table_width   max utf8_cstrwidth() over the listed table names
#
# The two widths are what LIST_KEYS_TEMPLATE pads with (cmd-list-keys.c:30-38),
# so they are not decoration: get the width wrong and every default `list-keys`
# line is misaligned. They are computed over the whole set before the per-key
# loop, so a port that recomputed them per line would print 1-wide padding here
# and look right on a single-key table.
#
# A CJK key is bound on purpose: its key string is three BYTES and two COLUMNS,
# so a port measuring strlen() instead of utf8_cstrwidth() (cmd-list-keys.c:75)
# reports 3 where the C reports 2.
#
# The listings are sorted for the same reason case 1498 sorts: `-O key` walks the
# table in KEY-CODE order, and ztmux's flat keyc enum numbers a literal, a C-,
# an F- and an M- key differently from the C's type-shifted one, so the two
# binaries emit the same set of lines in a different order. That encoding is a
# recorded gap; this case is about the widths and the repeat flags, so it
# compares the set and leaves the order to the case that owns it.
$TM bind -T zt-w a display-message A
$TM bind -T zt-w C-x display-message CX
$TM bind -T zt-w -N 'the note on F11' F11 display-message F11
$TM bind -T zt-w -r M-Up display-message MUP

echo "== key_string_width is the widest key string in the set =="
$TM list-keys -T zt-w -O key -F '#{key_string} width=#{key_string_width}' | sort

echo "== a 3-byte 2-column key is measured in COLUMNS =="
$TM bind -T zt-wide 世 display-message CJK
$TM list-keys -T zt-wide -F '[#{key_string}] bytes=#{n:key_string} width=#{key_string_width}'

echo "== key_has_repeat is a property of the SET, key_repeat of the binding =="
$TM list-keys -T zt-w -O key -F '#{key_string}: has_repeat=#{key_has_repeat} repeat=#{key_repeat}' | sort
echo "-- a table with no -r key at all --"
$TM list-keys -T zt-wide -F '#{key_string}: has_repeat=#{key_has_repeat} repeat=#{key_repeat}'

echo "== notes_only is 1 only under -N =="
$TM list-keys -T zt-w -O key -F '#{key_string}=#{notes_only}' | sort
$TM list-keys -T zt-w -N -a -O key -F '#{key_string}=#{notes_only}' | sort

echo "== key_table_width across a single table =="
$TM list-keys -T zt-wide -F 'table=[#{key_table}] width=#{key_table_width}'

echo "== the default template pads both columns from those widths =="
$TM list-keys -T zt-w -O key | sort | cat -v | perl -pe "s/ /./g"
echo "-- and -N uses the other half of the template --"
$TM list-keys -T zt-w -N -a -O key | sort | cat -v | perl -pe "s/ /./g"
