use chrono::Duration;

/// formats a duration as a coarse string
pub fn coarse(d: Duration) -> String {
    let (name, amount) = if d.num_weeks() > 52 {
        ("year", d.num_weeks() / 52)
    } else if d.num_weeks() > 4 {
        ("month", d.num_weeks() / 4)
    } else if d.num_weeks() > 0 {
        ("week", d.num_weeks())
    } else if d.num_days() > 0 {
        ("day", d.num_days())
    } else if d.num_hours() > 0 {
        ("hour", d.num_hours())
    } else if d.num_minutes() > 0 {
        ("minute", d.num_minutes())
    } else if d.num_seconds() > 0 {
        ("second", d.num_seconds())
    } else {
        ("", -1)
    };

    if amount < 0 {
        "now".to_string()
    } else {
        format!("{amount:2} {name}{}", if amount > 1 { "s" } else { "" })
    }
}

/// formats a duration as a fine string
pub fn fine(duration: Duration) -> String {
    let mut result = vec![];

    let original = duration.num_seconds();
    let mut seconds = original;

    // hours
    if original > 60 * 60 {
        let hours = seconds / (60 * 60);
        seconds -= hours * (60 * 60);

        result.push(format!("{hours}h"));
    }

    // minutes
    if original > 60 {
        let minutes = seconds / 60;
        seconds -= minutes * 60;

        result.push(format!("{minutes}m"));
    }

    result.push(format!("{seconds}s"));

    result.join(" ")
}
