use std::path::Path;
use anyhow::{anyhow, Context};
use tokio::process::Command;
use crate::repository::crypto;

fn db_file(name: &str) -> String {
    format!("{name}.db.tar.gz")
}

pub async fn add(name: &str, packages: &Vec<String>, dir: &Path) -> anyhow::Result<()> {
    let mut command = Command::new("repo-add");
    
    if crypto::should_sign_packages() {
        command.args(["--verify", "--sign"]);
    }

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

pub async fn sign(files: &Vec<String>, base_path: &Path) -> anyhow::Result<()> {
    for file in files {
        let path = base_path.join(file);
        let signature_path = base_path.join(format!("{file}.sig"));
        crypto::sign(&signature_path, &path).context(format!("failed to create signature for file: {file}"))?;
    }
    Ok(())
}