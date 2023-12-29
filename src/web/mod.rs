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
use crate::build::{BuildState, BuildSummary};
use crate::build::schedule::BuildScheduler;
use crate::package;
use crate::package::{aur, Package};
use crate::package::store::PackageStore;
use crate::web::auth::Auth;

mod auth;

type BuildSchedulerData = Data<RwLock<BuildScheduler>>;
type PackageStoreData = Data<RwLock<PackageStore>>;

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

#[derive(Serialize, Deserialize)]
struct PackagePeek {
    base: String,
    version: String,
    enabled: bool,
    devel: bool,
    build: Option<BuildPeek>
}

impl PackagePeek {
    pub fn create(package: &Package) -> Self {
        Self {
            base: package.base.clone(),
            enabled: package.enabled,
            devel: package.get_devel(),
            version: package.version.clone(),
            build: package.get_builds().iter()
                .max_by_key(|p| p.started)
                .map(BuildPeek::create)
        }
    }
}

#[derive(Serialize, Deserialize)]
struct PackageInfo {
    base: String,
    version: String,
    enabled: bool,
    devel: bool,
    clean: bool,
    schedule: String,
    added: DateTime<Utc>,
    builds: Vec<BuildPeek>
}

impl PackageInfo {
    pub fn create(package: &Package) -> Self {
        Self {
            base: package.base.clone(),
            enabled: package.enabled,
            devel: package.get_devel(),
            version: package.version.clone(),
            builds: package.get_builds().iter().map(BuildPeek::create).collect(),
            added: package.added,
            schedule: package.get_schedule(),
            clean: package.clean
        }
    }
}

#[derive(Serialize, Deserialize)]
struct BuildPeek {
    state: BuildState,
    version: Option<String>,
    started: DateTime<Utc>,
    ended: Option<DateTime<Utc>>
}

impl BuildPeek {
    pub fn create(summary: &BuildSummary) -> Self {
        Self {
            state: summary.state.clone(),
            version: summary.version.clone(),
            started: summary.started,
            ended: summary.ended
        }
    }
}


#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum PackageAddRequest {
    Aur { name: String },
    Custom { url: String, devel: bool }
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

    Ok(Json(PackagePeek::create(&package)))
}

#[get("/package/list")]
pub async fn list(_: Auth, store: PackageStoreData) -> actix_web::Result<impl Responder> {
    Ok(Json(
        store.read().await.peek().iter()
            .map(|p| PackagePeek::create(p))
            .collect::<Vec<PackagePeek>>()
    ))
}

#[get("/package/{name}")]
pub async fn status(_: Auth, package: Path<String>, store: PackageStoreData) -> actix_web::Result<impl Responder> {
    let package = store.read().await.get(&package)
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    Ok(Json(PackageInfo::create(&package)))
}

#[post("/package/{name}/build")]
pub async fn build(_: Auth, package: Path<String>, store: PackageStoreData, scheduler: BuildSchedulerData) -> actix_web::Result<impl Responder> {
    let package = store.read().await.get(&package)
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    scheduler.write().await.run(&package).await.internal()?;

    Ok(empty_response())
}

#[delete("/package/{name}")]
pub async fn remove(_: Auth, package: Path<String>, store: PackageStoreData, scheduler: BuildSchedulerData) -> actix_web::Result<impl Responder> {
    Ok(package.into_inner())
}

