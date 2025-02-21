use crate::repository::crypto;
use anyhow::{anyhow, Context};
use std::os::unix;
use std::path::{Path, PathBuf};
use tokio::process::Command;

fn db_file(name: &str) -> String {
    format!("{name}.db.tar.gz")
}

pub(crate) fn sig_path(path: &Path) -> PathBuf {
    path.with_file_name(format!(
        "{}.sig",
        path.file_name().unwrap_or_default().to_str().unwrap_or_default()
    ))
}

async fn sign_repository(name: &str, dir: &Path) -> anyhow::Result<()> {
    let db_path = &dir.join(format!("{name}.db"));
    let db_archive_path = &dir.join(db_file(name));
    let files_path = &dir.join(format!("{name}.files"));
    let files_archive_path = &dir.join(format!("{name}.files.tar.gz"));

    crypto::sign(&sig_path(db_archive_path), db_archive_path)
        .await
        .context("failed to sign repository database")?;
    if !sig_path(db_path).exists() {
        unix::fs::symlink(sig_path(db_archive_path), sig_path(db_path))
            .context("failed to link repository database signature")?;
    }
    crypto::sign(&sig_path(files_archive_path), files_archive_path)
        .await
        .context("failed to sign repository files")?;
    if !sig_path(files_path).exists() {
        unix::fs::symlink(sig_path(files_archive_path), sig_path(files_path))
            .context("failed to link repository database signature")?;
    }

    Ok(())
}

pub async fn add(name: &str, packages: &Vec<String>, dir: &Path) -> anyhow::Result<()> {
    let mut command = Command::new("repo-add");

    command.arg(db_file(name));
    command.args(packages);

    let output = command.current_dir(dir).output().await?;

    if output.status.success() {
        if crypto::should_sign_packages() {
            sign_repository(name, dir).await?;
        }
        Ok(())
    } else {
        Err(anyhow!("failed to use repo-add: {}", String::from_utf8_lossy(&output.stderr).trim()))
    }
}

pub async fn remove(name: &str, packages: &Vec<String>, dir: &Path) -> anyhow::Result<()> {
    let mut command = Command::new("repo-remove");

    command.arg(db_file(name));
    command.args(packages);

    let output = command.current_dir(dir).output().await?;

    if output.status.success() {
        if crypto::should_sign_packages() {
            sign_repository(name, dir).await?;
        }
        Ok(())
    } else {
        Err(anyhow!(
            "failed to use repo-remove: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

pub async fn sign(files: &Vec<String>, base_path: &Path) -> anyhow::Result<()> {
    for file in files {
        let path = &base_path.join(file);
        crypto::sign(&sig_path(path), path)
            .await
            .context(format!("failed to create signature for file: {file}"))?;
    }
    Ok(())
}
