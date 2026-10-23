#!/usr/bin/env bash
# rubash#438: a <(...) word handed to an external command names a
# descriptor the child actually inherits (subst.c process_substitute):
# the child can open /dev/fd/N and read the substitution output.
# Goldens from GNU bash 5.3.20.
diff <(echo a) <(echo a); echo "diff-rc=$?"
cat <(echo hello); echo "cat-rc=$?"
