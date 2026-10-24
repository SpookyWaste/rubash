//! suspend module.
//!
//! GNU Bash source ownership:
//! - builtins/suspend.def suspend_builtin (`killpg (shell_pgrp, SIGSTOP)`
//!   on -f; `sh_nojobs` failure without job control otherwise)

use std::io::{self, Write};

const EXECUTION_SUCCESS: i32 = 0;
const EXECUTION_FAILURE: i32 = 1;
const EX_USAGE: i32 = 2;

/// Returns the builtin status plus a `too_many_arguments` flag. GNU
/// suspend.def reports an operand with `suspend: too many arguments` —
/// the builtins/common.c no_args shape, whose DISCARD jump (exit class,
/// rubash#705) the Executor arms when it sees the flag: the rest of the
/// current list is abandoned with $?=2, and under `-c` the shell exits 1.
/// A bad OPTION is different: plain EX_USAGE and the shell continues.
pub fn execute_with_io<E>(
    args: &[String],
    diagnostic_prefix: &str,
    stderr: &mut E,
) -> io::Result<(i32, bool)>
where
    E: Write,
{
    let mut force = false;
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if arg == "--" {
            index += 1;
            break;
        }
        if !arg.starts_with('-') || arg == "-" {
            break;
        }
        for option in arg[1..].chars() {
            match option {
                'f' => force = true,
                other => {
                    writeln!(
                        stderr,
                        "{diagnostic_prefix}suspend: -{other}: invalid option"
                    )?;
                    writeln!(stderr, "suspend: usage: suspend [-f]")?;
                    return Ok((EX_USAGE, false));
                }
            }
        }
        index += 1;
    }

    if index < args.len() {
        writeln!(stderr, "{diagnostic_prefix}suspend: too many arguments")?;
        return Ok((EX_USAGE, true));
    }

    if force {
        // GNU builtins/suspend.def suspend_builtin: with -f the login-shell
        // and no-job-control guards are skipped and the shell stops its own
        // process group — `killpg (shell_pgrp, SIGSTOP)` — returning SUCCESS
        // once continued (rubash#1097). shell_pgrp is this process's group
        // (jobs.c:4737 `shell_pgrp = getpgid (0)`); SIGSTOP/SIGCONT ride the
        // kernel defaults (never in kill.rs's managed set), so no trap
        // bookkeeping is needed. Non-unix has no SIGSTOP: keep the
        // historical success no-op there.
        #[cfg(unix)]
        {
            // SAFETY: getpgid/killpg with our own pgrp are pure queries.
            let pgrp = unsafe { libc::getpgid(0) };
            if pgrp > 0 {
                unsafe {
                    libc::killpg(pgrp, libc::SIGSTOP);
                }
                // rubash#1474: the group stop is process-directed — in
                // this multithreaded executor another thread can field it
                // first, and the executing thread then runs several more
                // commands before the stop lands on it (observed: a whole
                // `e1..e5; sleep` tail executed after `suspend -f`). GNU's
                // single-threaded killpg (builtins/suspend.def
                // suspend_builtin) stops the shell before the next command
                // runs. Stop THIS thread thread-directed as well: a
                // pending SIGSTOP on the calling thread takes effect on
                // the tgkill return-to-userspace path, before any further
                // command executes. On SIGCONT both stops release and the
                // builtin returns success as before.
                let pid = unsafe { libc::getpid() };
                let tid = unsafe { libc::syscall(libc::SYS_gettid) } as libc::pid_t;
                unsafe {
                    libc::syscall(libc::SYS_tgkill, pid, tid, libc::SIGSTOP);
                }
            }
        }
        return Ok((EXECUTION_SUCCESS, false));
    }

    writeln!(
        stderr,
        "{diagnostic_prefix}suspend: cannot suspend: no job control"
    )?;
    Ok((EXECUTION_FAILURE, false))
}
