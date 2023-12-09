use std::time::{Duration, SystemTime, UNIX_EPOCH};
use anyhow::Context;
use bollard::container::{Config, CreateContainerOptions, ListContainersOptions, LogsOptions, StartContainerOptions, WaitContainerOptions};
use bollard::Docker;
use bollard::models::{ Mount, MountTypeEnum};
use bollard::secret::{ContainerWaitResponse};
use bollard::secret::HostConfig;
use futures_util::StreamExt;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::task::JoinHandle;
use crate::package::Package;

const RUNNER_IMAGE: &str = "serene-aur-runner:latest";

#[derive(Debug)]
pub struct BuildStatus {
    pub success: bool,
    pub fatal: bool,
    pub logs: String,

    started: SystemTime,
    duration: Duration,
}

pub struct Builder {
    docker: Docker,
    input: Receiver<Package>,
    output: Sender<(Package, BuildStatus)>
}

impl Builder {
    fn new(rx: Receiver<Package>) -> anyhow::Result<(Self, Receiver<(Package, BuildStatus)>)> {
        let (tx, outgoing) = channel(32);

        Ok((Self {
            docker: Docker::connect_with_socket_defaults()?,
            input: rx,
            output: tx
        }, outgoing))
    }

    fn start(&mut self) -> JoinHandle<()> {
        tokio::spawn(async {
            loop {
                // retrieve next package from channel
                let next = self.input.recv().await.expect("builder input dropped");

                // run build
                let result = build(&self.docker, &next, next.clean).await;
                let status = match result {
                    Ok(status) => { status }
                    Err(e) => { BuildStatus {
                        success: false,
                        fatal: true,
                        logs: e.to_string(),
                        started: SystemTime::now(),
                        duration: Default::default(),
                    }}
                };

                // send package away
                self.output.send((next, status)).await.expect("builder output dropped");
            }
        })
    }
}


pub async fn build(docker: &Docker, package: &Package, clean: bool) -> anyhow::Result<BuildStatus> {
    let time = SystemTime::now();

    // create or find container
    let id = if let Some(id) = find(docker, package).await? {
        id
    } else {
        create(docker, package).await?
    };

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

    // remove container if clean build
    if clean {
        docker.remove_container(&id, None).await?;
    }

    Ok(BuildStatus {
        success: result.first().and_then(|r| r.as_ref().ok()).is_some(),
        fatal: false,
        logs: logs.join(""),
        started: time,
        duration: end.duration_since(time).expect("should work")
    })
}

async fn find(docker: &Docker, package: &Package) -> anyhow::Result<Option<String>> {
    let summary = docker.list_containers::<String>(Some(ListContainersOptions {
        all: true,
        .. Default::default()
    })).await?;

    if let Some(s) = summary.iter().find(|s| {
        s.names.as_ref().map(|v| v.contains(&format!("/{}", container_name(package)))).unwrap_or_default()
    }) {
        Ok(Some(s.id.clone().context("container does not have id")?))
    } else {
        Ok(None)
    }
}

async fn create(docker: &Docker, package: &Package) -> anyhow::Result<String> {
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
        image: Some(RUNNER_IMAGE),
        host_config: Some(host_config),
        ..Default::default()
    };

    let options = CreateContainerOptions {
        name: container_name(&package),
        ..Default::default()
    };

    Ok(docker.create_container(Some(options), config).await?.id)
}

fn container_name(package: &Package) -> String{
    format!("serene-aur-runner-{}", package.get_id())
}