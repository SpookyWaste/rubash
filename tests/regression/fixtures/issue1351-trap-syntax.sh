#!/usr/bin/env bash
# Syntax error inside a trap action at fire time (rubash#1351): GNU
# runs trap actions through parse_and_execute (trap.c:496); a parse
# error in the action is a diagnostic for that action only — the shell
# survives, the script continues, and $? keeps the pre-trap value. The
# action must not unwind the whole script the way a top-level syntax
# error does.
# Capture (run from an EMPTY scratch copy of this directory):
#   env -i PATH=/usr/bin:/bin HOME=<scratch> THIS_SH=bash \
#     /path/to/gnu-bash issue1351-trap-syntax.sh \
#     > issue1351-trap-syntax.gnu.out 2> issue1351-trap-syntax.gnu.err
trap 'fi' USR1
kill -USR1 $BASHPID
echo "lived1 rc=$?"
trap 'echo trapped-ok' USR1
kill -USR1 $BASHPID
echo "lived3 rc=$?"
echo done
