use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::{anyhow, Context};
use log::{debug, error};
use tokio::fs;
use crate::package::source::{DevelSource, NormalSource, PackageSource};

pub mod git;
pub mod source;
pub mod aur;

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
            self.add(Box::new(DevelSource::empty(repository))).await
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

        self.packages.push(Package { base: base.clone(), source });

        Ok(base)
    }
}

struct Package {
    base: String,
    source: Box<dyn PackageSource>
}