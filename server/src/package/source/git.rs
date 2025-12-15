use crate::package::git;
use crate::package::source::{Source, SourceImpl};
use async_trait::async_trait;
use log::debug;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// this is a source which pulls the build files from a custom git repository
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GitSource {
    repository: String,
    last_commit: String,
}

impl GitSource {
    pub fn new(repository: &str) -> Self {
        Self { repository: repository.to_owned(), last_commit: "".to_owned() }
    }

    pub fn migrated(repository: String, last_commit: String) -> Self {
        Self { repository, last_commit }
    }
}

#[typetag::serde]
#[async_trait]
impl SourceImpl for GitSource {
    async fn initialize(&mut self, folder: &Path) -> anyhow::Result<()> {
        debug!("initializing git source for {}", self.repository);

        git::clone(&self.repository, folder, None).await?;
        self.last_commit = git::find_local_commit(folder).await?;

        Ok(())
    }

    fn get_url(&self) -> Option<String> {
        Some(self.repository.clone())
    }

    fn get_type(&self) -> String {
        "git repository".to_string()
    }

    fn get_state(&self) -> String {
        self.last_commit.clone()
    }

    async fn update(&mut self, folder: &Path) -> anyhow::Result<()> {
        debug!("updating git source for {}", self.repository);

        // pull repo
        git::pull(folder).await?;
        self.last_commit = git::find_local_commit(folder).await?;

        Ok(())
    }
}

/// create a new git source
pub fn new(repository: &str, devel: bool) -> Source {
    Source::new(Box::new(GitSource::new(repository)), devel)
}
