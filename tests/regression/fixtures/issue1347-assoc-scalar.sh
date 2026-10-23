#!/usr/bin/env bash
# Scalar assignment to an associative array (rubash#1347): GNU
# variables.c:3128-3142 binds a scalar value at key "0" of the existing
# array (assign_func), preserving the other keys — the bind must not
# replace the whole array with a fresh one-element storage.
# Capture (run from an EMPTY scratch copy of this directory):
#   env -i PATH=/usr/bin:/bin HOME=<scratch> THIS_SH=bash \
#     /path/to/gnu-bash issue1347-assoc-scalar.sh \
#     > issue1347-assoc-scalar.gnu.out 2> issue1347-assoc-scalar.gnu.err
declare -A h=([a]=1 [b]=2)
h=b
echo "rc=$? n=${#h[@]} a=[${h[a]}] b=[${h[b]}] zero=[${h[0]}]"
declare -A h2
h2='([x]=9)'
echo "n2=${#h2[@]} zero2=[${h2[0]}]"
declare -A h3=([k]=v)
h3[0]=direct
echo "n3=${#h3[@]} k=[${h3[k]}] zero3=[${h3[0]}]"
echo done
