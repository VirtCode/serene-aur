use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::{anyhow, Context};
use async_tar::Builder;
use hyper::Body;
use log::{debug, error};
use tokio::fs;
use crate::package::source::{PackageSource};
use crate::package::source::devel::DevelGitSource;
use crate::package::source::normal::NormalSource;

pub mod git;
pub mod source;
pub mod aur;

const DEFAULT_ARCH: &str = "x86_64";
const PACKAGE_EXTENSION: &str = ".pkg.tar.zst"; // see /etc/makepkg.conf

pub struct PackageManager {
    folder: PathBuf,
    packages: Vec<Package>
}

impl PackageManager {

    pub fn new(folder: &Path) -> Self {
        Self {
            folder: folder.to_owned(),
            packages: vec![]
        }
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

    async fn add(&mut self, mut source: Box<dyn PackageSource>) -> anyhow::Result<String>{
        let folder = self.folder
            .join("tmp")
            .join(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos().to_string());

        fs::create_dir_all(&folder).await?;

        // pull package
        source.create(&folder).await?;
        let base = source.read_base(&folder).await?;
        error!("package-base: {base}");

        // check other packages
        if self.packages.iter().any(|p| p.base == base) {
            fs::remove_dir_all(folder).await?;
            return Err(anyhow!("already have package with base {}", base))
        }

        // move package
        fs::rename(folder, self.folder.join(&base)).await?;

        self.packages.push(Package {
            base_folder: Some(self.folder.clone()),
            base: base.clone(),
            version: "".to_string(),
            source: source,
            devel: false,
            clean: false
        });

        Ok(base)
    }

    /// get a package by name
    pub fn get(&self, string: &str) -> Option<&Package> {
        self.packages.iter().find(|p| p.base == string)
    }

    /// mutably get a package by name
    pub fn get_mut(&mut self, string: &str) -> Option<&mut Package> {
        self.packages.iter_mut().find(|p| p.base == string)
    }
}

pub struct Package {
    pub base_folder: Option<PathBuf>, // base folder of the package, populated when it is read by a registry

    pub base: String,
    source: Box<dyn PackageSource>,
    version: String,

    devel: bool,
    clean: bool,
}

impl Package {

    /// gets the current folder for the package
    fn get_folder(&self) -> PathBuf {
        self.base_folder.as_ref()
            .map(|f| f.join(&self.base))
            .expect("package was not retreived from registry")
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

}

/// selects the built architecture from a list of architectures
/// TODO: make this depend on the env or something
fn select_arch(available: Vec<String>) -> String {
    // x86_64 system can only build either itself or any
    if available.iter().any(|s| s == DEFAULT_ARCH) { DEFAULT_ARCH.to_owned() }
    else { "any".to_string() }
}