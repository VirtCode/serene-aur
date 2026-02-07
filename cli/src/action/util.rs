use std::{f128, time::Duration};

const BYTE_SUFFIX: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];

/// turn bytes into a fractional string of the largest unit of [`BYTE_SUFFIX`] it contains
pub fn bytes_str(bytes: usize) -> String {
    let mut divider = 1;
    let mut index = 0;
    while bytes / divider > 1024 && index < BYTE_SUFFIX.len() - 1 {
        divider *= 1024;
        index += 1;
    }

    format!("{:.2} {}", (bytes as f64 / divider as f64), BYTE_SUFFIX[index])
}

const DAY_NS: u128 = 86_400_000_000_000;
const HOUR_NS: u128 = 3_600_000_000_000;
const MINUTE_NS: u128 = 60_000_000_000;
const SECOND_NS: u128 = 1_000_000_000;
const MILLISECOND_NS: u128 = 1_000_000;
const MICROSECOND_NS: u128 = 1000;

const UNITS: &[(&str, u128)] = &[
    ("d", DAY_NS),
    ("h", HOUR_NS),
    ("min", MINUTE_NS),
    ("s", SECOND_NS),
    ("ms", MILLISECOND_NS),
    ("us", MICROSECOND_NS),
    ("ns", 1),
];

/// turn a duration into a fractional string of the largest unit of [`UNITS`] it contains
pub fn duration_str(duration: Duration) -> String {
    let ns = duration.as_nanos();
    for (suffix, unit_ns) in UNITS {
        if ns >= *unit_ns {
            return format!("{:.2}{suffix}", (ns as f128 / *unit_ns as f128) as f64);
        }
    }

    String::from("0ns")
}
