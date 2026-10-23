v=42
declare -in ref2=v
echo "dcl rc=$?"
declare -p ref2 2>&1
echo "val=[$ref2] arith=$((ref2))"
ref2=plain
echo "plain rc=$? ref2=[$ref2]"
echo done
