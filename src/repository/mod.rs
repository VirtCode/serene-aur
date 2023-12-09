use std::fs;
use std::fs::{create_dir, create_dir_all, DirEntry};
use std::path::PathBuf;
use actix_files::Files;
use anyhow::Context;
use crate::package::Package;

mod manage;

const REPO_DIR: &str = "./app/repository";
const ARCH: &str = "x86_64";

/// returns the webservice which exposes the repository
pub fn webservice() -> anyhow::Result<Files> {
    create_dir_all(REPO_DIR)?;

    Ok(Files::new(ARCH, REPO_DIR)
        .show_files_listing())
}

pub struct RepositoryEntry {
    pub repository: String,
    pub file: String,
}

pub fn update(package: Package, state: &mut Vec<RepositoryEntry>, repository_name: &str) -> anyhow::Result<()> {
    let file = find_latest_file(&package)?;
    let new_file = file.file_name().expect("found package must have file name").to_string_lossy().to_string();

    if let Some(entry) = state.iter_mut().find(|s| s.repository == package.repository) {
        if entry.file == new_file { return Ok(()) }

        manage::remove(repository_name, vec![&entry.file], &PathBuf::from(REPO_DIR))?;
        fs::remove_dir(PathBuf::from(REPO_DIR).join(&entry.file))?;
        fs::copy(&file, PathBuf::from(REPO_DIR).join(&new_file))?;
        manage::add(repository_name, vec![new_file.as_str()], &PathBuf::from(REPO_DIR))?;

        entry.file = new_file;

    } else {
        fs::copy(&file, PathBuf::from(REPO_DIR).join(&new_file))?;
        manage::add(repository_name, vec![new_file.as_str()], &PathBuf::from(REPO_DIR))?;

        state.push(RepositoryEntry {
            repository: package.repository.clone(),
            file: new_file
        })
    }

    Ok(())

}

pub fn remove(package: Package, state: &mut Vec<RepositoryEntry>, repository_name: &str) -> anyhow::Result<()> {
    let index = state.iter().position(|s| s.repository == package.repository);

    if let Some(i) = index {
        let entry = state.remove(i);

        manage::remove(repository_name, vec![&entry.file], &PathBuf::from(REPO_DIR))?;
        fs::remove_dir(PathBuf::from(REPO_DIR).join(&entry.file))?;
    }

    Ok(())
}

/// finds the last built file in the package directory
pub fn find_latest_file(package: &Package) -> anyhow::Result<PathBuf> {
    let build_path = package.get_path();

    let files = build_path.read_dir()?
        .filter_map(|a| a.ok())
        .filter(|f| f.file_name().to_string_lossy().contains(".pkg.tar."))
        .collect::<Vec<DirEntry>>();

    let mut min: Option<DirEntry> = None;
    for x in files {
        if let Some(next) = &min {
            if x.metadata()?.created()? < next.metadata()?.created()? { min = Some(x) };
        } else {
            min = Some(x)
        }
    }

    min.map(|m| m.path()).context("could not find built package file")
}


