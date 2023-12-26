use std::error::Error;
use std::sync::Arc;
use anyhow::Context;
use bollard::Docker;
use log::{error, LevelFilter};
use simplelog::{ColorChoice, TerminalMode, TermLogger};
use tokio::sync::RwLock;
use crate::package::{Package, PackageManager};
use crate::package::store::PackageStore;
use crate::repository::PackageRepository;
use crate::runner::{archive, Runner};
use crate::runner::archive::read_version;

pub mod schedule;

struct Serene {
    store: Arc<RwLock<PackageStore>>,
    runner: Runner,
    repository: PackageRepository,
}

impl Serene {

    pub async fn start(&mut self, package: &str, force: bool) {
        let mut package =
            if let Some(p) = self.store.read().await.get(package) { p }
            else {
                error!("package scheduled for build is no longer in package store");
                return
            };

        let updatable = match package.updatable().await
            .context("failed to check for package updates on scheduled build") {
            Ok(u) => { u }
            Err(e) => { error!("{e:#}"); return }
        };

        if updatable || force {
            self.build(package, updatable).await;
        }
    }

    async fn build(&mut self, package: Package, update: bool) {







    }
}



#[tokio::main]
async fn main_test() -> Result<(), Box<dyn Error>>{
    TermLogger::init(LevelFilter::Debug, simplelog::Config::default(), TerminalMode::Mixed, ColorChoice::Auto).unwrap();

    let store = Arc::new(RwLock::new(PackageStore::init().await?));

    let mut manager = PackageManager::new(store.clone());
    let builder = Runner { docker: Docker::connect_with_socket_defaults().unwrap() };
    let mut repository = PackageRepository::new().await?;

    // download sources
    let package_name = "nvm";
    if !store.read().await.has(package_name) {
        manager.add_aur(package_name).await?;
    }

    let mut package = store.read().await.get(package_name).context("")?;
    if package.updatable().await? {
        package.upgrade_sources().await?
    }

    // create container
    let id = builder.prepare(&package).await?;
    builder.upload_sources(&id, &package).await?;

    // start container
    let result = builder.build(&id).await?;
    //println!("{result:?}");

    // retrieve data
    let mut archive = archive::begin_read(builder.download_packages(&id).await?)?;

    let version = read_version(&mut archive).await?;
    package.upgrade_version(&version).await?;

    // update repository
    repository.publish(&package, archive).await?;
    store.write().await.update(package).await?;

    Ok(())
}
