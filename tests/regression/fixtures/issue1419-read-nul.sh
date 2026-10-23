#!/usr/bin/env bash
# rubash#1419: GNU read (builtins/read.def) skips NUL bytes as the
# record is read: they never reach the assigned values, the field
# splitting, or the -a array. Goldens from GNU bash 5.3.20.
printf 'a\0b\n' | { read -r v; echo "len=${#v}"; printf '%s' "$v" | od -An -tx1; }
printf 'x\0y z\0w\n' | { read -r a b; printf '%s' "$a" | od -An -tx1; printf '%s' "$b" | od -An -tx1; }
printf 'p\0q\0r\n' | { read v; echo "multi len=${#v}"; }
printf 'a\0b\n' > nulinput.txt
{ read -r v < nulinput.txt; echo "file len=${#v}"; }
printf 'a\0b\n' | { read -r -a arr; echo "arrn=${#arr[@]}"; printf '%s' "${arr[0]}" | od -An -tx1; }
printf 'a\0b\n' | { IFS= read -r v; echo "ifs len=${#v}"; }
