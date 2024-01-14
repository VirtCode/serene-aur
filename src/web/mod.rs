use std::io::repeat;
use std::sync::Arc;
use actix_web::{delete, get, HttpResponse, post, Responder};
use actix_web::error::{ErrorBadRequest, ErrorInternalServerError, ErrorNotFound};
use actix_web::web::{Data, Json, Path};
use chrono::{DateTime, Utc};
use hyper::StatusCode;
use log::error;
use raur::Raur;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use serene_data::package::PackagePeek;
use crate::build::Builder;
use crate::build::schedule::BuildScheduler;
use crate::package;
use crate::package::{aur, Package};
use crate::package::store::PackageStore;
use crate::web::auth::Auth;
use crate::web::data::PackageAddRequest;

mod auth;
mod data;

type BuildSchedulerData = Data<RwLock<BuildScheduler>>;
type PackageStoreData = Data<RwLock<PackageStore>>;
type BuilderData = Data<RwLock<Builder>>;

pub trait InternalError<T> {
    fn internal(self) -> actix_web::Result<T>;
}

impl<T> InternalError<T> for anyhow::Result<T> {
    fn internal(self) -> actix_web::Result<T> {
        self.map_err(|e| ErrorInternalServerError(format!("{e:#}")))
    }
}

fn empty_response() -> impl Responder {
    (None::<Option<String>>, StatusCode::OK)
}

#[post("/package/add")]
pub async fn add(_: Auth, body: Json<PackageAddRequest>, store: PackageStoreData, scheduler: BuildSchedulerData) -> actix_web::Result<impl Responder> {

    // get repo and devel tag
    let (repository, devel) = match &body.0 {
        PackageAddRequest::Aur { name } => {
            let package = aur::find(&name).await.internal()?
                .ok_or_else(|| ErrorNotFound(format!("aur package '{}' does not exist", name)))?;

            (package.repository, package.devel)
        }
        PackageAddRequest::Custom { url, devel } => { (url.clone(), *devel) }
    };

    // create package
    let base = package::add_repository(store.clone().into_inner(), &repository, devel).await.internal()?
        .ok_or_else(|| ErrorBadRequest("package with the same base is already added"))?;

    let package = store.read().await.get(&base).ok_or_else(|| ErrorInternalServerError("failed to add package"))?;

    { // scheduling package
        let mut scheduler = scheduler.write().await;
        scheduler.schedule(&package).await.internal()?;
        scheduler.run(&package).await.internal()?;
    }

    Ok(Json(package.as_peek()))
}

#[get("/package/list")]
pub async fn list(_: Auth, store: PackageStoreData) -> actix_web::Result<impl Responder> {
    Ok(Json(
        store.read().await.peek().iter()
            .map(|p| p.as_peek())
            .collect::<Vec<PackagePeek>>()
    ))
}

#[get("/package/{name}")]
pub async fn status(_: Auth, package: Path<String>, store: PackageStoreData) -> actix_web::Result<impl Responder> {
    let package = store.read().await.get(&package)
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    Ok(Json(package.as_info()))
}

#[post("/package/{name}/build")]
pub async fn build(_: Auth, package: Path<String>, store: PackageStoreData, scheduler: BuildSchedulerData) -> actix_web::Result<impl Responder> {
    let package = store.read().await.get(&package)
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    scheduler.write().await.run(&package).await.internal()?;

    Ok(empty_response())
}

#[get("/package/{name}/build/{time}")]
pub async fn get_build(_: Auth, path: Path<(String, DateTime<Utc>)>, store: PackageStoreData) -> actix_web::Result<impl Responder> {
    let (package, time) = path.into_inner();

    let package = store.read().await.get(&package)
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    Ok(Json(
        package.get_builds().iter()
            .find(|b| b.started == time)
            .map(|b| b.as_info())
            .ok_or_else(|| ErrorNotFound("not build at this time found"))?
    ))
}

#[get("/package/{name}/build/{time}/logs")]
pub async fn get_logs(_: Auth, path: Path<(String, DateTime<Utc>)>, store: PackageStoreData) -> actix_web::Result<impl Responder> {
    let (package, time) = path.into_inner();

    let package = store.read().await.get(&package)
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    Ok(Json(
        package.get_builds().iter()
            .find(|b| b.started == time)
            .and_then(|b| b.logs.clone())
            .map(|s| s.logs)
            .ok_or_else(|| ErrorNotFound("no build at this time or it has no logs"))?
    ))
}

#[delete("/package/{name}")]
pub async fn remove(_: Auth, package: Path<String>, store: PackageStoreData, builder: BuilderData) -> actix_web::Result<impl Responder> {
    let package = store.read().await.get(&package)
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    store.write().await.remove(&package.base).await.internal()?;

    builder.write().await.run_remove(&package).await.internal()?;

    Ok(empty_response())
}

