#!/usr/bin/env bash
# Parenthesized key in a compound assignment element (rubash#1346):
# an element-leading `[` opens a subscript span that GNU
# read_token_word consumes with parse_matched_pair (P_ARRAYSUB) —
# parens inside are span data, so `[(x)]=v` assigns the literal key
# `(x)`. The parser's compound control-operator scan must skip the
# same span instead of reporting the `(` as a nested subshell.
# Capture (run from an EMPTY scratch copy of this directory):
#   env -i PATH=/usr/bin:/bin HOME=<scratch> THIS_SH=bash \
#     /path/to/gnu-bash issue1346-paren-key.sh \
#     > issue1346-paren-key.gnu.out 2> issue1346-paren-key.gnu.err
declare -A h=([a]=1)
h+=([(x)]=v)
echo "rc=$?"
declare -p h
echo "get:${h[(x)]}"
declare -A g
g=([(y)]=w [z]=q)
declare -p g
