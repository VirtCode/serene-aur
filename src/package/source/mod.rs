pub mod normal;
pub mod devel;

use std::path::Path;
use actix_web::HttpMessage;
use anyhow::Context;
use async_trait::async_trait;
use dyn_clone::{clone_trait_object, DynClone};
use srcinfo::Srcinfo;

const SRCINFO: &str = ".SRCINFO";

clone_trait_object!(Source);

/// this trait abstracts a package source
#[async_trait]
#[typetag::serde(tag = "type")]
pub trait Source: Sync + Send + DynClone {

    /// pulls the package sources for the first time, returns the srcinfo string
    async fn create(&mut self, folder: &Path) -> anyhow::Result<String>;

    /// checks whether an update would be available
    async fn update_available(&self) -> anyhow::Result<bool>;

    /// upgrades the sources, returns the srcinfo string
    async fn update(&mut self, folder: &Path) -> anyhow::Result<String>;

    fn is_devel(&self) -> bool;
}

async fn read_srcinfo_string(folder: &Path) -> anyhow::Result<String> {
    tokio::fs::read_to_string(folder.join(SRCINFO)).await
        .context("failed to read .SRCINFO")
}