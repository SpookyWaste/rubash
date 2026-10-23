#!/usr/bin/env bash
# rubash#1261/#1262/#1269/#1271: expansion-layer fixes.
#  - #1261: ${v#pat}/${v%pat} pattern lengths count CHARACTERS of the
#    decoded value; a raw byte carried as one marker char is one
#    character even though its decoded form is a single byte.
#  - #1262: printf leaves an unrecognized escape as a literal
#    backslash + character (subst.c/printf.def format walk).
#  - #1269: with zero positional parameters, "${@@A}"/"${*@A}" expand
#    like $@/$* themselves: quoted @ -> zero words, quoted * -> one
#    empty word, unquoted -> zero words.
#  - #1271: a malformed @ transform tail (`@Q@L`, `@@Q`) with the
#    variable SET is a bad substitution error; with it UNSET the
#    expansion is silently empty.
# Verified against GNU Bash 5.3.20 (owner-built oracle) on 2026-10-24.
# Capture:
#   cd <scratch> && THIS_SH=<gnu-bash> <gnu-bash> k4-expansion-fixes.sh \
#     > k4-expansion-fixes.gnu.out 2> k4-expansion-fixes.gnu.err

echo "== 1261 byte-carried char removal"
v=$(printf '\377')
echo "vlen=${#v}"
r=${v#?}
echo "prefix-rem len=${#r}"
r=${v%?}
echo "suffix-rem len=${#r}"
s=${v:0:1}
if [ "$s" = "$v" ]; then echo "substr is whole"; else echo "substr differs"; fi
echo "== 1262 printf unrecognized escape"
printf '<\%s>\n' 'a b'
printf '<\%s>\n' one two
echo "== 1269 zero-param @A transforms"
set --
show() { echo "argc=$#"; }
show "${@@A}"
show "${*@A}"
show ${@@A}
show ${*@A}
set -- 'a b'
show "${@@A}"
echo "== 1271 malformed transform, set var"
err=$("$THIS_SH" -c 'v="x y"; echo "${v@Q@L}"' 2>&1 >/dev/null)
echo "rc=$?"
echo "err:${err##*: line *: }"
echo "== 1271 malformed transform, unset var"
"$THIS_SH" -c 'unset v; echo "[${v@Q@L}] ok"'
echo "rc=$?"
echo "== 1271 valid transform still works"
v='x y'
echo "${v@Q}"
