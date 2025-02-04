use crate::package::aur::generate_srcinfo_string;
use crate::package::source::{Source, SrcinfoWrapper, PKGBUILD};
use crate::package::{aur, git};
use crate::runner::archive;
use crate::runner::archive::InputArchive;
use anyhow::Context;
use async_tar::Builder;
use async_trait::async_trait;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use serene_data::secret;
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

/// This is a static source which only consists of a pkgbuild
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SingleSource {
    pkgbuild: String,
    srcinfo: String,

    devel: bool,
    last_source_commits: HashMap<String, String>,
}

impl SingleSource {
    pub fn initialize(pkgbuild: String, devel: bool) -> Self {
        Self { devel, pkgbuild, srcinfo: "".to_owned(), last_source_commits: HashMap::new() }
    }
}

#[async_trait]
#[typetag::serde]
impl Source for SingleSource {
    async fn create(&mut self, folder: &Path) -> anyhow::Result<()> {
        info!("generating .SRCINFO for static package");
        self.srcinfo = generate_srcinfo_string(&self.pkgbuild)
            .await
            .context("failed to generate .SRCINFO for package")?;

        self.update(folder).await
    }

    async fn update_available(&self) -> anyhow::Result<bool> {
        if self.devel {
            // FIXME: same here as in devel source
            for (repo, commit) in &self.last_source_commits {
                debug!("checking source {}", repo);
                let latest = git::find_commit(repo).await?;
                if &latest != commit {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    async fn update(&mut self, folder: &Path) -> anyhow::Result<()> {
        if self.devel {
            self.last_source_commits =
                aur::source_latest_version(&self.get_srcinfo(folder).await?).await?
        }

        Ok(())
    }

    async fn get_pkgbuild(&self, _folder: &Path) -> anyhow::Result<String> {
        Ok(self.pkgbuild.clone())
    }

    async fn get_srcinfo(&self, _folder: &Path) -> anyhow::Result<SrcinfoWrapper> {
        SrcinfoWrapper::from_str(&self.srcinfo).context("failed to parse .SRCINFO for cli")
    }

    async fn load_build_files(
        &self,
        _folder: &Path,
        archive: &mut InputArchive,
    ) -> anyhow::Result<()> {
        archive.write_file(&self.pkgbuild, Path::new(PKGBUILD), true).await
    }

    fn get_state(&self) -> String {
        // yes, this is technically for secrets
        let mut string = secret::hash(&self.pkgbuild);

        if self.devel {
            for commit in self.last_source_commits.values() {
                string.push_str(commit);
            }
        }

        string
    }

    fn is_devel(&self) -> bool {
        self.devel
    }
}
