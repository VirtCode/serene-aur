use std::path::{Path, PathBuf};
use anyhow::{anyhow, Context};
use raur::Raur;

// this struct represents information about a package in the aur
pub struct AurInfo {
    // base name of the package
    pub base: String,
    // repository of the package source
    pub repository: String,
    // is development package
    pub devel: bool
}

/// finds a package in the aur
pub async fn find(name: &str) -> anyhow::Result<Option<AurInfo>> {
    let raur = raur::Handle::new();
    let pkg = raur.info(&[name]).await?;

    if let Some(info) = pkg.first() {
        Ok(Some(AurInfo {
            base: info.package_base.clone(),
            repository: to_aur_git(&info.package_base),
            devel: info.package_base.ends_with("-git")
        }))
    } else {
        Ok(None)
    }
}

fn to_aur_git(base: &str) -> String {
    format!("https://aur.archlinux.org/{base}.git")
}

/// Returns the srcinfo string for a pkgbuild located in the given directory
pub async fn generate_srcinfo_string(pkgbuild: &Path) -> anyhow::Result<String> {
    let parent = pkgbuild.parent().map(|p| p.to_owned()).unwrap_or(PathBuf::new());
    let file = pkgbuild.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or("PKGBUILD".to_string());

    let status = tokio::process::Command::new("makepkg")
        .current_dir(parent)
        .arg("-p")
        .arg(file)
        .arg("--printsrcinfo")
        .output().await?;

    if !status.status.success() { Err(anyhow!("failed generate srcinfo: {}", String::from_utf8_lossy(&status.stderr))) }
    else {
        Ok(String::from_utf8_lossy(&status.stdout).to_string())
    }
}