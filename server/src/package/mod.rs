use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::{anyhow, Context};
use async_tar::Builder;
use chrono::{DateTime, Utc};
use hyper::Body;
use log::{debug, error};
use serde::{Deserialize, Serialize};
use srcinfo::Srcinfo;
use tokio::fs;
use tokio::sync::{Mutex, RwLock};
use crate::build::BuildSummary;
use crate::config::CONFIG;
use crate::package::source::{Source};
use crate::package::source::devel::DevelGitSource;
use crate::package::source::normal::NormalSource;
use crate::package::store::{PackageStore, PackageStoreRef};
use crate::runner::archive;

pub mod git;
pub mod source;
pub mod aur;
pub mod store;

const SOURCE_FOLDER: &str = "sources";

const PACKAGE_EXTENSION: &str = ".pkg.tar.zst"; // see /etc/makepkg.conf

/// adds a repository as a package
pub async fn add_repository(store: Arc<RwLock<PackageStore>>, repository: &str, devel: bool) -> anyhow::Result<Option<String>>{
    debug!("adding package from {repository}, devel: {devel}");

    if devel {
        add_source(store, Box::new(DevelGitSource::empty(repository))).await
    } else {
        add_source(store, Box::new(NormalSource::empty(repository))).await
    }
}

/// adds a source to the package store as a package, returns none if base is already present, otherwise the base is returned
pub async fn add_source(store: Arc<RwLock<PackageStore>>, source: Box<dyn Source + Sync + Send>) -> anyhow::Result<Option<String>> {
    if let Some(source) = PackageSource::create(source, store.clone()).await? {
        let base = source.base.clone();

        store.write().await.update(Package {
            clean: !source.is_devel(),
            enabled: true,
            schedule: None,

            added: Utc::now(),

            base: source.base.clone(),
            version: "".to_string(),
            source,
            prepare: None,

            builds: vec![]
        }).await.context("failed to persist package in store")?;

        Ok(Some(base))
    } else {
        Ok(None)
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PackageSource {
    base: String,
    /// source of the package
    source: Box<dyn Source + Sync + Send>,
    /// string of the current srcinfo, this MUST be parsable, otherwise it will crash, but we cannot store a parsed srcinfo
    srcinfo: String
}

impl PackageSource {

    /// gets the current folder for the package
    fn get_folder_tmp() -> PathBuf {
        Path::new(SOURCE_FOLDER)
            .join("tmp")
            .join(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos().to_string())
    }

    /// gets the current folder for the package
    fn get_folder(&self) -> PathBuf {
        Path::new(SOURCE_FOLDER)
            .join(&self.base)
    }

    /// creates a package source from the given source and checks whether the package already exits with the given store
    pub async fn create(mut source: Box<dyn Source + Sync + Send>, store: Arc<RwLock<PackageStore>>) -> anyhow::Result<Option<Self>> {
        let folder = Self::get_folder_tmp();
        fs::create_dir_all(&folder).await?;

        let result = 'create: {
            // pull source
            let srcinfo = match source.create(&folder).await {
                Ok(s) => s,
                Err(e) => break 'create Err(anyhow!("failed to check out source: {e:?}"))
            };

            // parse pkgbuild
            let parsed: Srcinfo = match srcinfo.parse() {
                Ok(s) => s,
                Err(e) => break 'create Err(anyhow!("failed to parse .SRCINFO: {e:#}"))
            };

            let s = Self {
                base: parsed.base.pkgbase,
                source, srcinfo
            };

            // check other packages
            if store.read().await.has(&s.base) {
                break 'create Ok(None);
            }

            // move package
            if let Err(e) = fs::rename(&folder, s.get_folder()).await {
                break 'create Err(anyhow!("failed to copy source: {e:#}"))
            }

            return Ok(Some(s));
        };

        // cleanup when failed
        fs::remove_dir_all(folder).await?;

        result
    }

    pub async fn updatable(&self) -> anyhow::Result<bool> {
        self.source.update_available().await
    }

    pub async fn update(&mut self) -> anyhow::Result<()> {
        self.srcinfo = self.source.update(&self.get_folder()).await?;
        Ok(())
    }

    /// retrieves the source files for a package in a tar archive, inside a hyper body
    /// warning, this method will load all sources into memory, so be cautious
    pub async fn load_into_tar(&self, archive: &mut Builder<Vec<u8>>) -> anyhow::Result<()>{
        archive.append_dir_all("", &self.get_folder()).await
            .context("failed to load sources into tar")
    }

    pub fn get_version(&self) -> String {
        if self.is_devel() { "latest".to_owned() }
        else { self.get_srcinfo().base.pkgver }
    }

    pub fn get_packages(&self) -> Vec<String> {
        self.get_srcinfo().pkgs.into_iter()
            .map(|p| p.pkgname)
            .collect()
    }

    pub fn get_srcinfo(&self) -> Srcinfo {
        self.srcinfo.parse()
            .expect("failed to parse .SRCINFO")
    }

    pub fn is_devel(&self) -> bool {
        self.source.is_devel()
    }

    /// removes the source files of the source
    pub async fn self_destruct(&self) -> anyhow::Result<()> {
        fs::remove_dir_all(self.get_folder()).await
            .context("could not delete source directory")
    }
}

/// this struct represents a package built by serene
#[derive(Serialize, Deserialize, Clone)]
pub struct Package {
    /// base of the package
    pub base: String,
    /// time when the package was added
    pub added: DateTime<Utc>,

    /// source of the package
    pub source: PackageSource,
    /// last build version of the package
    pub version: String,

    /// whether package is enabled, meaning it is built automatically
    pub enabled: bool,
    /// whether package should be cleaned after building
    pub clean: bool,
    /// potential custom cron schedule string
    pub schedule: Option<String>,
    /// commands to run in container before package build, they are written to the shell
    pub prepare: Option<String>,

    /// contains the summaries of all builds done to the package
    pub(crate) builds: Vec<BuildSummary>
}

impl Package {

    /// gets the schedule string for the package
    pub fn get_schedule(&self) -> String {
        self.schedule.as_ref().unwrap_or_else(|| {
            if self.source.is_devel() { &CONFIG.schedule_devel }
            else { &CONFIG.schedule_default }
        }).clone()
    }

    pub fn get_builds(&self) -> &Vec<BuildSummary> {
        &self.builds
    }

    /// upgrades the version of the package
    /// returns an error if a version mismatch is detected with the source files
    pub async fn upgrade_version(&mut self, reported: &str) -> anyhow::Result<()> {
        if !self.source.is_devel() { // devel packages will report pkgbuild version

            // check for version mismatch
            let source_version = self.source.get_version();

            if source_version.as_str() != reported.trim() {
                return Err(anyhow!("version mismatch on package {}, expected {source_version} but built {reported}", &self.base))
            }
        }

        self.version = reported.to_owned();

        Ok(())
    }

    /// returns the expected built files
    /// requires the version to be upgraded
    pub async fn expected_files(&self) -> anyhow::Result<Vec<String>> {
        let srcinfo = self.source.get_srcinfo();

        let rel = srcinfo.base.pkgrel;
        let epoch = srcinfo.base.epoch.map(|s| format!("{}:", s)).unwrap_or_else(|| "".to_string());
        let arch = select_arch(srcinfo.pkg.arch);
        let version = &self.version;

        Ok(srcinfo.pkgs.iter().map(|p| &p.pkgname).map(|s| {
            format!("{s}-{epoch}{version}-{rel}-{arch}{PACKAGE_EXTENSION}")
        }).collect())
    }

    pub async fn build_files(&self) -> anyhow::Result<Body> {
        let mut archive = archive::begin_write();

        // upload sources
        self.source.load_into_tar(&mut archive).await?;

        // upload prepare script
        archive::write_file(
            self.prepare.clone().unwrap_or_default(),
            "serene-prepare.sh",
            &mut archive,
        ).await?;

        archive::end_write(archive).await
    }

    /// adds a build to the package
    pub fn add_build(&mut self, build: BuildSummary) {
        self.builds.push(build)
    }

    /// update a build summary with a newer version. Matching is done on the start date.
    pub fn update_build(&mut self, build: BuildSummary) {
        self.builds.retain(|f| f.started != build.started);
        self.builds.push(build)
    }
}

/// selects the built architecture from a list of architectures
fn select_arch(available: Vec<String>) -> String {
    // system can only build either itself or any
    if available.iter().any(|s| s == &CONFIG.architecture) { CONFIG.architecture.to_owned() }
    else { "any".to_string() }
}