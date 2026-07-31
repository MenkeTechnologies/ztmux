# tree-mode-* and switch-mode-match-style: the option surface.
#
# Five window options added by next-3.7 for the choose-tree/choose-client family.
# `tree-mode-preview-format` is a plain format string; the other four are styles,
# so they carry OPTIONS_TABLE_IS_STYLE and a "," separator, which is what makes
# `set-option -a` join with a comma and a malformed value get rejected.
#
# `tree-mode-preview-format` and `switch-mode-match-style` are WINDOW|PANE scope,
# the rest are WINDOW only. tmux does not police the table an explicit `-p`
# writes into, so scope shows up in where the value is *read* from, not in
# whether `set-option -p` is refused: window-tree.c reads the preview format out
# of `wp->options` when previewing a pane (1486 draws that) and out of
# `w->options` when previewing a window.
$TM show-options -wg tree-mode-preview-format
$TM show-options -wg tree-mode-preview-style
$TM show-options -wg tree-mode-border-style
$TM show-options -wg tree-mode-selection-style
$TM show-options -wg switch-mode-match-style

# They are window options, so the server table does not have them.
$TM show-options -g tree-mode-border-style
$TM show-options -sg tree-mode-border-style

# ...and they are listed with the rest of the window options.
$TM show-options -wg | grep -c '^tree-mode-'

# Round-trip a value on the global window table and on one window.
$TM new-window -d -n tm 'sleep 300'
$TM set-option -wg tree-mode-border-style 'fg=red,bg=black'
$TM show-options -wg tree-mode-border-style
$TM set-option -w -t tm tree-mode-border-style 'fg=blue'
$TM show-options -w -t tm tree-mode-border-style
$TM show-options -wg tree-mode-border-style

# The "," separator: -a appends with it.
$TM set-option -a -w -t tm tree-mode-border-style 'bright'
$TM show-options -w -t tm tree-mode-border-style
$TM set-option -u -w -t tm tree-mode-border-style
$TM show-options -w -t tm tree-mode-border-style

# IS_STYLE rejects a value that is not a style...
$TM set-option -wg tree-mode-selection-style 'not-a-style'
$TM set-option -wg tree-mode-border-style 'fg=nosuchcolour'
# ...while the format option takes any string.
$TM set-option -wg tree-mode-preview-format 'not-a-style'
$TM show-options -wg tree-mode-preview-format
$TM set-option -wg tree-mode-preview-format ''
$TM show-options -wg tree-mode-preview-format
$TM set-option -u -wg tree-mode-preview-format
$TM show-options -wg tree-mode-preview-format

# The per-pane table takes them, and reads back from it.
$TM set-option -p -t tm tree-mode-preview-format '#{window_index}'
$TM show-options -p -t tm tree-mode-preview-format
$TM set-option -p -t tm switch-mode-match-style 'fg=red'
$TM show-options -p -t tm switch-mode-match-style
$TM set-option -u -p -t tm tree-mode-preview-format

# The selection style defaults to whatever mode-style is, through #{E:}.
$TM set-option -u -wg tree-mode-selection-style
$TM show-options -wg tree-mode-selection-style
$TM display-message -t tm -p '#{E:tree-mode-selection-style}'
$TM set-option -g mode-style 'fg=cyan,bg=black'
$TM display-message -t tm -p '#{E:tree-mode-selection-style}'

# The preview style is a format too: it picks a colour from the item being
# previewed, so expanding it against a live pane takes the pane_active branch.
$TM display-message -t tm -p '#{E:tree-mode-preview-style}'
$TM display-message -t tm -p '#{E:tree-mode-preview-format}'
