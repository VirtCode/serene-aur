use chrono::Duration;

const YEAR_NS: i64 = 52 * WEEK_NS;
const MONTH_NS: i64 = 4 * WEEK_NS;
const WEEK_NS: i64 = 7 * DAY_NS;
const DAY_NS: i64 = 24 * HOUR_NS;
const HOUR_NS: i64 = 60 * MINUTE_NS;
const MINUTE_NS: i64 = 60 * SECOND_NS;
const SECOND_NS: i64 = 1000 * MILLISECOND_NS;
const MILLISECOND_NS: i64 = 1000 * MICROSECOND_NS;
const MICROSECOND_NS: i64 = 1000;

const UNITS: &[(&str, &str, i64)] = &[
    ("years", "Y", YEAR_NS),
    ("months", "M", MONTH_NS),
    ("weeks", "w", WEEK_NS),
    ("days", "d", DAY_NS),
    ("hours", "h", HOUR_NS),
    ("minutes", "m", MINUTE_NS),
    ("seconds", "s", SECOND_NS),
    ("milliseconds", "ms", MILLISECOND_NS),
    ("microseconds", "us", MICROSECOND_NS),
];

/// get the coarse string elements for a duration.
/// The returned tuple contains the unit suffix and the
/// count of this unit (e.g. `("ms", 3.2)`)
pub fn coarse_raw(
    duration: Duration,
    short: bool,
    show_subsec: bool,
) -> Option<(&'static str, f32)> {
    let ns = duration.num_nanoseconds().unwrap_or_default();
    let units_slice = if show_subsec { UNITS } else { &UNITS[..4] };

    for (long_suffix, short_suffix, unit_ns) in units_slice {
        if ns >= *unit_ns {
            let count = ns as f32 / *unit_ns as f32;
            return Some(if short { (*short_suffix, count) } else { (*long_suffix, count) });
        }
    }

    None
}

/// formats a duration as a coarse string
pub fn coarse(
    duration: Duration,
    short: bool,
    fraction: bool,
    show_subsec: bool,
) -> Option<String> {
    let (suffix, count) = coarse_raw(duration, short, show_subsec)?;
    let duration_str = if !fraction {
        let count = count as i64;
        if short { format!("{count}{suffix}") } else { format!("{count} {suffix}") }
    } else {
        if short { format!("{count:.2}{suffix}") } else { format!("{count:.2} {suffix}") }
    };
    Some(duration_str)
}

/// turn a duration into a string with exact durations of every unit of
/// [`UNITS`] it contains. E.g. `3 minutes 2 seconds`
pub fn fine(duration: Duration, short: bool, show_subsec: bool) -> String {
    let mut ns = duration.num_nanoseconds().unwrap_or_default();
    let units_slice = if show_subsec { UNITS } else { &UNITS[..4] };

    let format_strs = units_slice
        .iter()
        .filter_map(|(long_suffix, short_suffix, unit_ns)| {
            if ns >= *unit_ns {
                let count = ns / *unit_ns;
                ns -= count * *unit_ns;
                if short {
                    Some(format!("{count} {long_suffix}"))
                } else {
                    Some(format!("{count}{short_suffix}"))
                }
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if format_strs.is_empty() { String::from("instantaneous") } else { format_strs.join(" ") }
}
