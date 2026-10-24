#!/usr/bin/env bash
# Whitespace-padded numeric operands (rubash#1339): GNU get_exitstat /
# get_numeric_arg (builtins/common.c:488) accept any string general.c:248
# valid_number accepts — strtoimax skips leading isspace, one optional
# sign, digits, then only trailing isspace. exit/return/shift/break/
# continue all share that rule; "1 2", "1x", "" stay errors whose text
# quotes the operand verbatim.
# Capture (run from an EMPTY scratch copy of this directory):
#   env -i PATH=/usr/bin:/bin HOME=<scratch> THIS_SH=bash \
#     /path/to/gnu-bash issue1339-numeric-padding.sh \
#     > issue1339-numeric-padding.gnu.out 2> issue1339-numeric-padding.gnu.err
f() { shift "1 "; echo "shift1 rc=$? n=$#"; }
f a b
f() { shift " 1"; echo "shift2 rc=$? n=$#"; }
f a b
f() { shift "+1 "; echo "shift3 rc=$? n=$#"; }
f a b
f() { shift "1x"; echo "shift4 rc=$? n=$#"; }
f a b
g() { return "1 "; }
g; echo "return1 rc=$?"
g() { return $' 2\t'; }
g; echo "return2 rc=$?"
( exit "1 " ); echo "exit1 rc=$?"
( exit " 3 " ); echo "exit2 rc=$?"
i=0; while [ $i -lt 3 ]; do i=$((i+1)); break "1 "; done; echo "break rc=$? i=$i"
i=0; while [ $i -lt 3 ]; do i=$((i+1)); continue "1 "; echo never; done; echo "continue rc=$? i=$i"
echo done
