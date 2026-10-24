#!/usr/bin/env bash
# xtrace indirection depth in eval/source (rubash#1341): GNU
# builtins/evalstring.c:348 parse_and_execute increments
# indirection_level around the nested list, so commands inside eval
# and sourced files trace with the PS4 first character repeated one
# more time per nesting level than the eval/source line itself.
# Capture (run from an EMPTY scratch copy of this directory):
#   env -i PATH=/usr/bin:/bin HOME=<scratch> THIS_SH=bash \
#     /path/to/gnu-bash issue1341-xtrace-indirection.sh \
#     > issue1341-xtrace-indirection.gnu.out 2> issue1341-xtrace-indirection.gnu.err
printf 'echo from-source\n' > src.sh
set -x
eval 'echo from-eval'
eval "eval 'echo deep'"
source src.sh
. src.sh
set +x
echo done
