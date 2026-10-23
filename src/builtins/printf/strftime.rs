use super::zone::LocalTimeParts;

pub(super) fn strftime_subset(format: &str, time: &LocalTimeParts) -> String {
    let mut output = String::new();
    let mut chars = format.chars();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            output.push(ch);
            continue;
        }
        let Some(specifier) = chars.next() else {
            output.push('%');
            break;
        };
        match specifier {
            '%' => output.push('%'),
            'a' => output.push_str(WEEKDAYS_ABBR[time.weekday as usize]),
            'A' => output.push_str(WEEKDAYS_FULL[time.weekday as usize]),
            'b' | 'h' => output.push_str(MONTHS_ABBR[time.month as usize - 1]),
            'B' => output.push_str(MONTHS_FULL[time.month as usize - 1]),
            // glibc strftime batch verified against GNU bash 5.3.20
            // (rubash#1284): %c locale datetime, %C century, %D = %m/%d/%y,
            // %g ISO year mod 100, %j day of year, %n/%t literal controls,
            // %P lowercase am/pm, %R = %H:%M, %U/%W Sunday/Monday week
            // numbers, %w weekday digit (Sunday = 0).
            'c' => output.push_str(&format!(
                "{} {} {:2} {:02}:{:02}:{:02} {}",
                WEEKDAYS_ABBR[time.weekday as usize],
                MONTHS_ABBR[time.month as usize - 1],
                time.day,
                time.hour,
                time.minute,
                time.second,
                time.year
            )),
            'C' => output.push_str(&format!("{:02}", time.year.div_euclid(100))),
            'D' => output.push_str(&format!(
                "{:02}/{:02}/{:02}",
                time.month,
                time.day,
                time.year.rem_euclid(100)
            )),
            'g' => output.push_str(&format!(
                "{:02}",
                iso_week_year(time.year, time.month, time.day)
                    .0
                    .rem_euclid(100)
            )),
            'j' => output.push_str(&format!(
                "{:03}",
                day_of_year(time.year, time.month, time.day)
            )),
            'n' => output.push('\n'),
            'P' => output.push_str(if time.hour < 12 { "am" } else { "pm" }),
            'R' => output.push_str(&format!("{:02}:{:02}", time.hour, time.minute)),
            't' => output.push('\t'),
            'U' => output.push_str(&format!("{:02}", week_of_year(time, false))),
            'W' => output.push_str(&format!("{:02}", week_of_year(time, true))),
            'w' => output.push_str(&time.weekday.to_string()),
            'd' => output.push_str(&format!("{:02}", time.day)),
            'e' => output.push_str(&format!("{:2}", time.day)),
            'H' => output.push_str(&format!("{:02}", time.hour)),
            // glibc strftime: %I/%l 12-hour clock (zero/space padded),
            // %k 24-hour space padded, %u ISO 8601 weekday 1..7 (Mon=1).
            'I' => output.push_str(&format!("{:02}", twelve_hour(time.hour))),
            'l' => output.push_str(&format!("{:2}", twelve_hour(time.hour))),
            'k' => output.push_str(&format!("{:2}", time.hour)),
            'u' => output.push_str(&iso_weekday(time.weekday).to_string()),
            'm' => output.push_str(&format!("{:02}", time.month)),
            'M' => output.push_str(&format!("{:02}", time.minute)),
            'S' => output.push_str(&format!("{:02}", time.second)),
            'Y' => output.push_str(&format!("{:04}", time.year)),
            'y' => output.push_str(&format!("{:02}", time.year.rem_euclid(100))),
            // glibc strftime: %G/%V are the ISO 8601 week-based year and
            // week number (week 1 = the week with the year's first
            // Thursday; days before it belong to the previous year's last
            // week).
            'G' => output.push_str(&format!(
                "{:04}",
                iso_week_year(time.year, time.month, time.day).0
            )),
            'V' => output.push_str(&format!(
                "{:02}",
                iso_week_year(time.year, time.month, time.day).1
            )),
            'F' => output.push_str(&format!(
                "{:04}-{:02}-{:02}",
                time.year, time.month, time.day
            )),
            'T' => output.push_str(&format!(
                "{:02}:{:02}:{:02}",
                time.hour, time.minute, time.second
            )),
            'r' => output.push_str(&format!(
                "{:02}:{:02}:{:02} {}",
                twelve_hour(time.hour),
                time.minute,
                time.second,
                if time.hour < 12 { "AM" } else { "PM" }
            )),
            'p' => output.push_str(if time.hour < 12 { "AM" } else { "PM" }),
            'z' => output.push_str(&format_offset(time.offset)),
            'Z' => output.push_str(&time.zone_name),
            's' => output.push_str(&time.epoch.to_string()),
            'x' => output.push_str(&format!(
                "{:02}/{:02}/{:02}",
                time.month,
                time.day,
                time.year.rem_euclid(100)
            )),
            'X' => output.push_str(&format!(
                "{:02}:{:02}:{:02}",
                time.hour, time.minute, time.second
            )),
            other => {
                // GNU parse.y `case 'D'` / printf.def `%()T` delegate to the
                // system strftime; glibc emits an unknown conversion
                // verbatim as `%x` — the `%` is kept (probe-verified on
                // bash 5.3.20 + glibc strftime: `\D{%N}` -> `%N`,
                // `%( %N)T` -> `%N`, `strftime("%Q")` -> `%Q`). `%%`
                // stays `%` (arm above); a trailing lone `%` is kept
                // verbatim (arm above). rubash#1099.
                output.push('%');
                output.push(other);
            }
        }
    }
    output
}

