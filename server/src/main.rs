#![feature(extract_if)]
#![feature(type_alias_impl_trait)]

pub mod package;
pub mod runner;

pub mod config;
pub mod init;

mod build;
mod database;
mod repository;
mod resolve;
mod web;

use crate::build::schedule::BuildScheduler;
use crate::build::{cleanup_unfinished, Builder};
use crate::config::CONFIG;
use crate::database::package::migrate_sources;
use crate::package::srcinfo::SrcinfoGenerator;
use crate::package::{migrate_build_state, Package};
use crate::repository::PackageRepository;
use crate::runner::update::ImageScheduler;
use crate::runner::Runner;
use crate::web::broadcast::Broadcast;
use actix_web::web::Data;
use actix_web::{App, HttpServer};
use anyhow::Context;
use config::{Config, Info, INFO};
use database::DATABASE;
use log::{debug, error, info};
use package::srcinfo::SRCINFO_GENERATOR;
use web::broadcast::BROADCAST;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    INFO.init(Info::start());
    info!("starting serene version {}", INFO.version);

    debug!("loading configuration from env variables");
    CONFIG.init(Config::env());

    // initializing database
    DATABASE.init(database::connect().await?);
    BROADCAST.init(Broadcast::new());

    // initializing runner
    let runner = Arc::new(Runner::new().context("failed to connect to docker")?);

    // initializing repository
    let repository = Arc::new(Mutex::new(
        PackageRepository::new().await.context("failed to create package repository")?,
    ));

    // initializing srcinfo generator
    SRCINFO_GENERATOR.init(Mutex::new(SrcinfoGenerator::new(runner.clone())));

    // initializing builder
    let builder =
        Arc::new(Builder::new(runner.clone(), repository.clone()));

    // creating scheduler
    let mut schedule = BuildScheduler::new(builder.clone())
        .await
        .context("failed to start package scheduler")?;

    // creating image scheduler
    let image_scheduler = ImageScheduler::new(runner.clone());

    // yes, this will wait before starting the api, because after updates stuff
    // can't be built without a new image
    // we'll do this before the migrations because some need the new container
    // already
    image_scheduler.run_sync().await;

    // cleanup unfinished builds
    if let Err(e) = cleanup_unfinished().await {
        error!("failed to cleanup unfinished builds: {e:#}")
    }

    // migrations
    if let Err(e) = migrate_build_state().await {
        error!("failed apply heuristics to migrate to built_state: {e:#}")
    }

    migrate_sources().await?; // we should panic if it fails

    repository::remove_orphan_signature().await;

    // schedule packages (which are enabled)
    for package in Package::find_all().await?.iter().filter(|p| p.enabled) {
        schedule
            .schedule(package)
            .await
            .context(format!("failed to start schedule for package {}", &package.base))?;
    }

    if config::CONFIG.build_cli {
        if let Err(e) = package::try_add_cli(&mut schedule).await {
            error!("failed to add cli package: {e:#}")
        }
    }

    image_scheduler.start().await?;
    schedule.start().await?;

    let schedule = Arc::new(Mutex::new(schedule));

    info!("serene started successfully on port {}!", CONFIG.port);
    // web app
    HttpServer::new(move || {
        App::new()
            .app_data(Data::from(schedule.clone()))
            .app_data(Data::from(builder.clone()))
            .service(repository::webservice())
            .service(web::info)
            .service(web::add)
            .service(web::list)
            .service(web::status)
            .service(web::remove)
            .service(web::build_all)
            .service(web::build)
            .service(web::get_all_builds)
            .service(web::get_build)
            .service(web::get_logs)
            .service(web::subscribe_logs)
            .service(web::settings)
            .service(web::pkgbuild)
            .service(web::get_webhook_secret)
            .service(web::build_webhook)
            .service(web::get_signature_public_key)
    })
    .bind(("0.0.0.0", CONFIG.port))?
    .run()
    .await?;

    Ok(())
}
