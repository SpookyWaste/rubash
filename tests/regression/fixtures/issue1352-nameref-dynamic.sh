#!/usr/bin/env bash
# Nameref reads of dynamic variables (rubash#1352): GNU
# find_variable_nameref hands the resolved target to the ordinary
# variable read, whose dynamic get-hooks fire for the target — a
# nameref to RANDOM/SECONDS/BASHPID reads the dynamic value (set,
# non-empty), not the target's never-bound stored cell. Only set-ness
# is probed; the values themselves are nondeterministic.
# Capture (run from an EMPTY scratch copy of this directory):
#   env -i PATH=/usr/bin:/bin HOME=<scratch> THIS_SH=bash \
#     /path/to/gnu-bash issue1352-nameref-dynamic.sh \
#     > issue1352-nameref-dynamic.gnu.out 2> issue1352-nameref-dynamic.gnu.err
declare -n r=RANDOM
echo "random-set=${r:+yes} len-nonzero=$([ ${#r} -gt 0 ] && echo yes || echo no)"
declare -n s=SECONDS
echo "seconds-set=${s:+yes}"
declare -n p=BASHPID
echo "bashpid-set=${p:+yes}"
declare -n chain=r
echo "chain-set=${chain:+yes}"
declare -n plain=UNSET_VARIABLE_XYZ
echo "unset-target=${plain:+yes}empty-ok"
echo done
