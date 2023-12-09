mod source;
mod build;
pub mod package;

mod repository;

use std::collections::HashMap;
use std::error::Error;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use actix_web::{App, HttpServer};
use bollard::container::{Config, CreateContainerOptions, ListContainersOptions, LogsOptions, StartContainerOptions, WaitContainerOptions};
use bollard::Docker;
use bollard::exec::{CreateExecOptions, StartExecResults};
use raur::{Package, Raur};
use futures::stream::StreamExt;

#[tokio::main]
async fn main_web() -> std::io::Result<()> {
    HttpServer::new(||
        App::new()
            .service(actix_files::Files::new("/repo/x86_64", "runner/app/builds/nvm").show_files_listing())
    ).bind(("127.0.0.1", 8080))?.run().await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>>{
    let docker = Docker::connect_with_socket_defaults()?;

    let package = package::get_from_aur("hyprland-git")?;
    let result = build::build(&docker, &package, false).await?;
    repository::update(package, &mut vec![], "aur")?;

    println!("{}", result.logs);
    println!("{result:#?}");

    Ok(())
}
