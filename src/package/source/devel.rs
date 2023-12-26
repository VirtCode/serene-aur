use std::collections::HashMap;
use std::path::Path;
use async_trait::async_trait;
use log::debug;
use serde::{Deserialize, Serialize};
use srcinfo::Srcinfo;
use crate::package::git;
use crate::package::source::{PackageSource};

/// this is the source of a -git development package
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DevelGitSource {
    repository: String,
    last_commit: String,
    last_source_commits: HashMap<String, String>
}

impl DevelGitSource {
    /// creates an empty source
    pub fn empty(repository: &str) -> Self {
        Self {
            repository: repository.to_owned(),
            last_commit: "".to_owned(),
            last_source_commits: HashMap::new()
        }
    }
}

#[async_trait]
#[typetag::serde]
impl PackageSource for DevelGitSource {

    async fn create(&mut self, folder: &Path) -> anyhow::Result<()> {
        debug!("creating {}", self.repository);
        git::clone(&self.repository, folder).await?;

        self.upgrade(folder).await
    }

    async fn update_available(&self) -> anyhow::Result<bool> {
        // check pkgbuild repository
        debug!("updating {}", &self.repository);
        let current_commit = git::latest_commit(&self.repository).await?;
        if current_commit != self.last_commit { return Ok(true) }

        // check sources
        for (repo, commit) in &self.last_source_commits {
            debug!("updating source {}", repo);
            let latest = git::latest_commit(repo).await?;
            if &latest != commit { return Ok(true) }
        }

        Ok(false)
    }

    async fn upgrade(&mut self, folder: &Path) -> anyhow::Result<()> {
        // pull pkg repo
        debug!("upgrading {}", &self.repository);
        git::pull(folder).await?;
        self.last_commit = git::latest_commit(&self.repository).await?;

        // refresh sources
        self.last_source_commits = HashMap::new();
        let srcinfo: Srcinfo = self.read_srcinfo(folder).await?;

        for src in srcinfo.base.source.iter().flat_map(|s| &s.vec) { // TODO: only use required arch
            let mut split = src.split('+');

            if split.next() != Some("git") { continue } // skip non-git sources

            // TODO: Support more complex git urls
            if let Some(repo) = split.next() {
                debug!("upgrading source {}", repo);
                let commit = git::latest_commit(repo).await?;
                self.last_source_commits.insert(repo.to_owned(), commit);
            }
        }

        Ok(())
    }

    async fn read_version(&self, _folder: &Path) -> anyhow::Result<Option<String>> {
        Ok(None)
    }

    fn is_devel(&self) -> bool { true }
}