use std::str::FromStr;
use actix_web::{delete, get, post, Responder};
use actix_web::error::{ErrorBadRequest, ErrorInternalServerError, ErrorNotFound};
use actix_web::web::{Data, Json, Path, Query};
use chrono::{DateTime};
use hyper::StatusCode;
use sequoia_openpgp::parse::Parse;
use serde::Deserialize;
use tokio::sync::RwLock;
use serene_data::package::{PackageAddRequest, PackageAddSource, PackageBuildRequest, PackageSettingsRequest};
use crate::build::{Builder, BuildSummary};
use crate::build::schedule::BuildScheduler;
use crate::database::Database;
use crate::package;
use crate::package::{aur, Package};
use crate::package::source::devel::DevelGitSource;
use crate::package::source::normal::NormalSource;
use crate::package::source::single::SingleSource;
use crate::package::source::Source;
use crate::repository::crypto::{get_public_key_bytes, should_sign_packages};
use crate::web::auth::{AuthRead, AuthWrite};
use crate::web::broadcast::Broadcast;

mod auth;
mod data;
pub mod broadcast;

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
pub async fn add(_: AuthWrite, body: Json<PackageAddRequest>, db: Data<Database>, scheduler: BuildSchedulerData) -> actix_web::Result<impl Responder> {

    // get repo and devel tag
    let source: Box<dyn Source + Sync + Send> = match &body.0.source {
        PackageAddSource::Aur { name } => {
            let package = aur::find(name).await.internal()?
                .ok_or_else(|| ErrorNotFound(format!("aur package '{}' does not exist", name)))?;

            if package.devel {
                Box::new(DevelGitSource::empty(&package.repository))
            } else {
                Box::new(NormalSource::empty(&package.repository))
            }
        }
        PackageAddSource::Custom { url, devel } => {
            if *devel {
                Box::new(DevelGitSource::empty(url))
            } else {
                Box::new(NormalSource::empty(url))
            }
        }
        PackageAddSource::Single { pkgbuild: source, devel } => {
            Box::new(SingleSource::initialize(source.to_owned(), *devel))
        }
    };

    // create package
    let package = package::add_source(&db, source, body.0.replace).await.internal()?
        .ok_or_else(|| ErrorBadRequest("package with the same base is already added"))?;

    { // scheduling package
        let mut scheduler = scheduler.write().await;
        scheduler.schedule(&package).await.internal()?;
        scheduler.run(&package, true).await.internal()?;
    }

    Ok(Json(package.to_info()))
}

#[get("/package/list")]
pub async fn list(_: AuthRead, db: Data<Database>) -> actix_web::Result<impl Responder> {
    let package = Package::find_all(&db).await.internal()?;

    let mut peeks = vec![];

    for p in package {
        // retrieve latest build
        let b = BuildSummary::find_latest_for_package(&p.base, &db).await.internal()?;

        peeks.push(p.to_peek(b));
    }

    Ok(Json(peeks))
}

#[get("/package/{name}")]
pub async fn status(_: AuthRead, package: Path<String>, db: Data<Database>) -> actix_web::Result<impl Responder> {
    let package = Package::find(&package, &db).await.internal()?
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    Ok(Json(package.to_info()))
}

#[get("/package/{name}/pkgbuild")]
pub async fn pkgbuild(_: AuthRead, package: Path<String>, db: Data<Database>) -> actix_web::Result<impl Responder> {
    let package = Package::find(&package, &db).await.internal()?
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    Ok(Json(package.pkgbuild.ok_or_else(|| ErrorNotFound("package was never built and has thus no used package build"))?))
}

#[derive(Deserialize)]
struct CountQuery {
    count: Option<u32>
}

