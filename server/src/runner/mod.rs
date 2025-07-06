pub mod archive;
pub mod update;

use crate::config::{CONFIG, INFO};
use crate::package::Package;
use crate::runner::archive::{InputArchive, OutputArchive};
use crate::web::broadcast::{Broadcast, BROADCAST};
use anyhow::Context;
use async_tar::Archive;
use bollard::container::{
    Config, CreateContainerOptions, DownloadFromContainerOptions, ListContainersOptions,
    LogsOptions, StartContainerOptions, UploadToContainerOptions, WaitContainerOptions,
};
use bollard::image::{CreateImageOptions, PruneImagesOptions};
use bollard::{Docker, API_DEFAULT_VERSION};
use chrono::{DateTime, Utc};
use futures_util::{AsyncRead, StreamExt, TryStreamExt};
use hyper::body::HttpBody;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::sync::Arc;
use std::vec;
use tokio_util::compat::TokioAsyncReadCompatExt;
use tokio_util::io::StreamReader;

const RUNNER_IMAGE_BUILD_IN: &str = "/app/build";
const RUNNER_IMAGE_BUILD_OUT: &str = "/app/target";

const RUNNER_IMAGE_BULID_ENTRY: &str = "./build.sh";
const RUNNER_IMAGE_SRCINFO_ENTRY: &str = "./srcinfo.sh";

/// this is the status of a build run through the runner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStatus {
    pub success: bool,
    pub logs: String,

    pub started: DateTime<Utc>,
    pub ended: DateTime<Utc>,
}

pub type ContainerId = String;
pub type RunnerInstance = Arc<Runner>;

/// this is a wrapper for docker which creates and interacts with runner
/// containers
pub struct Runner {
    pub docker: Docker
}

impl Runner {
    /// creates a new runner by taking the docker from the default socket
    pub fn new() -> anyhow::Result<Self> {
        let docker = if let Some(url) = &CONFIG.docker_url {
            if url.starts_with("tcp://") || url.starts_with("http://") {
                info!("using docker via tcp at '{url}'");
                Docker::connect_with_http(url, 120, API_DEFAULT_VERSION)
            } else {
                if !url.starts_with("unix://") {
                    debug!("missing docker url scheme, assuming path to unix socket");
                }

                info!("using docker via unix socket at '{url}'");
                Docker::connect_with_unix(url, 120, API_DEFAULT_VERSION)
            }
        } else {
            info!("using docker via the default unix socket");
            Docker::connect_with_unix_defaults()
        };

        Ok(Self { docker: docker.context("failed to initialize docker")? })
    }

    /// runs the container
    pub async fn run(
        &self,
        container: &ContainerId,
        broadcast_target: Option<String>,
    ) -> anyhow::Result<RunStatus> {
        let start = Utc::now();

        // start container
        self.docker.start_container(container, None::<StartContainerOptions<String>>).await?;

        // retrieve logs
        let log_options = LogsOptions {
            stdout: true,
            stderr: true,
            follow: true, // follow is needed since we continuously read from the stream
            since: start.timestamp(),
            ..Default::default()
        };

        let mut stream = self.docker.logs::<String>(container, Some(log_options));

        let log_collector = tokio::spawn(async move {
            let mut logs = vec![];

            // collect logs from stream until the container exits (and the log stream
            // closes)
            while let Some(next) = stream.next().await {
                if let Ok(log) = next {
                    let value = log.to_string();

                    logs.push(value.clone());
                    if let Some(base) = &broadcast_target {
                        BROADCAST.log(base, value).await;
                    }
                }
            }
            logs.join("")
        });

        // wait for container to exit
        let result = self
            .docker
            .wait_container(container, None::<WaitContainerOptions<String>>)
            .collect::<Vec<_>>()
            .await;

        let end = Utc::now();

        // get logs from log collector thread
        let logs = log_collector.await.unwrap_or_default();

        Ok(RunStatus {
            success: result.first().and_then(|r| r.as_ref().ok()).is_some(),
            logs,
            started: start,
            ended: end,
        })
    }

    /// downloads the built directory from the container
    pub async fn download_outputs(
        &self,
        container: &ContainerId,
    ) -> anyhow::Result<OutputArchive<impl AsyncRead + Unpin>> {
        let options = DownloadFromContainerOptions { path: RUNNER_IMAGE_BUILD_OUT };

        let stream = self
            .docker
            .download_from_container(container, Some(options))
            .map(|b| b.map_err(std::io::Error::other));
        let reader = StreamReader::new(stream);

        OutputArchive::new(reader.compat())
    }

    /// uploads files to the build directory in a container
    pub async fn upload_inputs(
        &self,
        container: &ContainerId,
        files: InputArchive,
    ) -> anyhow::Result<()> {
        let options = UploadToContainerOptions {
            path: RUNNER_IMAGE_BUILD_IN,
            no_overwrite_dir_non_dir: "false",
        };

        self.docker
            .upload_to_container(container, Some(options), files.finish().await?)
            .await
            .context("could not upload sources to docker container")?;

        Ok(())
    }

