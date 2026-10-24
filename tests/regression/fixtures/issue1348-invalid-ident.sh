#!/usr/bin/env bash
# Invalid-identifier diagnostics (rubash#1348): GNU setattr.def reports
# a bad assignment operand through sh_invalidid on the NAME portion —
# the word up to the `=` — so `export 'a[0]=x'` complains about `a[0]',
# not the whole `a[0]=x'.
# Capture (run from an EMPTY scratch copy of this directory):
#   env -i PATH=/usr/bin:/bin HOME=<scratch> THIS_SH=bash \
#     /path/to/gnu-bash issue1348-invalid-ident.sh \
#     > issue1348-invalid-ident.gnu.out 2> issue1348-invalid-ident.gnu.err
export 'a[0]=x'; echo "export1 rc=$?"
export '9bad=v'; echo "export2 rc=$?"
export '9x[0]=v'; echo "export3 rc=$?"
readonly 'r[0]=x'; echo "readonly1 rc=$?"
readonly '9bad=v'; echo "readonly2 rc=$?"
echo done
