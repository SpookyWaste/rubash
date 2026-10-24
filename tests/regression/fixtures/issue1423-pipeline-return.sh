#!/usr/bin/env bash
# rubash#1423: a pipeline element runs in a forked subshell
# (execute_cmd.c execute_pipeline), so `return` inside a brace group
# element only exits the element: its status becomes the element's
# status and the enclosing function continues. Goldens from GNU bash
# 5.3.20.
f() { echo hi | { echo INNER; return 0; }; echo "after=$?"; }
f; echo "outer=$?"
g() { echo hi | { read x; return 3; }; echo "g-after=$?"; }
g; echo "g-outer=$?"
