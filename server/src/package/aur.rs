use std::collections::HashMap;
use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::anyhow;
use log::debug;
use raur::Raur;
use crate::config::CONFIG;
use crate::package::git;
use crate::package::source::SrcinfoWrapper;

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
            repository: to_git(info),
            devel: is_devel(info)
        }))
    } else {
        Ok(None)
    }
}

/// converts aur package to git url
pub fn to_git(package: &raur::Package) -> String {
    format!("https://aur.archlinux.org/{}.git", &package.package_base)
}

/// Returns the srcinfo string for a pkgbuild located in the given directory
/// TODO: This method of using makepkg quite dubious, as it switches to another user just for that. Improve this!
pub async fn generate_srcinfo_string(pkgbuild: &str) -> anyhow::Result<String> {
    let dir = PathBuf::from("/tmp").join(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos().to_string());

    tokio::fs::create_dir(&dir).await?;
    tokio::fs::write(dir.join("PKGBUILD"), pkgbuild).await?;

    let uid_output = tokio::process::Command::new("id").arg("-u").output().await?;
    let uid = String::from_utf8_lossy(&uid_output.stdout);

    // detect whether running in container (as root)
    let status = if uid.trim() == "0" {
        tokio::fs::set_permissions(&dir, Permissions::from_mode(0o777)).await?;
        tokio::fs::set_permissions(dir.join("PKGBUILD"), Permissions::from_mode(0o777)).await?;

        tokio::process::Command::new("su")
            .arg("user")
            .arg("sh")
            .arg("-c")
            .arg("makepkg --printsrcinfo")
            .current_dir(&dir)
            .output().await?
    } else {
        tokio::process::Command::new("makepkg").arg("--printsrcinfo")
            .current_dir(&dir)
            .output().await?
    };

    tokio::fs::remove_dir_all(dir).await?;

    if !status.status.success() { Err(anyhow!("failed generate srcinfo: {}", String::from_utf8_lossy(&status.stderr))) }
    else {
        Ok(String::from_utf8_lossy(&status.stdout).to_string())
    }
}


/// checks whether a package is a devel package based on its name
pub fn is_devel(pkg: &raur::Package) -> bool {
    pkg.package_base.ends_with("-git") // currently only -git devel packages are supported
}

/// Finds all latest commits for the sources of a srcinfo.
/// This is used to determine whether a devel package has to be updated.
pub async fn source_latest_version(srcinfo: &SrcinfoWrapper) -> anyhow::Result<HashMap<String, String>> {
    let mut commits = HashMap::new();

    for src in srcinfo.base.source.iter()
        .filter(|f| f.arch.as_ref().map(|a| a == &CONFIG.architecture).unwrap_or(true)) // only include relevant archs
        .flat_map(|s| &s.vec) {

        let url_start = src.find("::").map(|i| i + 2).unwrap_or(0);
        let url = &src[url_start..];

        debug!("considering source url: {url}");

        // we only support git urls, other urls are either static or not supported (like hg+, etc.)
        if url.starts_with("git+") {
            debug!("fetching state via git");

            let git_url = &url["git+".len()..];

            // we insert with the `git_url` for backwards compatibility
            commits.insert(git_url.to_owned(), git::find_commit(git_url).await?);
        }
    }

    Ok(commits)
}
