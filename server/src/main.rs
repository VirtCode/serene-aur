pub mod runner;
pub mod package;

mod repository;
mod web;
pub mod config;
mod build;
mod database;

use std::collections::HashSet;
use std::ops::Deref;
use std::process::exit;
use std::sync::Arc;
use actix_web::{App, HttpMessage, HttpServer};
use actix_web::web::Data;
use alpm::{Alpm, SigLevel};
use anyhow::Context;
use aur_depends::{Flags, PkgbuildRepo, Resolver};
use config::INFO;
use log::{error, info, warn};
use package::resolve::sync::{initialize_alpm, synchronize_alpm};
use tokio::sync::{RwLock};
use crate::build::schedule::{BuildScheduler, ImageScheduler};
use crate::build::Builder;
use crate::build::next::BuildResolver;
use crate::config::CONFIG;
use crate::package::Package;
use crate::package::resolve::AurResolver;
use crate::runner::{Runner};
use crate::repository::PackageRepository;
use crate::web::broadcast::Broadcast;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    // this is mainly here to initialize the lazy INFO struct
    info!("starting serene version {}", INFO.version);

    // initializing database
    let db = database::connect().await?;

    // initialize broadcast
    let broadcast = Broadcast::new();

    // initializing runner
    let runner = Arc::new(RwLock::new(
        Runner::new(broadcast.clone())
            .context("failed to connect to docker")?
    ));

    // initializing repository
    let repository = Arc::new(RwLock::new(
        PackageRepository::new().await
            .context("failed to create package repository")?
    ));

    // initializing builder
    let builder = Arc::new(RwLock::new(
        Builder::new(db.clone(), runner.clone(), repository.clone(), broadcast.clone())
    ));

    // creating scheduler
    let mut schedule = BuildScheduler::new(builder.clone()).await
        .context("failed to start package scheduler")?;

    // creating image scheduler
    let image_scheduler = ImageScheduler::new(runner.clone()).await
        .context("failed to start image scheduler")?;

    // schedule packages
    for package in Package::find_all(&db).await? {
        schedule.schedule(&package).await
            .context(format!("failed to start schedule for package {}", &package.base))?;
    }

    // pull image before cli build
    if let Err(e) = runner.read().await.update_image().await {
        error!("failed to update runner image on startup: {e:#}");
    }

    // add cli if enabled
    if config::CONFIG.build_cli {
        if let Err(e) = package::try_add_cli(&db, &mut schedule).await {
            error!("Failed to add cli package: {e:#}")
        }
    }

    image_scheduler.start().await?;
    schedule.start().await?;

    let schedule = Arc::new(RwLock::new(schedule));

    info!("serene started successfully on port {}!", CONFIG.port);
    // web app
    HttpServer::new(move ||
        App::new()
            .app_data(Data::new(db.clone()))
            .app_data(Data::from(schedule.clone()))
            .app_data(Data::from(builder.clone()))
            .app_data(Data::from(broadcast.clone()))
            .service(repository::webservice())
            .service(web::info)
            .service(web::add)
            .service(web::list)
            .service(web::status)
            .service(web::remove)
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
    ).bind(("0.0.0.0", CONFIG.port))?.run().await?;

    Ok(())
}
