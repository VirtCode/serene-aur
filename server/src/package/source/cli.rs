use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use anyhow::Context;
use async_tar::Builder;
use async_trait::async_trait;
use log::debug;
use serde::{Deserialize, Serialize};
use crate::config;
use crate::package::aur::generate_srcinfo_string;
use crate::package::git;
use crate::package::source::{PKGBUILD, Source, SrcinfoWrapper};

/// this is a custom source for the serene cli
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SereneCliSource {
    last_commit: String,
}

impl SereneCliSource {
    pub fn new() -> Self {
        Self {
            last_commit: "".to_owned(),
        }
    }
}

#[async_trait]
#[typetag::serde]
impl Source for SereneCliSource {

    async fn create(&mut self, folder: &Path) -> anyhow::Result<()> {
        debug!("creating cli source");
        git::clone(config::SOURCE_REPOSITORY, folder).await?;

        self.update(folder).await
    }

    async fn update_available(&self) -> anyhow::Result<bool> {
        debug!("updating cli source");

        let current_commit = git::find_commit(config::SOURCE_REPOSITORY).await?;
        Ok(current_commit != self.last_commit)
    }

    async fn update(&mut self, folder: &Path) -> anyhow::Result<()> {
        debug!("upgrading cli source");

        // pull repo
        git::pull(folder).await?;
        // set commit to newest (could also be done by looking at the local repository...)
        self.last_commit = git::find_commit(config::SOURCE_REPOSITORY).await?;

        Ok(())
    }

    async fn get_pkgbuild(&self, folder: &Path) -> anyhow::Result<String> {
        tokio::fs::read_to_string(folder.join("cli").join(PKGBUILD)).await
            .context("failed to read PKGBUILD of cli")
    }

    async fn get_srcinfo(&self, folder: &Path) -> anyhow::Result<SrcinfoWrapper> {
        generate_srcinfo_string(&self.get_pkgbuild(folder).await?).await
            .context("failed to generate .SRCINFO for cli")
            .and_then(|s| SrcinfoWrapper::from_str(&s).context("failed to parse .SRCINFO for cli"))
    }

    async fn load_build_files(&self, folder: &Path, archive: &mut Builder<Vec<u8>>) -> anyhow::Result<()> {
        // using async_std here because of our archive crate
        let mut file = async_std::fs::File::open(folder.join("cli").join(PKGBUILD)).await.context("failed to open pkgbuild for upload")?;

        archive.append_file("PKGBUILD", &mut file).await
            .context("failed to load sources into tar")
    }

    fn is_devel(&self) -> bool { true }
}