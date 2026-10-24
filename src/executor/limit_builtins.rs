use super::*;

impl Executor {
    pub(in crate::executor) fn execute_kill(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        if let Some(status) = self.execute_tracked_background_kill(cmd)? {
            return Ok(status);
        }

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = crate::builtins::kill::execute_with_io_posix(
            &cmd.words[1..],
            &mut stdout,
            &mut stderr,
            self.posix_mode_enabled(),
        )?;
        self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
        Ok(status)
    }

    fn execute_tracked_background_kill(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<Option<i32>, ExecuteError> {
        let Some(request) = kill_request(&cmd.words[1..]) else {
            return Ok(None);
        };
        if request.operands.is_empty() {
            return Ok(None);
        }

        let should_handle = request.operands.iter().any(|operand| {
            operand.starts_with('%')
                || operand
                    .parse::<u32>()
                    .ok()
                    .is_some_and(|pid| self.shell_state.job_table.pid_to_job.contains_key(&pid))
        });
        if !should_handle {
            let mut stderr = Vec::new();
            // GNU kill.def:87-268 counters — see the tracked loop below:
            // probe/delivery attempts decide any_succeeded/any_failed,
            // spec diagnostics feed neither.
            let mut any_succeeded = false;
            let mut any_failed = false;
            for operand in request.operands {
                let Some(pid) = operand.parse::<u32>().ok() else {
                    // Not a u32 pid or job spec — fall through to kill::execute_with_io
                    // for negative pids (Unix process groups) and other formats.
                    return Ok(None);
                };
                if request.check_only && pid == 0 {
                    // GNU kill(0, 0) probes the shell's own group and
                    // succeeds — it counts as a delivered probe.
                    any_succeeded = true;
                    continue;
                }
                if !process_exists(pid) {
                    writeln!(
                        stderr,
                        "{}kill: ({pid}) - No such process",
                        self.diagnostic_prefix()
                    )?;
                    any_failed = true;
                } else {
                    any_succeeded = true;
                }
            }
            self.write_buffered_builtin_output(cmd, &[], &stderr)?;
            if any_failed || request.check_only {
                return Ok(Some(if self.posix_mode_enabled() {
                    i32::from(any_failed)
                } else {
                    i32::from(!any_succeeded)
                }));
            }
            return Ok(None);
        }

        let mut stderr = Vec::new();
        // GNU kill.def:87-268: only real kill_pid attempts move the
        // counters — any_succeeded for a delivered signal, any_failed for
        // a failed delivery (ESRCH etc.). Spec-layer diagnostics
        // (sh_badjob/sh_badpid/DUP_JOB via CONTINUE_OR_FAIL) touch
        // neither counter, so `kill -0 %1 %?dup` returns 0 while
        // `kill -0 %?dup` alone returns 1. The final status is
        // `any_succeeded ? 0 : 1` (posix: `any_failed == 0 ? 0 : 1`).
        let mut any_succeeded = false;
        let mut any_failed = false;
        for operand in request.operands {
            let (pid, tracked_job) = match self.resolve_background_job_operand(&operand) {
                super::job_builtins::JobOperandResolution::Pid(pid) => (pid, true),
                super::job_builtins::JobOperandResolution::Ambiguous => {
                    // GNU kill.def:226-232 + builtins/common.c:672-679:
                    // DUP_JOB is INVALID_JOB, but kill skips sh_badjob
                    // for it — the get_job_by_name diagnostic is the
                    // whole report ("kill: name: ambiguous job spec").
                    writeln!(
                        stderr,
                        "{}kill: {}: ambiguous job spec",
                        self.diagnostic_prefix(),
                        super::job_builtins::jobspec_match_name(&operand)
                    )?;
                    continue;
                }
                super::job_builtins::JobOperandResolution::NoMatch => {
                    if operand.starts_with('%') {
                        // kill.def:227-229 sh_badjob — message only.
                        writeln!(
                            stderr,
                            "{}kill: {operand}: no such job",
                            self.diagnostic_prefix()
                        )?;
                        continue;
                    }
                    match operand.parse::<u32>() {
                        // kill.def:194-216: a numeric operand takes the
                        // pid path even outside the jobs table — the
                        // delivery below decides the counters.
                        Ok(pid) => (pid, false),
                        Err(_) => {
                            // kill.def:205-216 sh_badpid — message only.
                            writeln!(
                                stderr,
                                "{}kill: `{operand}': not a pid or valid job spec",
                                self.diagnostic_prefix()
                            )?;
                            continue;
                        }
                    }
                }
            };

            if request.check_only {
                // GNU kill -0 is a real liveness probe (kill(2) with sig 0
                // returns ESRCH for a dead pid), not an unconditional yes.
                // A pid the job table already marked Completed is dead even
                // before the process object is reaped — except a live
                // procsub placeholder (rubash#1022): its status is recorded
                // at registration while the forked zombie stays
                // unreaped (signallable) until an explicit wait or sweep,
                // exactly like GNU's just-exited procsub.
                let job_dead = self
                    .shell_state
                    .job_table
                    .completed_statuses
                    .contains_key(&pid)
                    && !self
                        .shell_state
                        .job_table
                        .procsub_placeholders
                        .contains(&pid);
                if job_dead || !process_exists(pid) {
                    writeln!(
                        stderr,
                        "{}kill: ({pid}) - No such process",
                        self.diagnostic_prefix()
                    )?;
                    any_failed = true;
                } else {
                    any_succeeded = true;
                }
                continue;
            }

            let signal_args = vec![format!("-{}", request.signal), pid.to_string()];
            let mut signal_stderr = Vec::new();
            let signal_status = crate::builtins::kill::execute_with_io(
                &signal_args,
                &mut std::io::sink(),
                &mut signal_stderr,
            )?;
            if signal_status != 0 {
                any_failed = true;
                stderr.extend(signal_stderr);
                continue;
            }
            any_succeeded = true;
            if !tracked_job {
                // A bare numeric pid outside the jobs table has no
                // job-table side effects (GNU kill_pid is a plain
                // signal delivery).
                continue;
            }
            // GNU: the shell learns that a background job stopped from
            // waitchld's WUNTRACED report, which only exists under job
            // control — a non-monitor shell never notices, so `jobs` keeps
            // reporting the job as Running even after the shell's OWN kill
            // sent SIGSTOP (nojobs.c; rubash#1247). Rubash has no WUNTRACED
            // poller, so the kill path's marking is its only stop detector;
            // gate it on monitor mode or a `set +m` shell would report its
            // jobs Stopped where GNU reports Running.
            //
            // rubash#1494: GNU kill_pid is pure delivery — the waitchld
            // report lands at a notification point observably ONE reader
            // command later (an immediate `jobs` still shows the
            // pre-signal state and +/- marks; one intervening command
            // lets the new state appear). Defer the mark instead of
            // applying it synchronously inside the kill.
            if crate::builtins::set::shell_option_enabled(&self.shell_state.env_vars, "monitor") {
                if request.signal == 18 {
                    self.shell_state
                        .job_table
                        .defer_state_mark(pid, crate::jobs::table::DeferredJobState::Running);
                } else if matches!(request.signal, 19 | 20) {
                    self.shell_state.job_table.defer_state_mark(
                        pid,
                        crate::jobs::table::DeferredJobState::Stopped(Some(request.signal)),
                    );
                }
            }
            // A terminating signal changes nothing in the table here:
            // the boundary reap marks the job Completed when the process
            // actually dies. rubash#796 used to mark + remove the entry
            // at delivery time for the DONT_REPORT signals (TERM/INT/
            // PIPE), conflating the suppressed reader-boundary NOTICE
            // (report_pending_signal_notices already stays silent for
            // background TERM deaths) with the job's lifetime — GNU
            // keeps the job, and the next `jobs` lists it once as
            // `Terminated` before cleaning it up (rubash#1256). The old
            // code also lied for a process that ignores TERM: it
            // vanished from the table while still running.
            continue;
        }

        self.write_buffered_builtin_output(cmd, &[], &stderr)?;
        // kill.def:265-268: posixly_correct fails on any_failed, the
        // default mode on no successes at all — spec diagnostics feed
        // neither counter (rubash#1054's jobspec exemption is the same
        // rule, only stated for % specs).
        let status = if self.posix_mode_enabled() {
            i32::from(any_failed)
        } else {
            i32::from(!any_succeeded)
        };
        Ok(Some(status))
    }

    pub(in crate::executor) fn execute_ulimit(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = crate::builtins::ulimit::execute_with_io(
            &cmd.words[1..],
            &mut self.shell_state.env_vars,
            &mut stdout,
            &mut stderr,
        )?;
        self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
        Ok(status)
    }
}

#[cfg(unix)]
fn process_exists(pid: u32) -> bool {
    crate::builtins::kill::process_exists(pid)
}

#[cfg(windows)]
fn process_exists(pid: u32) -> bool {
    crate::builtins::kill::process_exists(pid)
}

#[cfg(not(any(unix, windows)))]
fn process_exists(_pid: u32) -> bool {
    false
}

struct KillRequest {
    operands: Vec<String>,
    signal: i32,
    check_only: bool,
}

fn kill_request(words: &[String]) -> Option<KillRequest> {
    let mut index = 0;
    let mut signal = 15;
    let mut check_only = false;
    while let Some(word) = words.get(index) {
        if word == "--" {
            index += 1;
            break;
        }
        if word == "-l" || word == "--list" {
            return None;
        }
        if word == "-s" || word == "-n" {
            if words
                .get(index + 1)
                .is_none_or(|signal| crate::builtins::kill::translate_signal(signal).is_none())
            {
                return None;
            }
            let Some(sigspec) = words.get(index + 1) else {
                return None;
            };
            signal = crate::builtins::kill::signal_number_for_spec(sigspec)?;
            check_only |= signal == 0;
            index += 2;
            continue;
        }
        if word.starts_with('-') && word != "-" {
            if word != "-0"
                && crate::builtins::kill::translate_signal(word.trim_start_matches('-')).is_none()
            {
                return None;
            }
            signal = crate::builtins::kill::signal_number_for_spec(word.trim_start_matches('-'))?;
            check_only |= signal == 0;
            index += 1;
            continue;
        }
        break;
    }

    Some(KillRequest {
        operands: words[index..].to_vec(),
        signal,
        check_only,
    })
}
