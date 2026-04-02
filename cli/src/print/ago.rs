use chrono::Duration;
use num_traits::AsPrimitive;

const YEAR_NS: i64 = 52 * WEEK_NS;
const MONTH_NS: i64 = 4 * WEEK_NS;
const WEEK_NS: i64 = 7 * DAY_NS;
const DAY_NS: i64 = 24 * HOUR_NS;
const HOUR_NS: i64 = 60 * MINUTE_NS;
const MINUTE_NS: i64 = 60 * SECOND_NS;
const SECOND_NS: i64 = 1000 * MILLISECOND_NS;
const MILLISECOND_NS: i64 = 1000 * MICROSECOND_NS;
const MICROSECOND_NS: i64 = 1000;

#[rustfmt::skip]
const UNITS: &[(&str, &str, i64)] = &[
    ("years",        "Y",  YEAR_NS),
    ("months",       "M",  MONTH_NS),
    ("weeks",        "w",  WEEK_NS),
    ("days",         "d",  DAY_NS),
    ("hours",        "h",  HOUR_NS),
    ("minutes",      "m",  MINUTE_NS),
    ("seconds",      "s",  SECOND_NS),
    ("milliseconds", "ms", MILLISECOND_NS),
    ("microseconds", "us", MICROSECOND_NS),
];

/// get the coarse string elements for a duration.
/// The returned tuple contains the unit suffix and the
/// count of this unit (e.g. `("ms", 3.2)`)
pub fn coarse_raw<T>(
    duration: Duration,
    short: bool,
    show_subsec: bool,
) -> Option<(&'static str, T)>
where
    f32: AsPrimitive<T>,
    T: 'static + Copy + PartialOrd,
{
    let ns = duration.num_nanoseconds().unwrap_or_default();

    for (long_str, short_str, unit_ns) in UNITS {
        if !show_subsec && *unit_ns < SECOND_NS {
            break;
        }

        if ns >= *unit_ns {
            let count: T = (ns as f32 / *unit_ns as f32).as_();
            let one: T = 1.0f32.as_();

            return Some(if short {
                (*short_str, count)
            } else {
                (if count == one { &long_str[..(long_str.len() - 1)] } else { *long_str }, count)
            });
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
    Some(if fraction {
        let (unit, count) = coarse_raw::<f32>(duration, short, show_subsec)?;
        format!("{count:.2}{}{unit}", if short { "" } else { " " })
    } else {
        let (unit, count) = coarse_raw::<i32>(duration, short, show_subsec)?;
        format!("{count:.2}{}{unit}", if short { "" } else { " " })
    })
}

/// turn a duration into a string with exact durations of every unit of
/// [`UNITS`] it contains. E.g. `3 minutes 2 seconds`
pub fn fine(duration: Duration, short: bool, show_subsec: bool) -> String {
    let mut ns = duration.num_nanoseconds().unwrap_or_default();

    let format_strs = UNITS
        .iter()
        .filter_map(|(long_str, short_str, unit_ns)| {
            if !show_subsec && *unit_ns < SECOND_NS {
                return None;
            }

            if ns >= *unit_ns {
                let count = ns / *unit_ns;
                ns -= count * *unit_ns;
                if short {
                    Some(format!(
                        "{count} {}",
                        if count > 1 { *long_str } else { &long_str[..(long_str.len() - 1)] }
                    ))
                } else {
                    Some(format!("{count}{short_str}"))
                }
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if format_strs.is_empty() { String::from("instantaneous") } else { format_strs.join(" ") }
}
