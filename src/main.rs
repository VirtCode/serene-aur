pub mod runner;
pub mod package;

mod repository;
mod web;
pub mod config;
mod build;

use std::any;
use std::error::Error;
use std::sync::{Arc, };
use actix_web::{App, HttpMessage, HttpServer};
use actix_web::web::Data;
use anyhow::Context;
use bollard::Docker;
use futures::stream::StreamExt;
use futures_util::AsyncReadExt;
use log::LevelFilter;
use simplelog::{ColorChoice, TerminalMode, TermLogger};
use tokio::sync::{Mutex, RwLock};
use crate::build::schedule::BuildScheduler;
use crate::runner::{archive, Runner, ContainerId};
use crate::runner::archive::read_version;
use crate::package::{Package, PackageManager};
use crate::package::store::{PackageStore, PackageStoreRef};
use crate::repository::PackageRepository;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    TermLogger::init(LevelFilter::Debug, simplelog::Config::default(), TerminalMode::Mixed, ColorChoice::Auto).unwrap();

    // initializing storage
    let store = PackageStore::init().await
        .context("failed to serene data storage")?;

    // creating scheduler
    let mut schedule = BuildScheduler::new().await
        .context("failed to start package scheduler")?;

    for package in store.peek() {
        schedule.schedule(package).await
            .context(format!("failed to start schedule for package {}", &package.base))?;
    }

    schedule.start().await?;

    let schedule = Arc::new(RwLock::new(schedule));
    let store = Arc::new(RwLock::new(store));

    HttpServer::new(move ||
        App::new()
            .app_data(Data::new(schedule.clone()))
            .app_data(Data::new(store.clone()))
            .service(repository::webservice())
            .service(web::status)
            .service(web::add)
            .service(web::remove)
            .service(web::build)
    ).bind(("127.0.0.1", 8080))?.run().await?;

    Ok(())
}



