use std::process::Command;
use crate::config::Config;

// installs packages over pacman, and refreshes repositories before doing so
pub fn install(c: &Config, packages: Vec<String>) -> bool {
    Command::new(&c.elevator)
        .arg("pacman").arg("-Sy").args(packages)
        .status().map(|s| s.success()).unwrap_or_default()
}