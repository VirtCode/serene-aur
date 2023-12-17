pub mod build;
pub mod package;

mod repository;
mod web;

use std::any;
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use actix_web::{App, HttpServer};
use actix_web::web::Data;
use bollard::container::{Config, CreateContainerOptions, ListContainersOptions, LogsOptions, StartContainerOptions, WaitContainerOptions};
use bollard::Docker;
use bollard::exec::{CreateExecOptions, StartExecResults};
use futures::stream::StreamExt;
use futures_util::AsyncReadExt;
use hyper::Body;
use log::LevelFilter;
use simplelog::{ColorChoice, TerminalMode, TermLogger};
use crate::build::{archive, Builder, ContainerId};
use crate::build::archive::read_version;
use crate::package::{Package, PackageManager};
use crate::repository::PackageRepository;

#[tokio::main]
async fn main_web() -> anyhow::Result<()> {

    HttpServer::new(move ||
        App::new()
            .service(repository::webservice())
            .service(web::status)
            .service(web::add)
            .service(web::remove)
            .service(web::build)
    ).bind(("127.0.0.1", 8080))?.run().await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>>{
    TermLogger::init(LevelFilter::Debug, simplelog::Config::default(), TerminalMode::Mixed, ColorChoice::Auto).unwrap();

    let mut manager = PackageManager::new(&PathBuf::from("app/sources"));
    let builder = Builder { docker: Docker::connect_with_socket_defaults().unwrap() };
    let mut repository = PackageRepository::new("app/repository".into(), "aur".to_string());

    // download sources
    let package_name = "nvm";
    manager.add_aur(package_name).await?;
    let package = manager.get_mut(package_name).unwrap();

    // create container
    let id = builder.prepare(package).await?;
    builder.upload_sources(&id, package).await?;

    // start container
    let result = builder.build(&id).await?;
    println!("{result:?}");

    // retrieve data
    let mut archive = archive::begin_read(builder.download_packages(&id).await?)?;

    let version = read_version(&mut archive).await?;
    package.upgrade_version(&version).await?;

    // update repository
    repository.publish(package, archive).await?;

    Ok(())
}
