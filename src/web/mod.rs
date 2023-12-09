use actix_web::{get, Responder};
use actix_web::web::Path;
use raur::Raur;
use crate::web::auth::Auth;

mod auth;

#[get("/package/status/{name}")]
pub async fn status(_: Auth, package: Path<String>) -> actix_web::Result<impl Responder> {
    Ok(package.into_inner())
}

#[get("/package/build/{name}")]
pub async fn build(_: Auth, package: Path<String>) -> actix_web::Result<impl Responder> {
    Ok(package.into_inner())
}

#[get("/package/remove/{name}")]
pub async fn remove(_: Auth, package: Path<String>) -> actix_web::Result<impl Responder> {
    Ok(package.into_inner())
}

#[get("/package/add")]
pub async fn add(_: Auth) -> actix_web::Result<impl Responder> {
    Ok("")
}