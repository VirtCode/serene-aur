use std::path::PathBuf;

use anyhow::{Context, Result};
use tokio::fs;

use crate::{build::BuildSummary, package::Package};

const LOG_DIR: &str = "logs";

/// returns the path to the directory where the logs for a package are stored
fn path(package: &str) -> PathBuf {
    PathBuf::from(LOG_DIR).join(package)
}

/// returns the filename of the logs for a build
fn build_file(build: &BuildSummary) -> String {
    build.started.naive_utc().format("%Y-%m-%dT%H:%M:%S").to_string() + ".log"
}

/// writes the logs for a build to the filesystem
pub async fn write(build: &BuildSummary, logs: String) -> Result<()> {
    let path = path(&build.package);

    if !path.exists() {
        fs::create_dir_all(&path).await.context("failed to create directory to store logs in")?;
    }

    fs::write(path.join(build_file(build)), logs.as_bytes())
        .await
        .context("failed to write logs to file")
}

/// reads the logs for a build from the filesystem
pub async fn read(build: &BuildSummary) -> Result<Option<String>> {
    let path = path(&build.package).join(build_file(build));

    if path.exists() && path.is_file() {
        Ok(Some(fs::read_to_string(path).await.context("failed to read log file")?))
    } else {
        Ok(None)
    }
}

/// removes a package from the log store
pub async fn clean(package: &Package) -> Result<()> {
    let path = path(&package.base);

    if path.exists() {
        fs::remove_dir_all(path).await.context("failed to remove log files")
    } else {
        Ok(())
    }
}
