echo "case-fn"
f() { echo in-f; x=$((1/0)); echo f-after; }
f; echo "after-f rc=$?"
echo "case-if"
if [ $((1/0)) -eq 1 ]; then echo THEN; else echo ELSE; fi; echo "after-if rc=$?"
echo "case-list"
x=$((1/0)); echo SHOULD-NOT-PRINT
echo "next rc=$?"
echo "case-group"
{ echo in-grp; y=$((1/0)); echo grp-after; }; echo "after-grp rc=$?"
echo "case-and"
x=$((1/0)) && echo no2; echo "after-and rc=$?"
echo "case-comsub"
y=$(echo inner; echo $((1/0))); echo "comsub rc=$? y=[$y]"
echo "case-assign"
z=$((1/0))
echo "after-assign rc=$?"
echo "case-while"
while [ $((1/0)) -eq 1 ]; do echo LOOP; break; done; echo "after-while rc=$?"
echo "case-for-arith"
for ((i=0;i<$((1/0));i++)); do echo AF; done; echo "after-for rc=$?"
echo done
