use std::path::Path;
use std::process::{Command, Stdio};
use anyhow::anyhow;

pub fn clone(repository: &str, directory: &Path) -> anyhow::Result<()> {
    let status = Command::new("git")
        .arg("clone")
        .arg(repository)
        .arg(".")
        .current_dir(directory)
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()?.wait()?;

    if status.success() { Ok(()) }
    else { Err(anyhow!("failed to clone git repository")) }
}

pub fn pull(directory: &Path) -> anyhow::Result<()> {
    let status = Command::new("git")
        .arg("pull")
        .current_dir(directory)
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()?.wait()?;

    if status.success() { Ok(()) }
    else { Err(anyhow!("failed to pull git repository")) }
}