# The %if condition is a format string, not a literal: it is expanded and then
# tested for truth, so comparisons and arithmetic decide the branch.
cfg="${TMPDIR:-/tmp}/ztpar_cfg_if_fmt.conf"
cat > "$cfg" <<'CFG'
%if #{==:abc,abc}
set -g @eq yes
%endif
%if #{==:abc,xyz}
set -g @ne yes
%else
set -g @ne no
%endif
%if #{e|>|:3,2}
set -g @gt yes
%endif
CFG
$TM source-file "$cfg"; echo "rc=$?"
$TM show -gv @eq
$TM show -gv @ne
$TM show -gv @gt
command rm -f "$cfg"
