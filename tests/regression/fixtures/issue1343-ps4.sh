#!/usr/bin/env bash
# PS4 rendering (rubash#1343): GNU print_cmd.c indirection_level_string
# reads PS4 through get_string_value — unset or empty PS4 renders an
# EMPTY prefix (the "+ " text is only the startup default binding) —
# and runs the value through decode_prompt_string, so prompt escapes
# are decoded and the expansion pass still applies afterwards.
# Capture (run from an EMPTY scratch copy of this directory):
#   env -i PATH=/usr/bin:/bin HOME=<scratch> THIS_SH=bash \
#     /path/to/gnu-bash issue1343-ps4.sh \
#     > issue1343-ps4.gnu.out 2> issue1343-ps4.gnu.err
PS4='\\ '
set -x
echo one
PS4='$((6*7))| '
echo two
PS4=''
echo three
unset PS4
echo four
set +x
echo done
