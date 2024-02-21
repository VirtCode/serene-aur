use std::path::Path;
use anyhow::Context;
use async_trait::async_trait;
use log::debug;
use serde::{Deserialize, Serialize};
use crate::package::git;
use crate::package::source::{read_srcinfo_string, Source};

/// this is the source of a normally versioned package
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NormalSource {
    repository: String,
    last_commit: String
}

impl NormalSource {
    /// creates an empty normal source
    pub fn empty(repository: &str) -> Self {
        Self {
            repository: repository.to_owned(),
            last_commit: "".to_owned()
        }
    }
}

#[async_trait]
#[typetag::serde]
impl Source for NormalSource {

    async fn create(&mut self, folder: &Path) -> anyhow::Result<()> {
        debug!("creating {}", self.repository);
        git::clone(&self.repository, folder).await?;

        self.update(folder).await
    }

    async fn update_available(&self) -> anyhow::Result<bool> {
        debug!("updating {}", &self.repository);

        let current_commit = git::latest_commit(&self.repository).await?;
        Ok(current_commit != self.last_commit)
    }

    async fn update(&mut self, folder: &Path) -> anyhow::Result<()> {
        debug!("upgrading {}", &self.repository);

        // pull repo
        git::pull(folder).await?;
        // set commit to newest (could also be done by looking at the local repository...)
        self.last_commit = git::latest_commit(&self.repository).await?;

        Ok(())
    }

    fn is_devel(&self) -> bool { false }
}