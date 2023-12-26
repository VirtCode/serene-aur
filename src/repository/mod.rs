use std::collections::HashMap;
use std::path::{Path, PathBuf};
use actix_files::Files;
use anyhow::{anyhow, Context};
use async_tar::Entries;
use futures_util::AsyncRead;
use hyper::body::HttpBody;
use serde::{Deserialize, Serialize};
use tokio::fs;
use crate::build::archive;
use crate::config::CONFIG;
use crate::package::Package;

mod manage;

const REPO_DIR: &str = "repository";
const REPO_SERENE: &str = "bases.json";

/// returns the webservice which exposes the repository
pub fn webservice() -> Files {
    Files::new(&CONFIG.architecture, REPO_DIR)
        .show_files_listing()
}

pub struct PackageRepository {
    name: String,
    bases: HashMap<String, Vec<PackageEntry>>
}

#[derive(Serialize, Deserialize)]
struct PackageEntry {
    name: String,
    file: String
}

impl PackageRepository {

    pub async fn new() -> anyhow::Result<Self> {
        let mut s = Self {
            name: CONFIG.repository_name.to_owned(),
            bases: HashMap::new()
        };

        s.load().await?;

        Ok(s)
    }

    async fn load(&mut self) -> anyhow::Result<()>{
        let path = Path::new(REPO_DIR).join(REPO_SERENE);
        if !path.is_file() { return Ok(()) }

        let string = fs::read_to_string(path).await
            .context("failed to read database summary from file")?;

        self.bases = serde_json::from_str(&string)
            .context("failed to deserialize database summary")?;

        Ok(())
    }

    async fn save(&self) -> anyhow::Result<()> {
        let path = Path::new(REPO_DIR).join(REPO_SERENE);

        let string = serde_json::to_string(&self.bases)
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
        if let Some(entries) = self.bases.get(&package.base) {
            // remove old files from repository
            manage::remove(&self.name, &entries.iter().map(|e| e.name.clone()).collect(), Path::new(REPO_DIR)).await
                .context("failed to remove files from repository")?;

            // delete package files
            for entry in entries {
                fs::remove_file(Path::new(REPO_DIR).join(&entry.file)).await
                    .context(format!("failed to delete file from repository: {}", entry.file))?
            }
        }

        // extract package files
        archive::extract_files(&mut output, &files, Path::new(REPO_DIR)).await
            .context("failed to extract all packages from build container")?;

        // add package files
        manage::add(&self.name, &files, Path::new(REPO_DIR)).await
            .context("failed to add files to repository")?;

        // create entries
        let entries = package.expected_packages().await
            .context("failed to read expected packages from file")?
            .into_iter().zip(files)
            .map(|(name, file)| PackageEntry { name, file }).collect();

        self.bases.insert(package.base.clone(), entries);
        self.save().await?;

        Ok(())
    }

    async fn remove(&mut self, package: &Package) -> anyhow::Result<()> {

        if let Some(entries) = self.bases.remove(&package.base) {
            // remove old files from repository
            manage::remove(&self.name, &entries.iter().map(|e| e.name.clone()).collect(), Path::new(REPO_DIR)).await
                .context("failed to remove files from repository")?;

            // delete package files
            for entry in entries {
                fs::remove_file(Path::new(REPO_DIR).join(&entry.file)).await
                    .context(format!("failed to delete file from repository: {}", entry.file))?
            }
        } else {
            return Err(anyhow!("could not find package {} in repository", &package.base))
        }

        self.save().await?;

        Ok(())
    }
}