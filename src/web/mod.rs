use std::sync::Arc;
use actix_web::{delete, get, post, Responder};
use actix_web::web::{Data, Json, Path};
use log::error;
use raur::Raur;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use crate::build::schedule::BuildScheduler;
use crate::package::PackageManager;
use crate::package::store::PackageStore;
use crate::web::auth::Auth;

mod auth;

type BuildSchedulerData = Data<RwLock<BuildScheduler>>;
type PackageStoreData = Data<RwLock<PackageStore>>;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum PackageAddRequest {
    Aur { name: String },
    Custom { url: String, devel: bool }
}

#[post("/package/add")]
pub async fn add(_: Auth, body: Json<PackageAddRequest>, store: PackageStoreData, scheduler: BuildSchedulerData) -> actix_web::Result<impl Responder> {
    let mut manager = PackageManager::new(store.into_inner());

    let result = match &body.0 {
        PackageAddRequest::Aur { name } => { manager.add_aur(name).await }
        PackageAddRequest::Custom { url, devel } => { manager.add_custom(url, *devel).await }
    };

    error!("{result:#?}");

    Ok("")
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

