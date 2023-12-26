pub mod normal;
pub mod devel;

use std::path::Path;
use anyhow::Context;
use async_trait::async_trait;
use dyn_clone::{clone_trait_object, DynClone};
use srcinfo::Srcinfo;

const SRCINFO: &str = ".SRCINFO";

clone_trait_object!(PackageSource);

/// this trait abstracts a package source
#[async_trait]
#[typetag::serde(tag = "type")]
pub trait PackageSource: Sync + DynClone {

    /// pulls the package sources for the first time
    async fn create(&mut self, folder: &Path) -> anyhow::Result<()>;

    /// checks whether an update would be available
    async fn update_available(&self) -> anyhow::Result<bool>;

    /// upgrades the sources
    async fn upgrade(&mut self, folder: &Path) -> anyhow::Result<()>;

    /// try to read the current version, returning None if the version is unknown
    async fn read_version(&self, folder: &Path) -> anyhow::Result<Option<String>> {
        Ok(Some(self.read_srcinfo(folder).await?.base.pkgver))
    }

    /// read the package base name
    async fn read_base(&self, folder: &Path) -> anyhow::Result<String> {
        Ok(self.read_srcinfo(folder).await?.base.pkgbase)
    }

    /// read all package names contained in the pkgbuild
    async fn read_packages(&self, folder: &Path) -> anyhow::Result<Vec<String>> {
        Ok(self.read_srcinfo(folder).await?.pkgs.into_iter().map(|p| p.pkgname).collect())
    }

    /// read entire srcinfo from disk
    async fn read_srcinfo(&self, folder: &Path) -> anyhow::Result<Srcinfo> {
        tokio::fs::read_to_string(folder.join(SRCINFO)).await?
            .parse().context("failed to parse srcinfo")
    }

    fn is_devel(&self) -> bool;
}