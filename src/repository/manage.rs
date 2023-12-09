use std::path::Path;
use std::process::{Command, Stdio};
use anyhow::anyhow;

fn db_file(name: &str) -> String {
    format!("{name}.db.tar.gz")
}

pub fn add(name: &str, packages: Vec<&str>, dir: &Path) -> anyhow::Result<()> {
    let mut command = Command::new("repo-add");

    command.arg(db_file(name));
    for package in packages {
        command.arg(package);
    }

    let status = command.current_dir(dir)
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()?.wait()?;

    if status.success() { Ok(()) }
    else { Err(anyhow!("failed to clone git repository")) }
}

pub fn remove(name: &str, packages: Vec<&str>, dir: &Path) -> anyhow::Result<()> {
    let mut command = Command::new("repo-remove");

    command.arg(db_file(name));
    for package in packages {
        command.arg(package);
    }

    let status = command.current_dir(dir)
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()?.wait()?;

    if status.success() { Ok(()) }
    else { Err(anyhow!("failed to clone git repository")) }
}