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
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use bollard::Docker;
use futures::stream::StreamExt;
use futures_util::AsyncReadExt;
use log::{info, LevelFilter};
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, RwLock};
use crate::build::schedule::BuildScheduler;
use crate::build::Builder;
use crate::config::CONFIG;
use crate::runner::{archive, Runner, ContainerId};
use crate::package::store::{PackageStore};
use crate::repository::PackageRepository;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    // initializing storage
    let store = PackageStore::init().await
        .context("failed to create serene data storage")?;
    let store = Arc::new(RwLock::new(store));

    // initializing builder
    let builder = Builder::new(
        store.clone(),
        Runner::new()
            .context("failed to initialize docker runner")?,
        PackageRepository::new().await
            .context("failed to create package repository")?
    );
    let builder = Arc::new(RwLock::new(builder));

    // creating scheduler
    let mut schedule = BuildScheduler::new(builder.clone()).await
        .context("failed to start package scheduler")?;

    for package in store.read().await.peek() {
        schedule.schedule(package).await
            .context(format!("failed to start schedule for package {}", &package.base))?;
    }

    schedule.start().await?;

    let schedule = Arc::new(RwLock::new(schedule));

    // web app
    HttpServer::new(move ||
        App::new()
            .app_data(Data::from(schedule.clone()))
            .app_data(Data::from(store.clone()))
            .app_data(Data::from(builder.clone()))
            .service(repository::webservice())
            .service(web::add)
            .service(web::list)
            .service(web::status)
            .service(web::remove)
            .service(web::build)
    ).bind(("0.0.0.0", CONFIG.port))?.run().await?;

    Ok(())
}



