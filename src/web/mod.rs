use std::io::repeat;
use std::sync::Arc;
use actix_web::{delete, get, post, Responder};
use actix_web::error::{ErrorBadRequest, ErrorInternalServerError, ErrorNotFound};
use actix_web::web::{Data, Json, Path};
use chrono::{DateTime, Utc};
use log::error;
use raur::Raur;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
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

#[derive(Serialize, Deserialize)]
struct PackageInfo {
    base: String,
    added: DateTime<Utc>,
    enabled: bool,
    clean: bool,
    schedule: String,
}

impl PackageInfo {
    pub fn create(package: &Package) -> Self {
        Self {
            base: package.base.clone(),
            added: package.added.clone(),
            enabled: package.enabled,
            clean: package.clean,
            schedule: package.get_schedule()
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

    Ok(Json(PackageInfo::create(&package)))
}

#[get("/package/{name}")]
pub async fn status(_: Auth, package: Path<String>) -> actix_web::Result<impl Responder> {
    Ok(package.into_inner())
}

#[post("/package/{name}/build")]
pub async fn build(_: Auth, package: Path<String>, store: PackageStoreData, scheduler: BuildSchedulerData) -> actix_web::Result<impl Responder> {
    let mut a = store.read().await.get(&package.into_inner()).unwrap();
    scheduler.write().await.run(&a).await.unwrap();
    Ok("asdf")
}

#[delete("/package/{name}")]
pub async fn remove(_: Auth, package: Path<String>, store: PackageStoreData, scheduler: BuildSchedulerData) -> actix_web::Result<impl Responder> {
    Ok(package.into_inner())
}

