use anyhow::anyhow;
use std::cmp::min;
use std::collections::HashMap;
use std::path::Path;
use tokio::process::Command;

// clone a repository using git
pub async fn clone(
    repository: &str,
    directory: &Path,
    branch: Option<String>,
) -> anyhow::Result<()> {
    let mut command = Command::new("git");
    command.arg("clone");

    // if we want a specific branch, only fetch that one
    if let Some(branch) = branch {
        command.arg("--single-branch").arg("--branch").arg(branch);
    }

    command.arg(repository);

    // make sure working directory is fine
    command.arg(".").current_dir(directory);

    let status = command.output().await?;

    if status.status.success() {
        Ok(())
    } else {
        Err(anyhow!("failed to clone git repository: {}", String::from_utf8_lossy(&status.stderr)))
    }
}

// pull in a repository with git
pub async fn pull(directory: &Path) -> anyhow::Result<()> {
    let status = Command::new("git").arg("pull").current_dir(directory).output().await?;

    if status.status.success() {
        Ok(())
    } else {
        Err(anyhow!("failed to pull git repository: {}", String::from_utf8_lossy(&status.stderr)))
    }
}

pub async fn find_local_commit(directory: &Path) -> anyhow::Result<String> {
    let status =
        Command::new("git").arg("rev-parse").arg("HEAD").current_dir(directory).output().await?;

    if status.status.success() {
        Ok(String::from_utf8_lossy(&status.stdout).trim().to_owned())
    } else {
        Err(anyhow!("failed to pull git repository: {}", String::from_utf8_lossy(&status.stderr)))
    }
}

// finds the version of the git remote, given a git url.
// The url should be in the format described here (without the directory and git+): https://man.archlinux.org/man/PKGBUILD.5#USING_VCS_SOURCES
pub async fn find_remote_commit(url: &str) -> anyhow::Result<String> {
    let fragment = url.find('#');
    let query = url.find('?');

    // remote to make git query against
    let remote =
        &url[0..min(url.len(), min(fragment.unwrap_or(usize::MAX), query.unwrap_or(usize::MAX)))];

    // extract fragments
    let fragments = if let Some(fragment) = fragment {
        let fragments = &url[(fragment + 1)
            ..query.and_then(|q| if q > fragment { Some(q) } else { None }).unwrap_or(url.len())];

        fragments
            .split('&')
            .filter_map(|s| {
                let mut args = s.split('=');
                Some((args.next()?, args.next()?))
            })
            .collect::<HashMap<&str, &str>>()
    } else {
        HashMap::new()
    };

    // if a commit is specified, it'll always be that commit
    if let Some(commit) = fragments.get("commit") {
        return Ok(commit.to_string());
    }

    // target ref to get version for
    let target = if let Some(tag) = fragments.get("tag") {
        format!("refs/tags/{tag}")
    } else if let Some(branch) = fragments.get("branch") {
        format!("refs/heads/{branch}")
    } else {
        "HEAD".to_owned()
    };

    find_remote_ref(remote, &target)
        .await?
        .ok_or(anyhow!("failed to find ref '{target}' of remote '{remote}'"))
}

/// performs an ls-remote for a specific ref and returns its hash if found
pub async fn find_remote_ref(remote: &str, refstr: &str) -> anyhow::Result<Option<String>> {
    // query git
    let status = Command::new("git").arg("ls-remote").arg(remote).arg(refstr).output().await?;

    if !status.status.success() {
        return Err(anyhow!(
            "failed to check remote for {remote}: {}",
            String::from_utf8_lossy(&status.stderr)
        ));
    }

    let response = String::from_utf8_lossy(&status.stdout).to_string();
    let line = response.trim();

    // no response, the ref does not exist
    if response.is_empty() {
        return Ok(None);
    }

    let mut split = line.split_whitespace();

    if let (Some(commit), Some(what)) = (split.next(), split.next()) {
        debug_assert!(what == refstr); // otherwise git ain't working correctly

        Ok(Some(commit.to_owned()))
    } else {
        Err(anyhow!("response for ls-remote for {remote} and {refstr} was malformed: {response}"))
    }
}
