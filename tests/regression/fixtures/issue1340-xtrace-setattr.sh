#!/usr/bin/env bash
# Per-assignment xtrace lines for export/readonly (rubash#1340): GNU
# setattr.def:275 performs each assignment operand through
# do_assignment_no_expand, and subst.c:3580 xtrace_print_assignment
# traces the performed bind — one extra line per assignment operand,
# after the command's own trace line, in operand order. Marking an
# existing variable (no `=`) performs no assignment and traces no extra
# line; declare/typeset/local get no such lines at all.
# Capture (run from an EMPTY scratch copy of this directory):
#   env -i PATH=/usr/bin:/bin HOME=<scratch> THIS_SH=bash \
#     /path/to/gnu-bash issue1340-xtrace-setattr.sh \
#     > issue1340-xtrace-setattr.gnu.out 2> issue1340-xtrace-setattr.gnu.err
set -x
export FOO=bar
export A=1 B=2
export B
readonly R=5
C=q
export C+=z
export E=
export SP='a b'
declare D=9
local_never=1
set +x
echo "FOO=$FOO A=$A B=$B R=$R C=$C E=[$E] SP=[$SP] D=$D"
echo done
