use actix_web::{delete, get, post, Responder};
use actix_web::error::{ErrorBadRequest, ErrorInternalServerError, ErrorNotFound};
use actix_web::web::{Data, Json, Path};
use async_std::stream;
use chrono::{DateTime, Utc};
use hyper::StatusCode;
use tokio::sync::RwLock;
use serene_data::package::{PackageAddRequest, PackagePeek, PackageSettingsRequest};
use crate::build::{Builder, BuildSummary};
use crate::build::schedule::BuildScheduler;
use crate::database::Database;
use crate::package;
use crate::package::{aur, Package};
use crate::web::auth::Auth;

mod auth;
mod data;

type BuildSchedulerData = Data<RwLock<BuildScheduler>>;
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
pub async fn add(_: Auth, body: Json<PackageAddRequest>, db: Data<Database>, scheduler: BuildSchedulerData) -> actix_web::Result<impl Responder> {

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
    let package = package::add_repository(&db, &repository, devel).await.internal()?
        .ok_or_else(|| ErrorBadRequest("package with the same base is already added"))?;

    { // scheduling package
        let mut scheduler = scheduler.write().await;
        scheduler.schedule(&package).await.internal()?;
        scheduler.run(&package).await.internal()?;
    }

    Ok(Json(package.to_peek(&db).await.internal()?))
}

#[get("/package/list")]
pub async fn list(_: Auth, db: Data<Database>) -> actix_web::Result<impl Responder> {
    let package = Package::find_all(&db).await.internal()?;

    // TODO: make efficient
    let mut peeks = vec![];
    for p in package {
        peeks.push(p.to_peek(&db).await.internal()?);
    }

    Ok(Json(peeks))
}

#[get("/package/{name}")]
pub async fn status(_: Auth, package: Path<String>, db: Data<Database>) -> actix_web::Result<impl Responder> {
    let package = Package::find(&package, &db).await.internal()?
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    Ok(Json(package.to_info(&db).await.internal()?))
}

#[post("/package/{name}/build")]
pub async fn build(_: Auth, package: Path<String>, db: Data<Database>, scheduler: BuildSchedulerData) -> actix_web::Result<impl Responder> {
    let package = Package::find(&package, &db).await.internal()?
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    scheduler.write().await.run(&package).await.internal()?;

    Ok(empty_response())
}

#[get("/package/{name}/build/{time}")]
pub async fn get_build(_: Auth, path: Path<(String, DateTime<Utc>)>, db: Data<Database>) -> actix_web::Result<impl Responder> {
    let (package, time) = path.into_inner();

    let package = Package::find(&package, &db).await.internal()?
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    Ok(Json(
        BuildSummary::find(&time, &package, &db).await.internal()?
            .map(|b| b.as_info())
            .ok_or_else(|| ErrorNotFound("not build at this time found"))?
    ))
}

#[get("/package/{name}/build/{time}/logs")]
pub async fn get_logs(_: Auth, path: Path<(String, DateTime<Utc>)>, db: Data<Database>) -> actix_web::Result<impl Responder> {
    let (package, time) = path.into_inner();

    let package = Package::find(&package, &db).await.internal()?
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    Ok(Json(
        BuildSummary::find(&time, &package, &db).await.internal()?
            .map(|s| s.logs)
            .ok_or_else(|| ErrorNotFound("no build at this time or it has no logs"))?
    ))
}

#[delete("/package/{name}")]
pub async fn remove(_: Auth, package: Path<String>, db: Data<Database>, builder: BuilderData) -> actix_web::Result<impl Responder> {
    let package = Package::find(&package, &db).await.internal()?
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    builder.write().await.run_remove(&package).await.internal()?;

    Ok(empty_response())
}

#[post("/package/{name}/set")]
pub async fn settings(_: Auth, package: Path<String>, body: Json<PackageSettingsRequest>, db: Data<Database>, scheduler: BuildSchedulerData) -> actix_web::Result<impl Responder> {
    let mut package = Package::find(&package, &db).await.internal()?
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    // get repo and devel tag
    let reschedule = match &body.0 {
        PackageSettingsRequest::Clean(b) => {
            package.clean = *b;
            false
        }
        PackageSettingsRequest::Enabled(b) => {
            package.enabled = *b;
            true
        }
        PackageSettingsRequest::Schedule(s) => {
            package.schedule = Some(s.clone());
            true
        }
        PackageSettingsRequest::Prepare(s) => {
            package.prepare = Some(s.clone());
            false
        }
    };

    if reschedule {
        if package.enabled { scheduler.write().await.schedule(&package).await.internal()?; }
        else { scheduler.write().await.unschedule(&package).await.internal()?; }
    }

    package.change_settings(&db).await.internal()?;

    Ok(empty_response())
}

