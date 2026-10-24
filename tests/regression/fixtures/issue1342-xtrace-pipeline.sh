#!/usr/bin/env bash
# Pipeline elements trace exactly once (rubash#1342): GNU runs every
# pipeline element through execute_simple_command in its forked child,
# which prints the element's xtrace line exactly once
# (execute_cmd.c:4649). Query builtins (set/export/declare/shopt/
# alias/umask with no assignment operands) must not print it twice.
# Stage output is discarded so only the traces are compared.
# Capture (run from an EMPTY scratch copy of this directory):
#   env -i PATH=/usr/bin:/bin HOME=<scratch> THIS_SH=bash \
#     /path/to/gnu-bash issue1342-xtrace-pipeline.sh \
#     > issue1342-xtrace-pipeline.gnu.out 2> issue1342-xtrace-pipeline.gnu.err
set -x
set -o | cat >/dev/null
export | head -1 >/dev/null
umask | cat >/dev/null
declare | head -1 >/dev/null
echo hi | cat >/dev/null
set +x
echo done
