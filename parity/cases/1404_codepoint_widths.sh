# codepoint-widths (array) + variation-selector-always-wide options round-trip.
$TM show-options -sg variation-selector-always-wide
$TM set-option -sa codepoint-widths "U+1F600=2"
$TM set-option -sa codepoint-widths "U+261D-U+270D=1"
$TM show-options -sg codepoint-widths
