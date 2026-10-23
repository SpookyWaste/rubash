#!/usr/bin/env bash
# rubash#1422: a non-fatal word-expansion error (substring expression
# < 0) inside [[ ]] reports the diagnostic, fails the conditional, and
# abandons the rest of the current command list (subst.c
# expand_wdesc_error -> DISCARD), exactly as in a simple command.
# Goldens from GNU bash 5.3.20.
a=(x y z w)
[[ "${a[@]:1:-1}" == "y z" ]]; echo "m=$?"
echo "after=$?"
[[ "${a[@]:1:-1}" == "y z" ]] && echo YES
echo "rc=$?"
