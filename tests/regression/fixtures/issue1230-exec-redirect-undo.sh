# rubash#1230: redirections on a bare `exec` run in an undo scope
# while the exec builtin parses its options (redir.c do_redirections):
# the option diagnostics land in the redirected targets, but a failed
# exec (EX_BADUSAGE) undoes them — the shell keeps its own stdout.
exec --foo >cap1 2>cap2; echo "after-badopt rc=$?"
echo "cap2:"; cat cap2
exec --help >cap3 2>cap4; echo "after-help rc=$?"
echo "cap3 first line:"; head -1 cap3
echo "cap4 size: $(cat cap4 | wc -c)"
( exec >keep; echo "kept line" )
echo "keep:"; cat keep
echo "stdout still mine"
