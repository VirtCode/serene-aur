mod source;
mod build;
pub mod package;

mod repository;
mod web;

use std::any;
use std::collections::HashMap;
use std::error::Error;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use actix_web::{App, HttpServer};
use actix_web::web::Data;
use bollard::container::{Config, CreateContainerOptions, ListContainersOptions, LogsOptions, StartContainerOptions, WaitContainerOptions};
use bollard::Docker;
use bollard::exec::{CreateExecOptions, StartExecResults};
use raur::{Package, Raur};
use futures::stream::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {

    let docker = Docker::connect_with_socket_defaults()?;

    HttpServer::new(move ||
        App::new()
            .service(repository::webservice())
            .service(web::status)
            .service(web::add)
            .service(web::remove)
            .service(web::build)
    ).bind(("127.0.0.1", 8080))?.run().await?;

    Ok(())
}

#[tokio::main]
async fn main_old() -> Result<(), Box<dyn Error>>{
    let docker = Docker::connect_with_socket_defaults()?;

    let package = package::get_from_aur("hyprland-git")?;
    let result = build::build(&docker, &package, false).await?;
    repository::update(package, &mut vec![], "aur")?;

    println!("{}", result.logs);
    println!("{result:#?}");

    Ok(())
}
