use std::path::Path;
use std::process::{Stdio};
use anyhow::anyhow;
use tokio::process::Command;

fn db_file(name: &str) -> String {
    format!("{name}.db.tar.gz")
}

pub async fn add(name: &str, packages: &Vec<String>, dir: &Path) -> anyhow::Result<()> {
    let mut command = Command::new("repo-add");

    command.arg(db_file(name));
    for package in packages {
        command.arg(package);
    }

    let output = command.current_dir(dir).output().await?;

    if output.status.success() { Ok(()) }
    else { Err(anyhow!("failed to use repo-add: {}", String::from_utf8_lossy(&output.stderr))) }
}

pub async fn remove(name: &str, packages: &Vec<String>, dir: &Path) -> anyhow::Result<()> {
    let mut command = Command::new("repo-remove");

    command.arg(db_file(name));
    for package in packages {
        command.arg(package);
    }

    let output = command.current_dir(dir).output().await?;

    if output.status.success() { Ok(()) }
    else { Err(anyhow!("failed to use repo-remove: {}", String::from_utf8_lossy(&output.stderr))) }
}