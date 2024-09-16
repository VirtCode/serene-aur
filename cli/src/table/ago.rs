use chrono::Duration;

pub fn difference(d: Duration) -> String {
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
