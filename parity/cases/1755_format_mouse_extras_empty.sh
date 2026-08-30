# The remaining mouse formats have nothing to report outside a mouse key
# binding, and expand to nothing rather than to a placeholder.
$TM display-message -p 'pane=[#{mouse_pane}] hyperlink=[#{mouse_hyperlink}] range=[#{mouse_status_range}]'
$TM display-message -p 'in a conditional: #{?#{mouse_pane},set,unset}/#{?#{mouse_status_range},set,unset}'
