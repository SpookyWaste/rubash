#!/usr/bin/env bash
# rubash#1259: builtin option-error diagnostics must name the option
# the way GNU's builtin getopt scan reports it (builtins/*.def
# sh_invalidopt / internal_getopt order):
#  - a long-looking word is scanned after ONE dash: the first invalid
#    character decides (`--badflag` -> `--` for declare/getopts,
#    `-badflag` full word for kill's signal-spec parser).
#  - declare -f attribute conflicts report in the fixed GNU priority
#    n > i > A > a with status 1 and no usage; an invalid letter is a
#    usage error (status 2) reported before any conflict.
# Verified against GNU Bash 5.3.20 (owner-built oracle) on 2026-10-24.
# Capture:
#   cd <scratch> && THIS_SH=<gnu-bash> <gnu-bash> k4-declare-options.sh \
#     > k4-declare-options.gnu.out 2> k4-declare-options.gnu.err

echo "== declare --badflag"
declare --badflag
echo "rc=$?"
echo "== declare -badflag"
declare -badflag
echo "rc=$?"
echo "== declare -fin"
declare -fin x
echo "rc=$?"
echo "== declare -fai"
declare -fai x
echo "rc=$?"
echo "== declare -fZ"
declare -fZ x
echo "rc=$?"
echo "== getopts --badflag"
getopts --badflag
echo "rc=$?"
echo "== getopts -x missing args"
getopts -x
echo "rc=$?"
echo "== kill --badflag"
kill --badflag
echo "rc=$?"
echo "== trap whole word"
trap -badflag
echo "rc=$?"
echo "== umask whole word"
umask -badflag
echo "rc=$?"
echo "== mapfile whole word"
mapfile -badflag arr
echo "rc=$?"
