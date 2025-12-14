use crate::config::CONFIG;
use crate::package::git;
use crate::package::srcinfo::SrcinfoWrapper;
use log::debug;
use raur::{Package, Raur};
use std::collections::HashMap;

pub const GITHUB_MIRROR: &str = "https://github.com/archlinux/aur";

/// finds a package in the aur
pub async fn info(name: &str) -> anyhow::Result<Option<Package>> {
    let raur = raur::Handle::new();
    let pkg = raur.info(&[name]).await?;

    Ok(pkg.into_iter().next())
}

/// checks whether the package with a given base exists
/// using the experimental github mirror
pub async fn check_exists_mirror(base: &str) -> anyhow::Result<bool> {
    git::find_remote_ref(GITHUB_MIRROR, &format!("refs/heads/{base}")).await.map(|a| a.is_some())
}

/// get whether a package is a devel package
pub fn get_devel(base: &str) -> bool {
    base.ends_with("-git")
}

/// get the url to the git repository
pub fn get_repository(base: &str) -> String {
    format!("https://aur.archlinux.org/{base}.git")
}

/// get the url to the aurweb listing
pub fn get_listing(base: &str) -> String {
    format!("https://aur.archlinux.org/pkgbase/{base}")
}

/// Finds all latest commits for the sources of a srcinfo.
/// This is used to determine whether a devel package has to be updated.
pub async fn source_latest_version(
    srcinfo: &SrcinfoWrapper,
) -> anyhow::Result<HashMap<String, String>> {
    let mut commits = HashMap::new();

    for src in srcinfo
        .base
        .source
        .iter()
        .filter(|f| f.arch.as_ref().map(|a| a == &CONFIG.architecture).unwrap_or(true)) // only include relevant archs
        .flat_map(|s| &s.vec)
    {
        let url_start = src.find("::").map(|i| i + 2).unwrap_or(0);
        let url = &src[url_start..];

        debug!("considering source url: {url}");

        // we only support git urls, other urls are either static or not supported (like
        // hg+, etc.)
        if let Some(git_url) = url.strip_prefix("git+") {
            // we insert with the `git_url` for backwards compatibility
            commits.insert(git_url.to_owned(), git::find_remote_commit(git_url).await?);
        }
    }

    Ok(commits)
}
