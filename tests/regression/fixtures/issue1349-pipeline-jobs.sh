#!/usr/bin/env bash
# Job table visibility in pipeline elements (rubash#1349): GNU forks
# every pipeline element and execute_subshell_builtin_or_function calls
# without_job_control() (jobs.c delete_all_jobs), so a builtin running
# as a pipeline element sees an EMPTY job table — `compgen -A job`
# lists nothing there — while the jobs builtin itself is exempt (the
# execute_cmd.c jobs_hack) and an unpiped or command-substitution
# compgen sees the shell's jobs. Counts only: job NAME rendering is a
# separate surface.
# Capture (run from an EMPTY scratch copy of this directory):
#   env -i PATH=/usr/bin:/bin HOME=<scratch> THIS_SH=bash \
#     /path/to/gnu-bash issue1349-pipeline-jobs.sh \
#     > issue1349-pipeline-jobs.gnu.out 2> issue1349-pipeline-jobs.gnu.err
sleep 30 &
echo "piped-count=$(compgen -A job | wc -l)"
x=$(compgen -A job)
echo "comsub-set=$([ -n "$x" ] && echo yes || echo no)"
echo "jobs-piped-count=$(jobs | wc -l)"
kill %1 2>/dev/null
wait 2>/dev/null
echo done
