use std::collections::HashMap;
use std::path::{Path, PathBuf};
use actix_files::Files;
use anyhow::{anyhow, Context};
use async_tar::Entries;
use futures_util::AsyncRead;
use tokio::fs;
use crate::build::archive;
use crate::package::Package;

mod manage;

const REPO_DIR: &str = "repository";
const REPO_SERENE: &str = "bases.json";

const ARCH: &str = "x86_64";

/// returns the webservice which exposes the repository
pub fn webservice() -> Files {
    Files::new(ARCH, REPO_DIR)
        .show_files_listing()
}

pub struct PackageRepository {
    name: String,
    files: HashMap<String, Vec<String>>
}

impl PackageRepository {

    pub async fn new(name: String) -> anyhow::Result<Self> {
        let mut s = Self {
            name,
            files: HashMap::new()
        };

        s.load().await?;

        Ok(s)
    }

    async fn load(&mut self) -> anyhow::Result<()>{
        let path = Path::new(REPO_DIR).join(REPO_SERENE);
        if !path.is_file() { return Ok(()) }

        let string = fs::read_to_string(path).await
            .context("failed to read database summary from file")?;

        self.files = serde_json::from_str(&string)
            .context("failed to deserialize database summary")?;

        Ok(())
    }

    async fn save(&self) -> anyhow::Result<()> {
        let path = Path::new(REPO_DIR).join(REPO_SERENE);

        let string = serde_json::to_string(&self.files)
            .context("failed to serialize serene database")?;

        fs::write(path, string).await
            .context("failed to write serene database to file")?;

        Ok(())
    }

    pub async fn publish(&mut self, package: &Package, mut output: Entries<impl AsyncRead + Unpin + Sized>) -> anyhow::Result<()> {
        fs::create_dir_all(REPO_DIR).await
            .context("failed to create folder for repository")?;

        let files = package.expected_files().await
            .context("failed to construct expected files from package")?;

        // remove old things if present
        if let Some(files) = self.files.get(&package.base) {
            // remove old files from repository
            manage::remove(&self.name, files, Path::new(REPO_DIR)).await
                .context("failed to remove files from repository")?;

            // delete package files
            for x in files {
                fs::remove_file(Path::new(REPO_DIR).join(x)).await
                    .context(format!("failed to delete file from repository: {x}"))?
            }
        }

        // extract package files
        archive::extract_files(&mut output, &files, Path::new(REPO_DIR)).await
            .context("failed to extract all packages from build container")?;

        // add package files
        manage::add(&self.name, &files, Path::new(REPO_DIR)).await
            .context("failed to add files to repository")?;

        self.files.insert(package.base.clone(), files);
        self.save().await?;

        Ok(())
    }

    async fn remove(&mut self, package: &Package) -> anyhow::Result<()> {

        if let Some(files) = self.files.remove(&package.base) {
            // remove files from repository
            manage::remove(&self.name, &files, Path::new(REPO_DIR)).await
                .context("failed to remove files from repository")?;

            // delete package files
            for x in &files {
                fs::remove_file(Path::new(REPO_DIR).join(x)).await
                    .context(format!("failed to delete file from repository: {x}"))?
            }
        } else {
            return Err(anyhow!("could not find package {} in repository", &package.base))
        }

        self.save().await?;

        Ok(())
    }
}