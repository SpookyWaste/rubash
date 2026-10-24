#[path = "strftime.rs"]
mod strftime;
#[path = "zone.rs"]
mod zone;

use super::number::{invalid_number_error, parse_i64};
use super::value::{apply_width, truncate_precision};
use super::{FormatSpec, ParsedNumber};
use std::collections::HashMap;
use strftime::strftime_subset;
use zone::TimeZoneRule;

pub(super) fn format_time_value(
    value: &str,
    spec: &FormatSpec,
    env_vars: &HashMap<String, String>,
) -> (String, Option<String>) {
    let ParsedNumber {
        value: seconds,
        invalid,
    } = parse_i64(value);
    // GNU getintmax distinguishes a MISSING argument (the caller passes
    // the -1 "now" sentinel, never an empty string) from a PRESENT empty
    // one: strtoimax on "" leaves ep == s, so chk_converror reports
    // `: invalid number' and the value is 0 (rubash#1116). parse_i64's
    // empty arm models the missing case, so re-arm the diagnostic here,
    // where empty can only mean present.
    let invalid = if value.is_empty() {
        Some(String::new())
    } else {
        invalid
    };
    let seconds = match seconds {
        -1 => current_epoch_seconds(),
        -2 => env_vars
            .get("__RUBASH_SHELL_START_EPOCH")
            .and_then(|value| value.parse().ok())
            .unwrap_or_else(current_epoch_seconds),
        other => other,
    };
    // GNU printf.def getintmax clamps an out-of-range operand to
    // INTMAX_MAX/MIN and chk_converror raises ERANGE; the subsequent
    // localtime() on the clamped value fails, and bash renders the
    // conversion against the zeroed tm — the epoch (rubash#1286:
    // `printf '%(%F)T' 99999999999999999999` prints 1970-01-01 in GNU
    // 5.3.20, both signs; rubash wrapped the value through its civil
    // conversion and printed year 219250468). The ERANGE diagnostic
    // itself is unchanged (invalid_number_error).
    let seconds = if invalid
        .as_deref()
        .is_some_and(|marker| marker.starts_with("__rubash_printf_overflow__:"))
    {
        0
    } else {
        seconds
    };
    let timezone = TimeZoneRule::from_env(env_vars.get("TZ").map(String::as_str));
    let local = timezone.local_time(seconds);
    let format = spec.time_format.as_deref().unwrap_or_default();
    let format = if format.is_empty() { "%X" } else { format };
    let rendered = strftime_subset(format, &local);

    let mut width_spec = spec.clone();
    width_spec.zero_pad = false;
    (
        apply_width(truncate_precision(rendered, spec.precision), &width_spec),
        invalid.map(|value| invalid_number_error(&value)),
    )
}

pub(crate) fn format_current_time(format: &str, env_vars: &HashMap<String, String>) -> String {
    let timezone = TimeZoneRule::from_env(env_vars.get("TZ").map(String::as_str));
    let local = timezone.local_time(current_epoch_seconds());
    strftime_subset(format, &local)
}

/// Format an arbitrary epoch second with the same strftime subset and
/// TZ rules as `printf %()T` (history listings render HISTTIMEFORMAT
/// against each entry's recorded timestamp this way — builtins/
/// history.def display_history via strftime, rubash#1070).
pub(crate) fn format_epoch_time(
    format: &str,
    epoch: i64,
    env_vars: &HashMap<String, String>,
) -> String {
    let timezone = TimeZoneRule::from_env(env_vars.get("TZ").map(String::as_str));
    let local = timezone.local_time(epoch);
    strftime_subset(format, &local)
}

fn current_epoch_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
