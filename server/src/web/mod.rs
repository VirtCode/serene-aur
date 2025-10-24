use crate::build::schedule::{BuildMeta, BuildScheduler};
use crate::build::{BuildSummary, Builder};
use crate::config::{CLI_PACKAGE_NAME, CONFIG, INFO};
use crate::database::{self, Database};
use crate::package;
use crate::package::srcinfo::SrcinfoGenerator;
use crate::package::{aur, source, Package};
use crate::repository::crypto::{get_public_key_bytes, should_sign_packages};
use crate::repository::PackageRepositoryInstance;
use crate::web::auth::{AuthRead, AuthWrite};
use crate::web::broadcast::Broadcast;
use actix_web::error::{ErrorBadRequest, ErrorInternalServerError, ErrorNotFound};
use actix_web::web::{Data, Json, Path, Query, Redirect};
use actix_web::{delete, get, post, Responder};
use auth::{create_webhook_secret, AuthWebhook};
use chrono::DateTime;
use cron::Schedule;
use hyper::StatusCode;
use serde::Deserialize;
use serene_data::build::BuildReason;
use serene_data::package::{
    PackageAddRequest, PackageAddSource, PackageBuildRequest, PackageSettingsRequest,
};
use serene_data::SereneInfo;
use std::str::FromStr;
use tokio::sync::Mutex;

mod auth;
pub mod broadcast;
mod data;

type BuildSchedulerData = Data<Mutex<BuildScheduler>>;
type BuilderData = Data<Builder>;
type SrcinfoGeneratorData = Data<Mutex<SrcinfoGenerator>>;

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

#[get("/")]
pub async fn info() -> actix_web::Result<impl Responder> {
    Ok(Json(SereneInfo {
        version: INFO.version.clone(),
        started: INFO.start_time,
        name: CONFIG.repository_name.clone(),
        architecture: CONFIG.architecture.clone(),
        readable: CONFIG.allow_reads,
        signed: should_sign_packages(),
    }))
}

