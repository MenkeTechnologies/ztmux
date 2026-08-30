# clock-mode is a pane mode like the others: it is entered, reported by
# #{pane_mode}, and left again by cancel. The clock itself is drawn to the
# CLIENT's screen rather than into the pane's grid -- a server-side capture is
# empty for both binaries -- so what the client paints is not compared here;
# that divergence is recorded in parity/known_gaps/clock_mode_client_draw.sh.
$TM display-message -p 'before: [#{pane_mode}]'
$TM clock-mode; echo "rc=$?"
$TM display-message -p 'after clock-mode: [#{pane_mode}] in_mode=#{pane_in_mode}'
echo "the pane grid stays empty: $($TM capture-pane -p | tr -d ' \n' | wc -c | tr -d ' ')"
$TM send-keys -X cancel
$TM display-message -p 'after cancel: [#{pane_mode}] in_mode=#{pane_in_mode}'
echo "== the option it reads =="
$TM show -gwv clock-mode-style
$TM setw -g clock-mode-style 12; $TM show -gwv clock-mode-style
$TM setw -g clock-mode-style 24; $TM show -gwv clock-mode-style
$TM setw -g clock-mode-style nonsense 2>&1; echo "rc=$?"
$TM show -gwv clock-mode-colour
