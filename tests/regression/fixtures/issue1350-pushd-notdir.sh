#!/usr/bin/env bash
# pushd operand diagnostics (rubash#1350): GNU pushd.def routes the
# operand through cd_builtin, whose change_to_directory (cd.def)
# reports the chdir errno text — an existing non-directory is
# "Not a directory" (ENOTDIR), a missing path is
# "No such file or directory" (ENOENT).
# Capture (run from an EMPTY scratch copy of this directory):
#   env -i PATH=/usr/bin:/bin HOME=<scratch> THIS_SH=bash \
#     /path/to/gnu-bash issue1350-pushd-notdir.sh \
#     > issue1350-pushd-notdir.gnu.out 2> issue1350-pushd-notdir.gnu.err
touch regf
mkdir realdir
pushd regf; echo "file rc=$?"
pushd missing; echo "missing rc=$?"
pushd realdir >/dev/null; echo "dir rc=$? now=${PWD##*/}"
popd >/dev/null; echo "popd rc=$?"
echo done
