# #{L:...} loops over the clients the way #{S:}/#{W:}/#{P:} loop over sessions,
# windows and panes (format.c:5389). With nothing attached the loop body never
# runs, and its sort arguments still have to parse.
$TM display-message -p 'no clients: [#{L:#{client_name} }]'
$TM display-message -p 'sorted by name: [#{L/n:#{client_name} }]'
$TM display-message -p 'reversed: [#{L/r:#{client_name} }]'
$TM display-message -p 'with a separator: [#{L/n/,:#{client_name}}]'
$TM display-message -p 'loop_index outside a loop: [#{loop_index}]'
