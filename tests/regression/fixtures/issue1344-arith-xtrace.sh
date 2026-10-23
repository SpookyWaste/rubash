#!/usr/bin/env bash
# (( )) command xtrace shape (rubash#1344): GNU print_cmd.c:986
# xtrace_print_arith_cmd prints `(( ` + the expanded expression with
# its whitespace verbatim + ` ))` — the between-parens text is NOT
# trimmed, so `(( 1 + 1 ))` traces as `+ ((  1 + 1  ))` and `((1+1))`
# as `+ (( 1+1 ))`.
# Capture (run from an EMPTY scratch copy of this directory):
#   env -i PATH=/usr/bin:/bin HOME=<scratch> THIS_SH=bash \
#     /path/to/gnu-bash issue1344-arith-xtrace.sh \
#     > issue1344-arith-xtrace.gnu.out 2> issue1344-arith-xtrace.gnu.err
set -x
a=3
(( $a + 1 ))
((1+1))
(( 1+1))
((1+1 ))
((  1 + 1  ))
(( a > 2 ))
set +x
echo done