fn twelve_hour(hour: u8) -> u8 {
    match hour % 12 {
        0 => 12,
        other => other,
    }
}

/// ISO 8601 weekday number: Monday=1 .. Sunday=7. `weekday` here is
/// 0=Sunday based (civil_from_days).
fn iso_weekday(weekday: u8) -> u8 {
    if weekday == 0 {
        7
    } else {
        weekday
    }
}

/// ISO 8601 week-based (year, week): week 01 is the week containing the
/// first Thursday of the year; a year has 53 weeks when Jan 1 is a
/// Thursday or (leap year) a Wednesday. Determined by which year the
/// week's Thursday falls into (glibc strftime %G/%V).
fn iso_week_year(year: i32, month: u8, day: u8) -> (i32, u8) {
    let weekday = super::zone::weekday_of(year, month, day);
    let wday_iso = i32::from(iso_weekday(weekday));
    let doy = day_of_year(year, month, day);
    let thursday = doy - wday_iso + 4;
    if thursday < 1 {
        let prev_year_days = days_in_iso_year(year - 1);
        (year - 1, ((thursday + prev_year_days - 1) / 7 + 1) as u8)
    } else if thursday > days_in_iso_year(year) {
        (
            year + 1,
            (((thursday - days_in_iso_year(year)) - 1) / 7 + 1) as u8,
        )
    } else {
        (year, ((thursday - 1) / 7 + 1) as u8)
    }
}

fn day_of_year(year: i32, month: u8, day: u8) -> i32 {
    const CUMULATIVE: [i32; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let leap = month > 2 && is_leap(year);
    CUMULATIVE[(month - 1) as usize] + i32::from(day) + leap as i32
}

/// glibc strftime %U (Sunday-first) / %W (Monday-first) week number:
/// days before the year's first week-start day are week 0; the first
/// week-start day opens week 1.
fn week_of_year(time: &LocalTimeParts, monday_first: bool) -> i32 {
    let doy = day_of_year(time.year, time.month, time.day);
    // Weekday of this date in the week's own numbering (first day = 0).
    let weekday = if monday_first {
        (i32::from(time.weekday) + 6) % 7
    } else {
        i32::from(time.weekday)
    };
    let jan1_weekday = (weekday - (doy - 1)).rem_euclid(7);
    let first_start_doy = (7 - jan1_weekday) % 7 + 1;
    if doy < first_start_doy {
        0
    } else {
        1 + (doy - first_start_doy) / 7
    }
}

fn days_in_iso_year(year: i32) -> i32 {
    if is_leap(year) {
        366
    } else {
        365
    }
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn format_offset(offset: i32) -> String {
    let sign = if offset < 0 { '-' } else { '+' };
    let abs = offset.abs();
    format!("{sign}{:02}{:02}", abs / 3600, (abs % 3600) / 60)
}

const WEEKDAYS_ABBR: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const WEEKDAYS_FULL: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];
const MONTHS_ABBR: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const MONTHS_FULL: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
