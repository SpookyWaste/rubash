#!/usr/bin/env bash
# Inherited BASH_ARGV0 must reach a spawned bash child (rubash#1338):
# GNU variables.c:1547-1555 (set_argv0) — a BASH_ARGV0 that crosses the
# environment boundary (exported, or a prefix assignment) becomes the
# child's $0 and $BASH_ARGV0. The child here is the `bash` found on
# PATH (in the capture environment, the oracle shell itself). Rubash's
# spawn builder used to fabricate a $0 operand (the typed command
# word) for `bash -c`, which outranks the import in the child and
# clobbered both values with the resolved bash path.
# Capture (run from an EMPTY scratch copy of this directory):
#   env -i PATH=/usr/bin:/bin HOME=<scratch> THIS_SH=bash \
#     /path/to/gnu-bash issue1338-bash-argv0.sh \
#     > issue1338-bash-argv0.gnu.out 2> issue1338-bash-argv0.gnu.err
BASH_ARGV0=argv0hi bash -c 'echo "temp: x0=[$0] av=[$BASH_ARGV0]"'
export BASH_ARGV0=exported0
bash -c 'echo "exported: x0=[$0] av=[$BASH_ARGV0]"'
unset BASH_ARGV0
bash -c 'echo "plain: x0=[$0] av=[$BASH_ARGV0]"'
echo done
