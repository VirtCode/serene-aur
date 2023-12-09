use std::fs::{create_dir_all, read_to_string};
use std::path::{Path, PathBuf};
use anyhow::Context;
use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use git2::Repository;
use regex::Regex;

const BUILD_DIR: &str = "./app/build";
const PKGBUILD_FILE: &str = "PKGBUILD";

#[derive(Debug)]
pub struct Package {
    /// url to the repository
    repository: String,

    /// name of the package to be used in the cli, etc.
    name: String,
    /// current version of the package
    version: String,

    /// is it a package from the aur
    aur: bool,
    /// is it a git package that needs frequent rebuilding
    git: bool
}

impl Package {
    pub fn get_id(&self) -> String {
        repository_id(&self.repository)
    }

    pub fn get_path(&self) -> PathBuf {
        repository_path(&self.repository)
    }
}

fn repository_id(repository: &str) -> String {
    BASE64_URL_SAFE_NO_PAD.encode(&repository)
}

fn repository_path(repository: &str) -> PathBuf {
    PathBuf::from(BUILD_DIR).join(repository_id(repository))
}

pub fn get_from_aur(name: &str) -> anyhow::Result<Package> {
    let git = name.ends_with("-git");

    get(&format!("https://aur.archlinux.org/{name}.git"), true, git)
}

pub fn get(repository: &str, aur: bool, git: bool) -> anyhow::Result<Package> {
    create_dir_all(BUILD_DIR)?;

    let path = repository_path(repository);

    let _repo = Repository::clone(repository, &path).context("failed to clone given git repository")?;
    let (name, version) = parse_pkgbuild(&path)?;


    Ok(Package {
        repository: repository.to_owned(),
        name, version, aur, git
    })
}

fn parse_pkgbuild(path: &Path) -> anyhow::Result<(String, String)> {
    let text = read_to_string(path.join(PKGBUILD_FILE)).context("repo does not contain readable PKGBUILD")?;

    let name_regex = Regex::new("pkgname=(.+)").expect("regex should compile");
    let version_regex = Regex::new("pkgver=(.+)").expect("regex should compile");

    let name = name_regex.captures(&text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_owned())
        .context("PKGBUILD did not contain package name")?;

    let version = version_regex.captures(&text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_owned())
        .context("PKGBUILD did not contain package version")?;

    Ok((name, version))
}

