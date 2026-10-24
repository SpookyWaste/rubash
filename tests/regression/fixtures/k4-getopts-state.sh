#!/usr/bin/env bash
# rubash#1268 + rubash#1270: getopts state handling per GNU
# builtins/getopts.def dogetopts + variables.c attributes.
#  - OPTIND is integer-attributed (`declare -i`) from shell start.
#  - A readonly OPTIND refuses each write with `OPTIND: readonly
#    variable` (once per getopts call) while the option scan keeps its
#    own counter and still yields the options in order.
#  - Binding the option character into OPTIND (name == OPTIND) happens
#    AFTER the counter write; a `?`/`:` value is a fatal arithmetic
#    error for the call (status 1, script continues, OPTARG untouched).
# Verified against GNU Bash 5.3.20 (owner-built oracle) on 2026-10-24.
# Capture:
#   cd <scratch> && THIS_SH=<gnu-bash> <gnu-bash> k4-getopts-state.sh \
#     > k4-getopts-state.gnu.out 2> k4-getopts-state.gnu.err

echo "== OPTIND attribute"
declare -p OPTIND
echo "== readonly OPTIND loop"
set -- -a -b
readonly OPTIND
while getopts "ab" opt; do
  echo "got $opt OPTIND=$OPTIND"
done
echo "rc=$?"
unset opt
echo "== name is OPTIND, unknown option"
set -- -z
OPTIND=1
while getopts "a" OPTIND; do
  echo "got opt=[$OPTIND] OPTARG=[$OPTARG]"
done
echo "rc=$? OPTIND=[$OPTIND] OPTARG=[$OPTARG]"
echo "== name is OPTIND, missing argument"
set -- -a
OPTIND=1
while getopts ":a" OPTIND; do
  echo "got opt=[$OPTIND] OPTARG=[$OPTARG]"
done
echo "rc=$? OPTIND=[$OPTIND]"
