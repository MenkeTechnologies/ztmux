# The four quoting modifiers on one string carrying every character each of them
# cares about. `#{q:}` escapes for the shell, `#{q|a:}` for a command argument
# (args_escape, which wraps in double quotes rather than backslash-escaping each
# byte) and `#{q|h:}` / `#{q|e:}` for a style string (doubling `#`).
#
# These three produce visibly different output for the same input, so a port that
# parses a modifier and then forgets to apply it still looks correct on `#{q:}`
# alone. That is exactly how FORMAT_QUOTE_ARGUMENTS went missing: `#{q|a:}` was
# accepted by the parser and returned its input unchanged.
#
# A user option holds the payload so the case does not depend on any host value.
$TM set-option -g @q 'a b;c$d"e'"'"'f'
$TM display-message -p 'raw =[#{@q}]'
$TM display-message -p 'q   =[#{q:@q}]'
$TM display-message -p 'q|a =[#{q|a:@q}]'
# Style quoting only shows itself on a string containing `#`.
$TM set-option -g @s 'a#b,c]d#{x}'
$TM display-message -p 'raw =[#{@s}]'
$TM display-message -p 'q|h =[#{q|h:@s}]'
$TM display-message -p 'q|e =[#{q|e:@s}]'
$TM display-message -p 'q|a =[#{q|a:@s}]'
# An empty value, and one that is nothing but characters needing escapes.
$TM set-option -g @e ''
$TM set-option -g @x '$`"\'
$TM display-message -p 'empty=[#{q:@e}][#{q|a:@e}][#{q|h:@e}]'
$TM display-message -p 'all  =[#{q:@x}]'
$TM display-message -p 'all|a=[#{q|a:@x}]'
