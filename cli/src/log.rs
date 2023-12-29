
macro_rules! info {
    ($($t:tt)*) => {{
        println!($($t)*);
    }};
}

macro_rules! error {
    ($($t:tt)*) => {{
        use colored::Colorize;

        let formatted = format!($($t)*);
        println!("{}{}", "error: ".red().bold(), formatted);
    }};
}
