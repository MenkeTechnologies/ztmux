# The t modifier renders a time-valued variable: /f takes a strftime format, /p
# is the pretty form and /r the relative age. The values are wall clocks, so
# only the shapes and the parts that cannot change during a run are compared.
$TM display-message -p '#{t/f/%Y-%m-%d:session_created}'
$TM display-message -p '#{t/f/%H:session_created}' | perl -pe 's/^\d\d$/HH-SHAPE-OK/'
$TM display-message -p '#{t/p:session_created}' | perl -pe 's/^\d\d:\d\d$/PRETTY-SHAPE-OK/'
$TM display-message -p '#{t/r:session_created}' | perl -pe 's/^\d+[smhd][0-9smhd]*$/RELATIVE-SHAPE-OK/'
echo "== the default rendering =="
$TM display-message -p '#{t:session_created}' | perl -pe 's/\d\d:\d\d:\d\d/HH:MM:SS/; s/\b\d{4}\b/YYYY/; s/\b\d+\b/N/g'
echo "== a variable that holds no time =="
$TM display-message -p '[#{t:window_name}]'
$TM display-message -p '[#{t/f/%Y:nosuchvariable}]'
