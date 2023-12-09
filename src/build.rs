use std::collections::HashMap;
use std::error::{Error, };
use std::fmt::format;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use bollard::container::{Config, CreateContainerOptions, LogsOptions, StartContainerOptions, WaitContainerOptions};
use bollard::Docker;
use bollard::models::{ Mount, MountTypeEnum};
use bollard::secret::{ContainerWaitExitError, ContainerWaitResponse};
use bollard::secret::HostConfig;
use futures_util::StreamExt;
use crate::package::Package;

const RUNNER_IMAGE: &str = "serene-aur-runner:latest";

#[derive(Debug)]
pub struct BuildStatus {
    pub success: bool,
    pub logs: String,

    started: SystemTime,
    duration: Duration,
}

pub async fn build(docker: &Docker, package: &Package, clean: bool) -> anyhow::Result<BuildStatus> {
    let time = SystemTime::now();

    // TODO: reuse container if is any

    // create container
    let id = create(docker, package).await?;

    // start container
    docker.start_container(&id, None::<StartContainerOptions<String>>).await?;

    // wait for container to exit and collect logs
    let result: Vec<Result<ContainerWaitResponse, bollard::errors::Error>> = docker.wait_container(&id,  None::<WaitContainerOptions<String>>).collect().await;

    let end = SystemTime::now();

    let logs: Vec<String> = docker.logs::<String>(&id, Some(LogsOptions {
        stdout: true, stderr: true,
        since: time.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64,
        ..Default::default()
    })).filter_map(|r| async {
        r.ok().map(|c| c.to_string())
    }).collect::<Vec<_>>().await;

    Ok(BuildStatus {
        success: result.first().and_then(|r| r.as_ref().ok()).is_some(),
        logs: logs.join(""),
        started: time,
        duration: end.duration_since(time).expect("should work")
    })
}

pub async fn create(docker: &Docker, package: &Package) -> anyhow::Result<String> {
    let volume = Mount {
        target: Some("/app".to_string()),
        source: Some(format!("{}/{}", std::env::var("VOLUME_HOST")?, package.get_id())),
        typ: Some(MountTypeEnum::BIND),
        ..Default::default()
    };

    let host_config = HostConfig {
        mounts: Some(vec![volume]),
        ..Default::default()
    };

    let config = Config {
        image: Some("serene-aur-runner:latest"),
        host_config: Some(host_config),
        ..Default::default()
    };

    let options = CreateContainerOptions {
        name: format!("serene-aur-runner-{}", package.get_id()),
        ..Default::default()
    };

    Ok(docker.create_container(Some(options), config).await?.id)
}