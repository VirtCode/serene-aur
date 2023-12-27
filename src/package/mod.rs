use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::{anyhow, Context};
use async_tar::Builder;
use chrono::{DateTime, Utc};
use hyper::Body;
use log::{debug, error};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::sync::Mutex;
use crate::build::BuildSummary;
use crate::config::CONFIG;
use crate::package::source::{PackageSource};
use crate::package::source::devel::DevelGitSource;
use crate::package::source::normal::NormalSource;
use crate::package::store::{PackageStore, PackageStoreRef};

pub mod git;
pub mod source;
pub mod aur;
pub mod store;

const SOURCE_FOLDER: &str = "sources";

const PACKAGE_EXTENSION: &str = ".pkg.tar.zst"; // see /etc/makepkg.conf

pub struct PackageManager {
    store: PackageStoreRef
}

impl PackageManager {

    pub fn new(store: PackageStoreRef) -> Self {
        Self { store }
    }

    /// adds a package from the aur to the manager
    pub async fn add_aur(&mut self, name: &str) -> anyhow::Result<String> {
        debug!("adding aur package {name}");
        let info = aur::find(name).await?;

        self.add_custom(&info.repository, info.devel).await
    }

    /// adds a custom repository to the manager
    pub async fn add_custom(&mut self, repository: &str, devel: bool) -> anyhow::Result<String>{
        debug!("adding package from {repository}, devel: {devel}");

        if devel {
            self.add(Box::new(DevelGitSource::empty(repository))).await
        } else {
            self.add(Box::new(NormalSource::empty(repository))).await
        }
    }

    async fn add(&mut self, mut source: Box<dyn PackageSource + Sync + Send>) -> anyhow::Result<String>{
        let folder = Path::new(SOURCE_FOLDER)
            .join("tmp")
            .join(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos().to_string());

        fs::create_dir_all(&folder).await?;

        // pull package
        source.create(&folder).await?;
        let base = source.read_base(&folder).await?;
        error!("package-base: {base}");

        // check other packages
        if self.store.read().await.has(&base) {
            fs::remove_dir_all(folder).await?;
            return Err(anyhow!("already have package with base {}", base))
        }

        // move package
        fs::rename(folder, Path::new(SOURCE_FOLDER).join(&base)).await?;

        self.store.write().await.update(Package {
            source,
            version: "".to_string(),

            base: base.clone(),
            added: Utc::now(),

            clean: false,
            enabled: true,
            schedule: None,

            builds: vec![]
        }).await.context("failed to persist package in store")?;

        Ok(base)
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Package {
    pub base: String,
    added: DateTime<Utc>,

    source: Box<dyn PackageSource + Sync + Send>,
    pub version: String,

    pub enabled: bool,
    pub clean: bool,
    schedule: Option<String>,

    builds: Vec<BuildSummary>
}

impl Package {

    /// gets the current folder for the package
    fn get_folder(&self) -> PathBuf {
        Path::new(SOURCE_FOLDER).join(&self.base)
    }

    /// gets the schedule string for the package
    pub fn get_schedule(&self) -> String {
        self.schedule.as_ref().unwrap_or_else(|| {
            if self.source.is_devel() { &CONFIG.schedule_devel }
            else { &CONFIG.schedule_default }
        }).clone()
    }

    /// upgrades the version of the package
    /// returns an error if a version mismatch is detected with the source files
    pub async fn upgrade_version(&mut self, reported: &str) -> anyhow::Result<()> {
        if let Some(version) = self.source.read_version(&self.get_folder()).await? {
            if version.as_str() != reported.trim() { return Err(anyhow!("version mismatch on package {}, expected {version} but built {reported}", &self.base)) }

            self.version = version;
        } else {
            self.version = reported.to_owned();
        }

        Ok(())
    }

    /// is there an update available for the package source
    pub async fn updatable(&self) -> anyhow::Result<bool> {
        self.source.update_available().await
    }

    /// upgrades the sources to the newest version
    pub async fn upgrade_sources(&mut self) -> anyhow::Result<()> {
        self.source.upgrade(&self.get_folder()).await
    }

    /// returns the expected built files
    /// requires the version to be upgraded
    pub async fn expected_files(&self) -> anyhow::Result<Vec<String>> {
        // get epoch and rel from srcinfo
        // TODO: this reads the .SRCINFO twice, but once is enough
        let srcinfo = self.source.read_srcinfo(&self.get_folder()).await?;
        let packages = self.source.read_packages(&self.get_folder()).await?;

        let rel = srcinfo.base.pkgrel;
        let epoch = srcinfo.base.epoch.map(|s| format!("{}:", s)).unwrap_or_else(|| "".to_string());
        let arch = select_arch(srcinfo.pkg.arch);
        let version = &self.version;

        Ok(packages.iter().map(|s| {
            format!("{s}-{epoch}{version}-{rel}-{arch}{PACKAGE_EXTENSION}")
        }).collect())
    }

    // returns the expected packages
    pub async fn expected_packages(&self) -> anyhow::Result<Vec<String>> {
        self.source.read_packages(&self.get_folder()).await
    }

    /// retrieves the source files for a package in a tar archive, inside a hyper body
    /// warning, this method will load all sources into memory, so be cautious
    pub async fn sources_tar(&self) -> anyhow::Result<Body> {
        let folder = self.get_folder();

        let buffer = vec![];
        let mut archive = Builder::new(buffer);

        archive.append_dir_all("", &folder).await?;
        archive.finish().await?;

        Ok(Body::from(archive.into_inner().await?))
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