    /// prepares a container for srcinfo generation
    pub async fn prepare_srcinfo_container(&self, clean: bool) -> anyhow::Result<ContainerId> {
        self.prepare_container(&CONFIG.container_srcinfo_name, RUNNER_IMAGE_SRCINFO_ENTRY, clean)
            .await
    }

    /// prepares a container for a package build
    pub async fn prepare_build_container(
        &self,
        package: &Package,
        clean: bool,
    ) -> anyhow::Result<ContainerId> {
        self.prepare_container(&container_name(package), RUNNER_IMAGE_BULID_ENTRY, clean).await
    }

    /// prepares a container based on the runner image
    /// either creates a new one or takes an old one which was already created
    pub async fn prepare_container(
        &self,
        name: &str,
        entrypoint: &str,
        clean: bool,
    ) -> anyhow::Result<ContainerId> {
        // try recycle old container
        if let Some(id) = self.find_container(name).await? {
            'check: {
                if clean {
                    info!("recreating container {name} because of clean build");
                    break 'check;
                }

                let Some(config) = self.docker.inspect_container(&id, None).await?.config else {
                    warn!("updating container {name}, because container is inaccessible");
                    break 'check;
                };

                if config.image != Some(target_docker_image()) {
                    info!("updating container {name}, image was {:?}", config.image);
                    break 'check;
                }

                if config.entrypoint != Some(vec![entrypoint.to_owned()]) {
                    info!(
                        "updating container {name}, entrypoint was {:?} and not {entrypoint}",
                        config.entrypoint
                    );
                    break 'check;
                }

                return Ok(id);
            }

            self.clean(&id).await.context("could not remove container whilst update")?;
        }

        Ok(self.create_container(name, entrypoint).await?)
    }

    /// finds an already created container under a name
    pub async fn find_container(&self, name: &str) -> anyhow::Result<Option<ContainerId>> {
        let summary = self
            .docker
            .list_containers::<String>(Some(ListContainersOptions {
                all: true,
                ..Default::default()
            }))
            .await?;

        if let Some(s) = summary.iter().find(|s| {
            s.names.as_ref().map(|v| v.contains(&format!("/{}", name))).unwrap_or_default()
        }) {
            Ok(Some(s.id.clone().context("container does not have id")?))
        } else {
            Ok(None)
        }
    }

    /// creates a new container given name and entry point
    async fn create_container(&self, name: &str, entrypoint: &str) -> anyhow::Result<ContainerId> {
        let config = Config {
            image: Some(target_docker_image()),
            entrypoint: Some(vec![entrypoint.to_owned()]),
            ..Default::default()
        };

        let options = CreateContainerOptions { name, ..Default::default() };

        Ok(self.docker.create_container(Some(options), config).await?.id)
    }

    pub async fn update_image(&self) -> anyhow::Result<()> {
        info!("updating runner image");

        let results = self
            .docker
            .create_image(
                Some(CreateImageOptions {
                    from_image: target_docker_image(),
                    ..Default::default()
                }),
                None,
                None,
            )
            .collect::<Vec<Result<_, _>>>()
            .await;

        // can this be directly collected into a result? probably... but streams suck
        let _statuses = results
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .context("failed to pull new docker image")?;

        // we just make sure the stream is finished, and don't process the results
        // (yet?)

        // prune images if enabled
        if CONFIG.prune_images {
            info!("pruning unused images on server to free space");
            let result = self
                .docker
                .prune_images(None::<PruneImagesOptions<String>>)
                .await
                .context("failed to prune unused images")?;

            if let Some(amount) = result.space_reclaimed {
                info!("reclaimed about {:.3} GB by pruning old images", amount as f32 / 10e9)
            }
        }

        Ok(())
    }

    /// cleans the container for a given package
    pub async fn clean_build_container(&self, package: &Package) -> anyhow::Result<()> {
        if let Some(container) = self.find_container(&container_name(package)).await? {
            self.clean(&container).await?
        }

        Ok(())
    }

    /// cleans the container, i.e. removes it
    pub async fn clean(&self, container: &ContainerId) -> anyhow::Result<()> {
        self.docker
            .remove_container(&container, None)
            .await
            .context("failed to remove container whilst cleaning")
    }
}

/// constructs the container name from package and configuration
fn container_name(package: &Package) -> String {
    format!("{}{}", CONFIG.container_prefix, &package.base)
}

/// get the docker image name that should be used
fn target_docker_image() -> String {
    CONFIG.runner_image.replace("{version}", &INFO.version)
}

/// creates the repository string which adds itself as a repository
pub fn repository_file() -> String {
    if let Some(s) = &CONFIG.own_repository_url {
        format!("[{}]\nSigLevel = Never\nServer = {}", &CONFIG.repository_name, s)
    } else {
        "".to_string()
    }
}