#[get("/package/{name}/build")]
pub async fn get_all_builds(_: AuthRead, package: Path<String>, Query(count): Query<CountQuery>, db: Data<Database>) -> actix_web::Result<impl Responder> {
    let builds = if let Some(count) = count.count {
        BuildSummary::find_latest_n_for_package(&package, count, &db).await.internal()?
    } else {
        BuildSummary::find_all_for_package(&package, &db).await.internal()?
    };

    Ok(Json(builds.iter().map(|b| b.as_info()).collect::<Vec<_>>()))
}

#[post("/package/{name}/build")]
pub async fn build(_: AuthWrite, package: Path<String>, db: Data<Database>, body: Json<PackageBuildRequest>, scheduler: BuildSchedulerData) -> actix_web::Result<impl Responder> {
    let package = Package::find(&package, &db).await.internal()?
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    scheduler.write().await.run(&package, body.into_inner().clean).await.internal()?;

    Ok(empty_response())
}

async fn get_build_for(base: &str, time: &str, db: &Database) -> actix_web::Result<Option<BuildSummary>> {
    match time {
        "latest" =>
            BuildSummary::find_latest_for_package(base, &db).await.internal(),
        t =>
            BuildSummary::find(&DateTime::from_str(t).map_err(|_| ErrorBadRequest(format!("expected valid date or 'latest', not '{time}'")))?, base, &db).await.internal()
    }
}

#[get("/package/{name}/build/{time}")]
pub async fn get_build(_: AuthRead, path: Path<(String, String)>, db: Data<Database>) -> actix_web::Result<impl Responder> {
    let (package, time) = path.into_inner();

    Ok(Json(
        get_build_for(&package, &time, &db).await?
            .map(|b| b.as_info())
            .ok_or_else(|| ErrorNotFound("package not found or no build at this time"))?
    ))
}

#[get("/package/{name}/build/{time}/logs")]
pub async fn get_logs(_: AuthRead, path: Path<(String, String)>, db: Data<Database>) -> actix_web::Result<impl Responder> {
    let (package, time) = path.into_inner();

    Ok(Json(
        get_build_for(&package, &time, &db).await?
            .and_then(|s| s.logs)
            .map(|l| l.logs)
            .ok_or_else(|| ErrorNotFound("package not found, no build at this time or it has no logs"))?
    ))
}

#[get("/package/{name}/build/logs/subscribe")]
pub async fn subscribe_logs(_: AuthRead, path: Path<String>, broadcast: Data<Broadcast>, db: Data<Database>) -> actix_web::Result<impl Responder> {
    let package = path.into_inner();
    let _ = Package::find(&package, &db).await.internal()?
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;
    
    broadcast.subscribe(package).await
}

#[delete("/package/{name}")]
pub async fn remove(_: AuthWrite, package: Path<String>, db: Data<Database>, builder: BuilderData) -> actix_web::Result<impl Responder> {
    let package = Package::find(&package, &db).await.internal()?
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    builder.write().await.run_remove(&package).await.internal()?;

    Ok(empty_response())
}

#[post("/package/{name}/set")]
pub async fn settings(_: AuthWrite, package: Path<String>, body: Json<PackageSettingsRequest>, db: Data<Database>, scheduler: BuildSchedulerData) -> actix_web::Result<impl Responder> {
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
            package.schedule =
                if s.trim().is_empty() { None }
                else { Some(s.clone()) };

            true
        }
        PackageSettingsRequest::Prepare(s) => {
            package.prepare =
                if s.trim().is_empty() { None }
                else { Some(s.clone()) };

            false
        }
        PackageSettingsRequest::Flags(f) => {
            package.flags = f.clone();

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

#[get("/key")]
pub async fn get_signature_public_key(_: AuthRead) -> actix_web::Result<impl Responder> {
    if !should_sign_packages() {
        Err(ErrorNotFound("the server has no private key to sign packages"))?
    }

    let mut body = vec![];
    get_public_key_bytes(&mut body).map_err(|err| ErrorInternalServerError(err.to_string()))?;

    Ok(body)
}

