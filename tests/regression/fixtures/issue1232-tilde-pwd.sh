# rubash#1232: tilde expansion of the ~+ / ~- family (tilde.c
# tilde_expand_word): bare forms take $PWD/$OLDPWD, ~+N counts the
# directory stack from the left (current dir is element 0), ~-N counts
# from the right, out-of-range indexes and unset PWD/OLDPWD leave the
# word literal; the forms also expand in assignment position and with
# / or : tails.
chk() { if [ "$1" = "$2" ]; then echo "ok $3"; else echo "BAD $3: [$1] != [$2]"; fi; }
cd /
cd /usr
x=~+;  chk "$x" /usr plus
x=~-;  chk "$x" / minus
x=~+0; chk "$x" /usr plus0
x=~-0; chk "$x" /usr minus0
x=~+1; chk "$x" "~+1" plus1-literal
x=~+/x; chk "$x" /usr/x plus-slash
x=~/x;  chk "$x" "$HOME/x" home-slash
pushd /bin >/dev/null
x=~+1; chk "$x" /usr stack-plus1
x=~-1; chk "$x" /usr stack-minus1
x=~-0; chk "$x" / stack-minus0
popd >/dev/null
w=first:~+:/last; chk "$w" "first:/usr:/last" colon-tail
unset OLDPWD
x=~-; chk "$x" "~-" minus-unset-literal
