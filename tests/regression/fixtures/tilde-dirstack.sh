mkdir -p da db dc
cd da || exit 1
pushd ../db >/dev/null
pushd ../dc >/dev/null
p0=~+0; p1=~+1; p2=~+2
m0=~-0; m1=~-1; m2=~-2
echo "plus: ${p0##*/} ${p1##*/} ${p2##*/}"
echo "minus: ${m0##*/} ${m1##*/} ${m2##*/}"
bp=~+; bm=~-
echo "bare: ${bp##*/} ${bm##*/}"
s1=~+1/sub; s2=~-0/sub
t1=${s1%/*}; t2=${s2%/*}
echo "suffix: ${t1##*/}=>${s1##*/} ${t2##*/}=>${s2##*/}"
a=~+1; b=~-2
echo "assign: ${a##*/} ${b##*/}"
echo "oob: [~+9] [~-9] [~1abc]"
d1=~1; d2=~2
echo "digits: ${d1##*/} ${d2##*/}"
v=~+1:~-1
v1=${v%%:*}; v2=${v##*:}
echo "colon: ${v1##*/} ${v2##*/}"
popd >/dev/null; popd >/dev/null
echo done
