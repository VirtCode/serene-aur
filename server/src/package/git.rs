use std::cmp::min;
use std::collections::HashMap;
use std::path::Path;
use anyhow::{anyhow, Context};

// clone a repository using git
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

// pull in a repository with git
pub async fn pull(directory: &Path) -> anyhow::Result<()> {
    let status = tokio::process::Command::new("git")
        .arg("pull")
        .current_dir(directory)
        .output().await?;

    if status.status.success() { Ok(()) }
    else { Err(anyhow!("failed to pull git repository: {}", String::from_utf8_lossy(&status.stderr))) }
}

// finds the version of the git remote, given a git url.
// The url should be in the format described here (without the directory and git+): https://man.archlinux.org/man/PKGBUILD.5#USING_VCS_SOURCES
pub async fn find_commit(url: &str) -> anyhow::Result<String> {
    let fragment = url.find('#');
    let query = url.find('?');

    // remote to make git query against
    let remote = &url[0..min(url.len(), min(fragment.unwrap_or(usize::MAX), query.unwrap_or(usize::MAX)))];

    // extract fragments
    let fragments = if let Some(fragment) = fragment {
        let fragments = &url[(fragment + 1)..query.and_then(|q| if q > fragment { Some(q) } else { None }).unwrap_or(url.len())];

        fragments.split('&').filter_map(|s| {
            let mut args = s.split('=');
            Some((args.next()?, args.next()?))
        }).collect::<HashMap<&str, &str>>()

    } else { HashMap::new() };

    // if a commit is specified, it'll always be that commit
    if let Some(commit) = fragments.get("commit") {
        return Ok(commit.to_string());
    }

    // query git
    let status = tokio::process::Command::new("git")
        .arg("ls-remote")
        .arg(remote)
        .output().await?;

    if !status.status.success() {
        return Err(anyhow!("failed to check remote for {remote}: {}", String::from_utf8_lossy(&status.stderr)))
    }

    let response = String::from_utf8_lossy(&status.stdout).to_string();

    // target ref to get version for
    let target = if let Some(tag) = fragments.get("tag") {
        format!("refs/tags/{tag}")
    } else if let Some(branch) = fragments.get("branch") {
        format!("refs/heads/{branch}")
    } else {
        "HEAD".to_owned()
    };

    for line in response.split('\n') {
        let mut split = line.split_whitespace();
        
        if let (Some(commit), Some(what)) = (split.next(), split.next()) {
            if what == target { return Ok(commit.to_owned()) }
        } 
    }

    Err(anyhow!("failed to find ref '{target}' of remote '{remote}'"))
}