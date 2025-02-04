pub mod cli;
pub mod devel;
pub mod normal;
pub mod single;

use crate::runner::archive::InputArchive;
use anyhow::Context;
use async_trait::async_trait;
use dyn_clone::{clone_trait_object, DynClone};
use srcinfo::Srcinfo;
use std::ops::Deref;
use std::path::Path;
use std::str::FromStr;

const SRCINFO: &str = ".SRCINFO";
const PKGBUILD: &str = "PKGBUILD";

clone_trait_object!(Source);

/// this trait abstracts a package source
#[async_trait]
#[typetag::serde(tag = "type")]
pub trait Source: Sync + Send + DynClone {
    /// pulls the package sources for the first time
    async fn create(&mut self, folder: &Path) -> anyhow::Result<()>;

    /// checks whether an update would be available
    async fn update_available(&self) -> anyhow::Result<bool>;

    /// upgrades the sources
    async fn update(&mut self, folder: &Path) -> anyhow::Result<()>;

    /// returns srcinfo
    async fn get_srcinfo(&self, folder: &Path) -> anyhow::Result<SrcinfoWrapper> {
        tokio::fs::read_to_string(folder.join(SRCINFO))
            .await
            .context("failed to read .SRCINFO")
            .and_then(|s| SrcinfoWrapper::from_str(&s).context("failed to parse .SRCINFO"))
    }

    /// returns pkgbuild
    async fn get_pkgbuild(&self, folder: &Path) -> anyhow::Result<String> {
        tokio::fs::read_to_string(folder.join(PKGBUILD)).await.context("failed to read PKGBUILD")
    }

    /// loads files required for build into archive
    async fn load_build_files(
        &self,
        folder: &Path,
        archive: &mut InputArchive,
    ) -> anyhow::Result<()> {
        archive
            .append_directory(folder, Path::new(""))
            .await
            .context("failed to load sources into tar")
    }

    /// returns internal state of the source, used for checking whether the
    /// current build is up-to-date
    fn get_state(&self) -> String;

    fn is_devel(&self) -> bool;
}

async fn read_srcinfo_string(folder: &Path) -> anyhow::Result<String> {
    tokio::fs::read_to_string(folder.join(SRCINFO)).await.context("failed to read .SRCINFO")
}

/// wraps a srcinfo together with its source so we can convert to and from the
/// src
#[derive(Clone)]
pub struct SrcinfoWrapper {
    source: String,
    inner: Srcinfo,
}

impl FromStr for SrcinfoWrapper {
    type Err = srcinfo::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self { source: s.to_owned(), inner: s.parse()? })
    }
}

impl ToString for SrcinfoWrapper {
    fn to_string(&self) -> String {
        self.source.clone()
    }
}

impl Deref for SrcinfoWrapper {
    type Target = Srcinfo;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Into<Srcinfo> for SrcinfoWrapper {
    fn into(self) -> Srcinfo {
        self.inner
    }
}
