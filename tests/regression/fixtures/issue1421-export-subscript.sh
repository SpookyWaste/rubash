#!/usr/bin/env bash
# rubash#1421: GNU export/readonly (builtins/setattr.def) never treat
# name[sub]=value as an element assignment: the subscript is NOT
# evaluated (no arithmetic side effects) and the operand is rejected.
# The diagnostic text itself is pinned by rubash#1347's fixture; this
# one pins the observable state only. Goldens from GNU bash 5.3.20.
i=1; a=(x y z)
export 'a[i++]=Q' 2>/dev/null; echo "rc=$? i=$i"
declare -p a
export 'a[2]=Q' 2>/dev/null; echo "numrc=$?"
readonly 'a[i++]=R' 2>/dev/null; echo "rorc=$? i=$i"
declare -p a
