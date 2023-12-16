use std::path::Path;
use anyhow::{anyhow, Context};

pub async fn clone(repository: &str, directory: &Path) -> anyhow::Result<()> {
    let status = tokio::process::Command::new("git")
        .arg("clone")
        .arg(repository)
        .arg(".")
        .current_dir(directory)
        .output().await?;

    if status.status.success() { Ok(()) }
    else { Err(anyhow!("failed to clone git repository: {}", String::from_utf8_lossy(&status.stderr))) }
}

pub async fn pull(directory: &Path) -> anyhow::Result<()> {
    let status = tokio::process::Command::new("git")
        .arg("pull")
        .current_dir(directory)
        .output().await?;

    if status.status.success() { Ok(()) }
    else { Err(anyhow!("failed to pull git repository: {}", String::from_utf8_lossy(&status.stderr))) }
}

pub async fn latest_commit(repository: &str) -> anyhow::Result<String> {
    let status = tokio::process::Command::new("git")
        .arg("ls-remote")
        .arg(repository)
        .output().await?;

    if !status.status.success() { Err(anyhow!("failed to check remote for {repository}: {}", String::from_utf8_lossy(&status.stderr))) }
    else {
        String::from_utf8_lossy(&status.stdout)
            .split('\n').next()
            .and_then(|l| l.split_whitespace().next())
            .map(|s| s.to_owned())
            .context("could not find head in repository")
    }
}