use crate::config::CONFIG;
use crate::package::source::{Source, SourceImpl};
use crate::package::{aur, git};
use anyhow::Context;
use async_trait::async_trait;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

/// this is a source which pulls from the AUR, checking via RPC for updates
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AurSource {
    base: String,
    version: String,

    /// whether the source is currently using the github mirror
    #[serde(default)]
    mirror: bool,
}

impl AurSource {
    pub fn new(base: &str) -> Self {
        Self { base: base.to_owned(), version: "".to_owned(), mirror: CONFIG.aur_github_mirror }
    }

    pub fn migrated(base: String, version: String) -> Self {
        Self { base, version, mirror: false }
    }

    /// reads the version of the package from the AUR RPC
    pub async fn get_version_aur(&self) -> anyhow::Result<Option<String>> {
        if self.mirror {
            return Ok(None);
        }

        Ok(aur::info(&self.base).await?.map(|p| p.version))
    }

    /// reads the version from the _local_ srcinfo, make sure the repo is
    /// updated beforehand. this is only used as a fallback if the aur lookup
    /// fails or we are using the mirror
    pub async fn get_version_srcinfo(&self, folder: &Path) -> anyhow::Result<String> {
        if !self.mirror {
            warn!("version lookup over RPC failed for AUR package `{}` using srcinfo", self.base);
        }

        self.get_srcinfo(folder)
            .await?
            .context("official AUR package does not contain a .SRCINFO")
            .map(|srcinfo| srcinfo.version())
    }
}

#[typetag::serde]
#[async_trait]
impl SourceImpl for AurSource {
    async fn initialize(&mut self, folder: &Path) -> anyhow::Result<()> {
        debug!("initializing aur source for {}", self.base);

        if self.mirror {
            git::clone(aur::GITHUB_MIRROR, folder, Some(self.base.clone())).await?;
        } else {
            git::clone(&aur::get_repository(&self.base), folder, None).await?;
        }

        self.version = if let Some(version) = self.get_version_aur().await? {
            version
        } else {
            // some packages do not have a package that has the same base (e.g.
            // `material-symbols-git`) and the aur rpc interface does not
            // support looking up a package base, so the above version lookup
            // will fail, thus we will fall back to using the srcinfo for the
            // version if required
            //
            // this will also be used if we are using the github mirror

            self.get_version_srcinfo(folder).await?
        };

        Ok(())
    }

    fn get_url(&self) -> Option<String> {
        Some(aur::get_listing(&self.base))
    }

    fn get_type(&self) -> String {
        format!("aur package{}", if self.mirror { ", github mirror" } else { "" })
    }

    fn get_state(&self) -> String {
        self.version.clone()
    }

    async fn update(&mut self, folder: &Path) -> anyhow::Result<()> {
        debug!("updating aur source for {}", self.base);

        // this should never be hit, unless something went wrong beforehand
        if !folder.exists() {
            warn!("source folder for {} does not exist, recreating it", self.base);
            return self.initialize(folder).await;
        }

        // the user wants to use the mirror now
        if CONFIG.aur_github_mirror != self.mirror {
            info!(
                "recreating aur source for {} to use {}",
                self.base,
                if CONFIG.aur_github_mirror { "github mirror" } else { "aur rpc " }
            );

            // we remove the repository completely, easier than to change the git remote
            fs::remove_dir_all(folder).await?;
            fs::create_dir_all(folder).await?;

            self.mirror = CONFIG.aur_github_mirror;
            return self.initialize(folder).await;
        }

        if let Some(version) = self.get_version_aur().await? {
            // only update if version has changed
            if version != self.version {
                git::pull(folder).await?;

                self.version = version;
            }
        } else {
            // for packages where the aur version lookup does not work,
            // see above for context. in these cases, we have to pull anyway
            git::pull(folder).await?;

            self.version = self.get_version_srcinfo(folder).await?;
        }

        Ok(())
    }
}

/// create a new aur source
pub fn new(base: &str, force_devel: bool) -> Source {
    Source::new(Box::new(AurSource::new(base)), aur::get_devel(base) || force_devel)
}
