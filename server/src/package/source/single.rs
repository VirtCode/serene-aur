use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use anyhow::Context;
use async_tar::Builder;
use async_trait::async_trait;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use crate::package::aur::generate_srcinfo_string;
use crate::package::{aur, git};
use crate::package::source::{PKGBUILD, Source, SrcinfoWrapper};
use crate::runner::archive;

/// This is a static source which only consists of a pkgbuild
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SingleSource {
    pkgbuild: String,
    srcinfo: String,

    devel: bool,
    last_source_commits: HashMap<String, String>
}

impl SingleSource {
    pub fn initialize(pkgbuild: String, devel: bool) -> Self {
        Self {
            devel, pkgbuild,
            srcinfo: "".to_owned(),
            last_source_commits: HashMap::new()
        }
    }
}

#[async_trait]
#[typetag::serde]
impl Source for SingleSource {

    async fn create(&mut self, _folder: &Path) -> anyhow::Result<()> {

        info!("generating .SRCINFO for static package");
        self.srcinfo = generate_srcinfo_string(&self.pkgbuild).await
            .context("failed to generate .SRCINFO for package")?;

        self.update(_folder).await
    }

    async fn update_available(&self) -> anyhow::Result<bool> {
        if self.devel {
            for (repo, commit) in &self.last_source_commits {
                debug!("checking source {}", repo);
                let latest = git::latest_commit(repo).await?;
                if &latest != commit { return Ok(true) }
            }
        }

        Ok(false)
    }

    async fn update(&mut self, _folder: &Path) -> anyhow::Result<()> {
        if self.devel {
            self.last_source_commits = aur::source_latest_commits(&self.get_srcinfo(_folder).await?).await?
        }

        Ok(())
    }

    async fn get_pkgbuild(&self, _folder: &Path) -> anyhow::Result<String> {
        Ok(self.pkgbuild.clone())
    }

    async fn get_srcinfo(&self, _folder: &Path) -> anyhow::Result<SrcinfoWrapper> {
            SrcinfoWrapper::from_str(&self.srcinfo).context("failed to parse .SRCINFO for cli")
    }

    async fn load_build_files(&self, _folder: &Path, archive: &mut Builder<Vec<u8>>) -> anyhow::Result<()> {
        archive::write_file(
            self.pkgbuild.clone(),
            PKGBUILD, true,
            archive,
        ).await
    }

    fn is_devel(&self) -> bool {
        self.devel
    }
}