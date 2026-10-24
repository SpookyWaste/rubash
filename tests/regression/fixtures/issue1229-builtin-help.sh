# rubash#1229: every builtin's getopt table maps --help in option
# position to CASE_HELPOPT: the long help goes to stdout and the
# status is EX_USAGE (2). printf already behaved this way; the rest of
# the family printed "invalid option" instead.
for b in source eval command builtin type hash; do $b --help >/dev/null 2>&1; echo "$b rc=$?"; done
( exec --help >/dev/null 2>&1; echo "exec rc=$?" )
printf --help >/dev/null 2>&1; echo "printf rc=$?"
source --help 2>/dev/null | head -1
type --help 2>/dev/null | head -1
hash --help 2>/dev/null | head -1
eval --help 2>/dev/null | head -1
command --help 2>/dev/null | head -1
builtin --help 2>/dev/null | head -1
exec --help 2>/dev/null | head -1
