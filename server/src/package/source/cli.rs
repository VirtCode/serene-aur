use crate::config;
use crate::package::git;
use crate::package::source::{Source, SourceImpl, SrcinfoWrapper, PKGBUILD};
use crate::runner::archive::InputArchive;
use anyhow::Context;
use async_trait::async_trait;
use log::debug;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// this is a custom source for the serene cli
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CliSource {
    last_commit: String,
}

impl CliSource {
    pub fn new() -> Self {
        Self { last_commit: "".to_owned() }
    }

    pub fn migrated(last_commit: String) -> Self {
        Self { last_commit }
    }
}

#[typetag::serde]
#[async_trait]
impl SourceImpl for CliSource {
    async fn initialize(&mut self, folder: &Path) -> anyhow::Result<()> {
        debug!("initializing cli source");

        git::clone(config::SOURCE_REPOSITORY, folder).await?;
        self.last_commit = git::find_local_commit(folder).await?;

        Ok(())
    }

    fn get_url(&self) -> Option<String> {
        Some(config::SOURCE_REPOSITORY.to_string())
    }

    fn get_type(&self) -> String {
        "internal cli".to_string()
    }

    fn get_state(&self) -> String {
        self.last_commit.clone()
    }

    async fn update(&mut self, folder: &Path) -> anyhow::Result<()> {
        debug!("updating cli source");

        // pull repo
        git::pull(folder).await?;
        self.last_commit = git::find_local_commit(folder).await?;

        Ok(())
    }

    async fn get_pkgbuild(&self, folder: &Path) -> anyhow::Result<String> {
        fs::read_to_string(folder.join("cli").join("PKGBUILD"))
            .context("failed to read PKGBUILD in serene-aur git repository")
    }

    async fn get_srcinfo(&self, _folder: &Path) -> anyhow::Result<Option<SrcinfoWrapper>> {
        Ok(None)
    }

    async fn load_build_files(
        &self,
        archive: &mut InputArchive,
        folder: &Path,
    ) -> anyhow::Result<()> {
        archive.append_file(&folder.join("cli").join(PKGBUILD), Path::new(PKGBUILD)).await
    }
}

/// create a new cli souce
pub fn new() -> Source {
    Source::new(Box::new(CliSource::new()), true)
}
