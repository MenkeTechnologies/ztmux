# save-buffer writes a buffer to a file and load-buffer reads one back
# (cmd-save-buffer.c:40, args 1,1 -- exactly one path). The round trip must
# preserve the bytes, and `-` means stdout.
d=$(mktemp -d)
$TM set-buffer -b src 'line one
line two'
$TM save-buffer -b src "$d/out.txt"; echo "save rc=$?"
echo "== file contents =="; cat "$d/out.txt"
$TM load-buffer -b back "$d/out.txt"; echo "load rc=$?"
echo "== reloaded buffer =="; $TM show-buffer -b back
echo "== save to stdout =="; $TM save-buffer -b src -
command rm -rf "$d"
