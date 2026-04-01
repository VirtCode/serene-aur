pub mod ago;
pub mod table;

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