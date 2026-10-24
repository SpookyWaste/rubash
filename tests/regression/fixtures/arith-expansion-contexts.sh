for i in $((1/0)); do echo iter; done
echo "for rc=$?"
case $((1/0)) in *) echo matched;; esac
echo "case rc=$?"
echo plain $((1/0))
echo "plain rc=$?"
echo done
