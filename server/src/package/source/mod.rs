pub mod aur;
pub mod cli;
pub mod git;
mod legacy;
pub mod raw;

use crate::package;
use crate::package::srcinfo::{SrcinfoGenerator, SrcinfoGeneratorInstance, SrcinfoWrapper};
use crate::runner::archive::InputArchive;
use anyhow::Context;
use async_trait::async_trait;
use dyn_clone::{clone_trait_object, DynClone};
use log::debug;
use serde::{Deserialize, Serialize};
use srcinfo::Srcinfo;
use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::str::FromStr;
use tokio::fs;

const SRCINFO: &str = ".SRCINFO";
const PKGBUILD: &str = "PKGBUILD";

// Source types:
// - cli source
// - git source (arbitrary git repository containing pkgbuild)
// - aur source (aur source where updates are first checked via rpc, and only
//   then via git)
// - static source (static pkgbuild file)
// - extern source (using folder on the filesystem)

clone_trait_object!(SourceImpl);

#[typetag::serde(tag = "type")]
#[async_trait]
pub trait SourceImpl: Sync + Send + DynClone {
    /// initialize the source by pulling all the build files for the first time
    async fn initialize(&mut self, folder: &Path) -> anyhow::Result<()>;

    /// return an url associated with the upstream of the source
    fn get_url(&self) -> Option<String>;

    /// return the name of the source type
    fn get_type(&self) -> String;

    /// returns the current state of the source as a string
    fn get_state(&self) -> String;

    /// update the source files to the newest version
    async fn update(&mut self, folder: &Path) -> anyhow::Result<()>;

    /// get the pkgbuild of the source
    async fn get_pkgbuild(&self, folder: &Path) -> anyhow::Result<String> {
        fs::read_to_string(folder.join(PKGBUILD)).await.context("failed to read PKGBUILD")
    }

    /// get the srcinfo of the source
    async fn get_srcinfo(&self, folder: &Path) -> anyhow::Result<Option<SrcinfoWrapper>> {
        let path = folder.join(SRCINFO);

        if path.exists() {
            fs::read_to_string(path)
                .await
                .context("failed to read .SRCINFO")
                .and_then(|s| SrcinfoWrapper::from_str(&s).context("failed to parse .SRCINFO"))
                .map(|a| Some(a))
        } else {
            Ok(None)
        }
    }

    /// load the build files for this package into an archive
    async fn load_build_files(
        &self,
        archive: &mut InputArchive,
        folder: &Path,
    ) -> anyhow::Result<()> {
        archive.append_directory(folder, Path::new("")).await
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Source {
    /// is this source devel
    pub devel: bool,
    /// do we want to override srcinfo explicitly
    pub srcinfo_override: bool,

    /// srcinfo stored if the inner source does not provide any
    srcinfo: Option<SrcinfoWrapper>,
    /// revisions of the devel sources
    devel_revisions: HashMap<String, String>,

    /// actual source housed by this
    inner: Box<dyn SourceImpl + Sync + Send>,
}

impl Source {
    /// create a new source with default values
    pub fn new(inner: Box<dyn SourceImpl + Sync + Send>, devel: bool) -> Self {
        Self {
            devel,
            inner,
            srcinfo_override: false,
            srcinfo: None,
            devel_revisions: HashMap::new(),
        }
    }

    /// initializes the source in the folder
    pub async fn initialize(
        &mut self,
        srcinfo_generator: &SrcinfoGeneratorInstance,
        folder: &Path,
    ) -> anyhow::Result<()> {
        // run inner initialization
        self.inner.initialize(folder).await?;

        // initialize itself by updating (will gen srcinfo etc.)
        self.update(srcinfo_generator, folder).await
    }

    /// update the build files of the source to their newest state
    pub async fn update(
        &mut self,
        srcinfo_generator: &SrcinfoGeneratorInstance,
        folder: &Path,
    ) -> anyhow::Result<()> {
        let before = self.inner.get_state();
        self.inner.update(folder).await?;

        let inner_no_srcinfo = self.inner.get_srcinfo(folder).await?.is_none();

        if (self.inner.get_state() != before && (inner_no_srcinfo || self.srcinfo_override))
            || (self.srcinfo.is_none() && inner_no_srcinfo)
        {
            let mut input = InputArchive::new();
            self.inner.load_build_files(&mut input, folder).await?;

            self.srcinfo = Some(
                srcinfo_generator
                    .lock()
                    .await
                    .generate_srcinfo(input)
                    .await
                    .context("failed to generate srcinfo for package")?,
            );
        }

        if self.devel {
            self.devel_revisions =
                package::aur::source_latest_version(&self.get_srcinfo(folder).await?).await?;
        }

        Ok(())
    }

    /// get state of the source, used to check whether up-to-date
    pub fn get_state(&self) -> String {
        let mut string = self.inner.get_state();

        if self.devel {
            for commit in self.devel_revisions.values() {
                string.push_str(commit);
            }
        }

        string
    }

    /// get the srcinfo of the source
    pub async fn get_srcinfo(&self, folder: &Path) -> anyhow::Result<SrcinfoWrapper> {
        let srcinfo = if self.srcinfo_override {
            self.srcinfo.clone()
        } else {
            self.inner.get_srcinfo(folder).await?.clone().or_else(|| self.srcinfo.clone())
        };

        srcinfo.context(
            "failed to get a srcinfo for a package, this is an internal error, please report",
        )
    }

    /// get the pkgbuild of the source
    pub async fn get_pkgbuild(&self, folder: &Path) -> anyhow::Result<String> {
        self.inner.get_pkgbuild(folder).await
    }

    /// load the files required for build into a given archive
    pub async fn load_build_files(
        &self,
        folder: &Path,
        archive: &mut InputArchive,
    ) -> anyhow::Result<()> {
        self.inner.load_build_files(archive, folder).await
    }
}
