# Style `width=` and `pad=` directives.
#
# These were rejected outright as `invalid style:` before being ported, which no
# case covered. They are what `pane-scrollbars-style` is written in
# (`width=1,pad=0`) and what the message area sizes itself from.
s() { $TM set-option -g status-style "$1" 2>&1 | head -1; $TM show-options -g status-style; }

# A cell count, a percentage, and both together.
s 'bg=red,width=10'
s 'bg=red,width=0'
s 'bg=red,width=50%'
s 'bg=red,width=0%'
s 'bg=red,width=100%'
s 'bg=red,pad=2'
s 'bg=red,pad=0'
s 'bg=red,width=10,pad=2'
s 'bg=red,pad=2,width=10'

# Order and round-tripping: what show-options prints must parse back the same.
$TM set-option -g status-style 'width=3,pad=1,bg=red,align=centre'
$TM show-options -g status-style
$TM set-option -g status-style "$($TM show-options -gv status-style)"
$TM show-options -g status-style

# Out of range and malformed forms have to be refused, not silently clamped.
s 'bg=red,width=101%'
s 'bg=red,width=-1'
s 'bg=red,width='
s 'bg=red,width=abc'
s 'bg=red,width=10%%'
s 'bg=red,pad=-1'
s 'bg=red,pad='
s 'bg=red,pad=abc'

# Unset leaves neither directive in the output.
$TM set-option -g -u status-style
$TM show-options -g status-style
