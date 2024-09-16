use crate::package::source::Source;
use crate::package::{aur, git};
use async_trait::async_trait;
use log::debug;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// this is the source of a -git development package
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DevelGitSource {
    repository: String,
    last_commit: String,
    last_source_commits: HashMap<String, String>,
}

impl DevelGitSource {
    /// creates an empty source
    pub fn empty(repository: &str) -> Self {
        Self {
            repository: repository.to_owned(),
            last_commit: "".to_owned(),
            last_source_commits: HashMap::new(),
        }
    }
}

#[async_trait]
#[typetag::serde]
impl Source for DevelGitSource {
    async fn create(&mut self, folder: &Path) -> anyhow::Result<()> {
        debug!("creating {}", self.repository);
        git::clone(&self.repository, folder).await?;

        self.update(folder).await
    }

    async fn update_available(&self) -> anyhow::Result<bool> {
        // check pkgbuild repository
        debug!("updating {}", &self.repository);
        let current_commit = git::find_commit(&self.repository).await?;
        if current_commit != self.last_commit {
            return Ok(true);
        }

        // check sources
        // FIXME: compare this with aur::source_latest_version, as not everything has to
        // be a git commit
        for (repo, commit) in &self.last_source_commits {
            debug!("updating source {}", repo);
            let latest = git::find_commit(repo).await?;
            if &latest != commit {
                return Ok(true);
            }
        }

        Ok(false)
    }

    async fn update(&mut self, folder: &Path) -> anyhow::Result<()> {
        // pull pkg repo
        debug!("upgrading {}", &self.repository);
        git::pull(folder).await?;
        self.last_commit = git::find_commit(&self.repository).await?;

        // refresh sources
        self.last_source_commits =
            aur::source_latest_version(&self.get_srcinfo(folder).await?).await?;

        Ok(())
    }

    fn is_devel(&self) -> bool {
        true
    }
}
