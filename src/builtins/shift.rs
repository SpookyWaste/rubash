//! shift module.
//!
//! GNU Bash source ownership:
// - builtins/shift.def

use std::io::{self, Write};

pub enum ShiftAction {
    Complete(i32),
    Shift(usize),
    /// Surplus operands: GNU builtins/common.c no_args() — the
    /// diagnostic, EX_BADUSAGE, then jump_to_top_level (DISCARD).
    /// Kept distinct from Complete(2) because the other EX_USAGE
    /// paths (non-numeric operand, --help) return a plain status
    /// with no discard; only the no_args class abandons the rest
    /// of the current list (rubash#1650).
    TooManyArguments,
}

/// GNU error.h: EX_USAGE (2) marks a usage error, EXECUTION_FAILURE (1) a
/// plain runtime failure. shift.def routes a non-numeric operand through
/// get_numeric_arg (builtins/common.c:501-505 `sh_neednumarg` + `return 0`
/// → `return (EX_USAGE)`) and a surplus operand through `no_args`
/// (builtins/common.c `set_exit_status (EX_BADUSAGE)`), so both exit 2;
/// only the shift-count-out-of-range erange path returns
/// EXECUTION_FAILURE.
pub(crate) const EX_USAGE: i32 = 2;

pub fn execute(args: &[String]) -> io::Result<ShiftAction> {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    execute_with_io(args, &mut stdout, &mut stderr)
}

pub(crate) fn execute_with_io<W, E>(
    args: &[String],
    stdout: &mut W,
    stderr: &mut E,
) -> io::Result<ShiftAction>
where
    W: Write,
    E: Write,
{
    if args.first().map(String::as_str) == Some("--help") {
        crate::builtins::help::print_shift_help_with_io(stdout)?;
        return Ok(ShiftAction::Complete(2));
    }

    // GNU builtins/shift.def: strip leading `--` argument terminator before
    // processing the shift count. `shift -- 5` is equivalent to `shift 5`.
    let args: &[String] = if args.first().map(String::as_str) == Some("--") {
        &args[1..]
    } else {
        args
    };

    if args.len() > 1 {
        writeln!(stderr, "{}shift: too many arguments", diagnostic_prefix())?;
        // get_numeric_arg (builtins/common.c:511) ends the operand walk with
        // no_args(), whose surplus-argument failure is EX_BADUSAGE
        // followed by a DISCARD jump — the executor arms that discard
        // for this action (rubash#1650).
        return Ok(ShiftAction::TooManyArguments);
    }

    let amount = match args.first() {
        // GNU shift.def reads the count through get_numeric_arg
        // (builtins/common.c:488), whose acceptance rule is
        // general.c:248 valid_number: whitespace-padded numerics are
        // legal (rubash#1339).
        Some(arg) => match crate::builtins::exit::gnu_valid_number(arg) {
            Some(amount) if amount >= 0 => amount as usize,
            Some(_) => {
                writeln!(
                    stderr,
                    "{}shift: {arg}: shift count out of range",
                    diagnostic_prefix()
                )?;
                return Ok(ShiftAction::Complete(1));
            }
            None => {
                writeln!(
                    stderr,
                    "{}shift: {arg}: numeric argument required",
                    diagnostic_prefix()
                )?;
                // shift.def: `get_numeric_arg (list, 0, &times) == 0`
                // (non-numeric operand, builtins/common.c:501-505) returns
                // EX_USAGE, not EXECUTION_FAILURE (issue #759 item 3: GNU
                // 5.3.0 `( set -- 1 2; shift x )` exits 2).
                return Ok(ShiftAction::Complete(EX_USAGE));
            }
        },
        None => 1,
    };
    Ok(ShiftAction::Shift(amount))
}

fn diagnostic_prefix() -> String {
    if let (Ok(script), Ok(line)) = (
        std::env::var("__RUBASH_SCRIPT_NAME"),
        std::env::var("__RUBASH_CURRENT_LINE"),
    ) {
        return format!("{script}: line {line}: ");
    }

    "rubash: ".to_string()
}
