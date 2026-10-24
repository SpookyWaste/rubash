umask 022
echo "cur: $(umask)"
echo "sym: $(umask -S)"
echo "reuse: $(umask -p)"
x=$(umask 077; umask); echo "in-sub: $x"
echo "after-sub: $(umask)"
echo "padded: $(umask 0; umask)"
echo "symset: $(umask -S u=rwx,go=; umask)"
echo "after-all: $(umask)"
echo done
