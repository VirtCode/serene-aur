use crate::config::Config;
use std::{fs::read_to_string, path::PathBuf, process::Command};

// installs packages over pacman, and refreshes repositories before doing so
pub fn install(c: &Config, packages: Vec<String>) -> bool {
    Command::new(&c.elevator)
        .arg("pacman")
        .arg("-Sy")
        .args(packages)
        .status()
        .map(|s| s.success())
        .unwrap_or_default()
}

/// returns the path to the pacman config
pub fn config() -> PathBuf {
    PathBuf::from("/etc/pacman.conf")
}

/// returns whether the pacman config already contains a given repository
/// returns true if it fails to read the config
pub fn has_repo(repo: &str) -> bool {
    // we return true if we fail
    read_to_string(config()).map(|s| s.contains(&format!("[{repo}]"))).unwrap_or(true)
}

/// returns the configuration segment needed for a config
pub fn config_repo(c: &Config, repo: &str, signed: bool) -> String {
    format!(
        "\n[{repo}]{}\nServer = {}/{}\n",
        if signed { "" } else { "\nSigLevel = Never" },
        c.url,
        std::env::consts::ARCH
    )
}