#[post("/package/add")]
pub async fn add(
    _: AuthWrite,
    body: Json<PackageAddRequest>,
    db: Data<Database>,
    srcinfo_generator: SrcinfoGeneratorData,
    scheduler: BuildSchedulerData,
) -> actix_web::Result<impl Responder> {
    // get repo and devel tag
    let source = match &body.0.source {
        PackageAddSource::Aur { name } => {
            let package = aur::info(name)
                .await
                .internal()?
                .ok_or_else(|| ErrorNotFound(format!("aur package '{name}' does not exist")))?;

            source::aur::new(&package, false) // TODO: support the devel flag
        }
        PackageAddSource::Git { url, devel } => source::git::new(url, *devel),
        PackageAddSource::Raw { pkgbuild: src, devel } => source::raw::new(src, *devel),
    };

    // create package
    let packages = package::add_source(&db, &srcinfo_generator, source, body.replace)
        .await
        .internal()?
        .ok_or_else(|| ErrorBadRequest("package with the same base is already added"))?;

    let mut response = vec![];
    for package in &packages {
        let count = BuildSummary::count_for_package(&package.base, &db).await.internal()?;
        response.push(package.to_info(count));
    }

    {
        // scheduling package
        let mut scheduler = scheduler.lock().await;

        for p in &packages {
            scheduler.schedule(p).await.internal()?;
        }

        scheduler
            .run(packages, BuildMeta::new(BuildReason::Initial, body.resolve, true, false))
            .await
            .internal()?;
    }

    Ok(Json(response))
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
pub async fn status(
    _: AuthRead,
    package: Path<String>,
    db: Data<Database>,
) -> actix_web::Result<impl Responder> {
    let package = Package::find(&package, &db)
        .await
        .internal()?
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    let count = BuildSummary::count_for_package(&package.base, &db).await.internal()?;

    Ok(Json(package.to_info(count)))
}

#[get("/package/{name}/pkgbuild")]
pub async fn pkgbuild(
    _: AuthRead,
    package: Path<String>,
    db: Data<Database>,
) -> actix_web::Result<impl Responder> {
    let package = Package::find(&package, &db)
        .await
        .internal()?
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    Ok(Json(package.pkgbuild.ok_or_else(|| {
        ErrorNotFound("package was never built and has thus no used package build")
    })?))
}

#[derive(Deserialize)]
struct CountQuery {
    count: Option<u32>,
}

#[get("/package/{name}/build")]
pub async fn get_all_builds(
    _: AuthRead,
    package: Path<String>,
    Query(count): Query<CountQuery>,
    db: Data<Database>,
) -> actix_web::Result<impl Responder> {
    let builds = if let Some(count) = count.count {
        BuildSummary::find_latest_n_for_package(&package, count, &db).await.internal()?
    } else {
        BuildSummary::find_all_for_package(&package, &db).await.internal()?
    };

    Ok(Json(builds.iter().map(|b| b.as_info()).collect::<Vec<_>>()))
}

#[post("/build/all")]
pub async fn build_all(
    _: AuthWrite,
    db: Data<Database>,
    body: Json<PackageBuildRequest>,
    scheduler: BuildSchedulerData,
) -> actix_web::Result<impl Responder> {
    let packages = Package::find_all(&db)
        .await
        .internal()?
        .into_iter()
        .filter(|p| p.enabled)
        .collect::<Vec<_>>();

    scheduler
        .lock()
        .await
        .run(packages, BuildMeta::new(BuildReason::Manual, body.resolve, body.clean, body.force))
        .await
        .internal()?;

    Ok(empty_response())
}

#[post("/build")]
pub async fn build(
    _: AuthWrite,
    db: Data<Database>,
    body: Json<PackageBuildRequest>,
    scheduler: BuildSchedulerData,
) -> actix_web::Result<impl Responder> {
    let mut packages = vec![];

    for package in &body.packages {
        packages.push(
            Package::find(package, &db).await.internal()?.ok_or_else(|| {
                ErrorNotFound(format!("package with base {package} is not added"))
            })?,
        )
    }

    if packages.is_empty() {
        return Ok(empty_response());
    }

    scheduler
        .lock()
        .await
        .run(packages, BuildMeta::new(BuildReason::Manual, body.resolve, body.clean, body.force))
        .await
        .internal()?;

    Ok(empty_response())
}

async fn get_build_for(
    base: &str,
    time: &str,
    db: &Database,
) -> actix_web::Result<Option<BuildSummary>> {
    if time == "latest" {
        BuildSummary::find_latest_for_package(base, db).await.internal()
    } else if let Ok(n) = u32::from_str(time) {
        BuildSummary::find_nth_for_package(n, base, db).await.internal()
    } else if let Ok(date) = DateTime::from_str(time) {
        BuildSummary::find(&date, base, db).await.internal()
    } else {
        Err(ErrorBadRequest(format!(
            "expected valid date, valid index number, or 'latest', not '{time}'"
        )))
    }
}

#[get("/package/{name}/build/{time}")]
pub async fn get_build(
    _: AuthRead,
    path: Path<(String, String)>,
    db: Data<Database>,
) -> actix_web::Result<impl Responder> {
    let (package, time) = path.into_inner();

    Ok(Json(
        get_build_for(&package, &time, &db)
            .await?
            .map(|b| b.as_info())
            .ok_or_else(|| ErrorNotFound("package not found or no build at this time"))?,
    ))
}

#[get("/package/{name}/build/{time}/logs")]
pub async fn get_logs(
    _: AuthRead,
    path: Path<(String, String)>,
    db: Data<Database>,
) -> actix_web::Result<impl Responder> {
    let (package, time) = path.into_inner();

    let b = get_build_for(&package, &time, &db)
        .await?
        .ok_or_else(|| ErrorNotFound("package not found or no build at this time"))?;

    Ok(Json(
        database::log::read(&b)
            .await
            .internal()?
            .ok_or_else(|| ErrorNotFound("build does not have any logs"))?,
    ))
}

#[get("/package/{name}/build/logs/subscribe")]
pub async fn subscribe_logs(
    _: AuthRead,
    path: Path<String>,
    broadcast: Data<Broadcast>,
    db: Data<Database>,
) -> actix_web::Result<impl Responder> {
    let package = path.into_inner();
    let _ = Package::find(&package, &db)
        .await
        .internal()?
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    broadcast.subscribe(package).await
}

#[delete("/package/{name}")]
pub async fn remove(
    _: AuthWrite,
    package: Path<String>,
    db: Data<Database>,
    builder: BuilderData,
) -> actix_web::Result<impl Responder> {
    let package = Package::find(&package, &db)
        .await
        .internal()?
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    builder.run_remove(&package).await.internal()?;

    Ok(empty_response())
}

#[post("/package/{name}/set")]
pub async fn settings(
    _: AuthWrite,
    package: Path<String>,
    body: Json<PackageSettingsRequest>,
    db: Data<Database>,
    scheduler: BuildSchedulerData,
    srcinfo_generator: SrcinfoGeneratorData,
) -> actix_web::Result<impl Responder> {
    let mut package = Package::find(&package, &db)
        .await
        .internal()?
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    // get repo and devel tag
    let (reschedule, source) = match body.0 {
        PackageSettingsRequest::Clean(b) => {
            package.clean = b;
            (false, false)
        }
        PackageSettingsRequest::Private(b) => {
            package.private = b;
            (false, false)
        }
        PackageSettingsRequest::Enabled(b) => {
            package.enabled = b;
            (true, false)
        }
        PackageSettingsRequest::Dependency(b) => {
            package.dependency = b;
            (false, false)
        }
        PackageSettingsRequest::Schedule(s) => {
            if s.as_ref().and_then(|c| Schedule::from_str(c).err()).is_some() {
                return Err(ErrorBadRequest(
                    "cannot parse cron expression (you probably forgot the seconds)",
                ));
            }
            package.schedule = s;
            (true, false)
        }
        PackageSettingsRequest::Prepare(s) => {
            package.prepare = s;
            (false, false)
        }
        PackageSettingsRequest::Flags(f) => {
            package.flags = f;
            (false, false)
        }
        PackageSettingsRequest::Devel(b) => {
            package.source.devel = b;
            (true, true)
        }
        PackageSettingsRequest::SrcinfoOverride(b) => {
            package.source.srcinfo_override = b;
            (false, true)
        }
    };

    if reschedule {
        if package.enabled {
            scheduler.lock().await.schedule(&package).await.internal()?;
        } else {
            scheduler.lock().await.unschedule(&package).await.internal()?;
        }
    }

    if source {
        // update source if we have changed anything in it
        package.update(&srcinfo_generator).await.internal()?;

        package.change_sources(&db).await.internal()?;
    } else {
        package.change_settings(&db).await.internal()?;
    }

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

#[get("/webhook/package/{name}/secret")]
pub async fn get_webhook_secret(
    auth: AuthWrite,
    db: Data<Database>,
    package: Path<String>,
) -> actix_web::Result<impl Responder> {
    let _ = Package::find(&package, &db)
        .await
        .internal()?
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    create_webhook_secret(&package, &serene_data::secret::hash(auth.get_secret())).map(Json)
}

#[post("/webhook/package/{name}/build")]
pub async fn build_webhook(
    _: AuthWebhook,
    package: Path<String>,
    db: Data<Database>,
    scheduler: BuildSchedulerData,
) -> actix_web::Result<impl Responder> {
    let package = Package::find(&package, &db)
        .await
        .internal()?
        .ok_or_else(|| ErrorNotFound(format!("package with base {} is not added", &package)))?;

    scheduler
        .lock()
        .await
        .run(vec![package], BuildMeta::normal(BuildReason::Webhook))
        .await
        .internal()?;

    Ok(empty_response())
}

#[get("/cli")]
pub async fn get_cli_package(
    repository: Data<PackageRepositoryInstance>,
) -> actix_web::Result<impl Responder> {
    let repository = repository.lock().await;
    if let Some(filename) = repository.package_file(CLI_PACKAGE_NAME) {
        // should serene ever support multiple architectures we should get the
        // architecture based on the http user-agent header (in which pacman includes
        // the host architecture) => alternatively we could move the /cli
        // endpoint to /<arch>/cli
        Ok(Redirect::to(format!("/{}/{filename}", CONFIG.architecture)).temporary())
    } else {
        Err(ErrorNotFound(format!(
            "package '{CLI_PACKAGE_NAME}' does not exist or is not yet built"
        )))
    }
}

#[get("/{arch}/package/{name}")]
pub async fn get_package_by_name(
    path: Path<(String, String)>,
    repository: Data<PackageRepositoryInstance>,
) -> actix_web::Result<impl Responder> {
    let (arch, package) = path.into_inner();
    // should serene ever support multiple architectures we could match the provided
    // architecture against the available architectures. At the moment allowing to
    // provide an architecture is a bit useless.
    if arch != CONFIG.architecture {
        Err(ErrorBadRequest(format!("architecture '{arch}' is not supported by this server")))?
    }
    let repository = repository.lock().await;
    if let Some(filename) = repository.package_file(&package) {
        Ok(Redirect::to(format!("/{arch}/{filename}")).temporary())
    } else {
        Err(ErrorNotFound(format!("package '{package}' does not exist or is not yet built")))
    }
}
