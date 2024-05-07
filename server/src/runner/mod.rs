pub mod archive;

use std::error::Error;
use std::io::Read;
use anyhow::{Context};
use async_tar::Archive;
use bollard::container::{Config, CreateContainerOptions, DownloadFromContainerOptions, ListContainersOptions, LogsOptions, StartContainerOptions, UploadToContainerOptions, WaitContainerOptions};
use bollard::{API_DEFAULT_VERSION, Docker};
use bollard::image::CreateImageOptions;
use chrono::{DateTime, Utc};
use futures_util::{AsyncRead, StreamExt, TryStreamExt};
use hyper::body::HttpBody;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use tokio_util::io::StreamReader;
use tokio_util::compat::{TokioAsyncReadCompatExt};
use crate::config::CONFIG;
use crate::package::Package;

const RUNNER_IMAGE_BUILD_IN: &str = "/app/build";
const RUNNER_IMAGE_BUILD_OUT: &str = "/app/target";

/// this is the status of a build run through the runner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStatus {
    pub success: bool,
    pub logs: String,

    pub started: DateTime<Utc>,
    pub ended: DateTime<Utc>,
}

pub type ContainerId = String;

/// this is a wrapper for docker which creates and interacts with runner containers
pub struct Runner {
    pub docker: Docker,
}

impl Runner {

    /// creates a new runner by taking the docker from the default socket
    pub fn new() -> anyhow::Result<Self> {
        let docker = if let Some(url) = &CONFIG.docker_url {

            if url.starts_with("tcp://") || url.starts_with("http://") {

                info!("using docker via tcp at '{url}'");
                Docker::connect_with_http(url, 120, API_DEFAULT_VERSION)

            } else {
                if !url.starts_with("unix://") { warn!("missing docker url scheme, assuming path to unix socket"); }

                info!("using docker via unix socket at '{url}'");
                Docker::connect_with_unix(url, 120, API_DEFAULT_VERSION)
            }
        } else {
            info!("using docker via the default unix socket");
            Docker::connect_with_unix_defaults()
        };

        Ok(Self {
            docker: docker.context("failed to initialize docker")?
        })
    }

    /// builds the package inside a container
    pub async fn build(&self, container: &ContainerId) -> anyhow::Result<RunStatus> {
        let start = Utc::now();

        // start container
        self.docker.start_container(container, None::<StartContainerOptions<String>>).await?;

        // wait for container to exit and collect logs
        let result =
            self.docker.wait_container(container,  None::<WaitContainerOptions<String>>).collect::<Vec<_>>().await;

        let end = Utc::now();

        // retrieve logs
        let log_options = LogsOptions {
            stdout: true, stderr: true,
            since: start.timestamp(),
            ..Default::default()
        };

        let logs: Vec<String> = self.docker.logs::<String>(container, Some(log_options)).filter_map(|r| async {
            r.ok().map(|c| c.to_string())
        }).collect::<Vec<_>>().await;

        Ok(RunStatus {
            success: result.first().and_then(|r| r.as_ref().ok()).is_some(),
            logs: logs.join(""),

            started: start,
            ended: end,
        })
    }

    /// downloads the built directory from the container, in a stream.
    /// the files are in a tar archive, all in the `serene-build` folder. See RUNNER_IMAGE_BUILD_OUT
    pub async fn download_packages(&self, container: &ContainerId) -> anyhow::Result<Archive<impl AsyncRead + Unpin>> {
        let options = DownloadFromContainerOptions {
            path: RUNNER_IMAGE_BUILD_OUT,
        };

        let stream = self.docker.download_from_container(container, Some(options))
            .map(|b| b.map_err(std::io::Error::other));
        let reader = StreamReader::new(stream);

        let archive = Archive::new(reader.compat());

        Ok(archive)
    }

    /// uploads files to the build directory in a container
    /// the files should be in a tar archive, in a body, where everything is in the root
    pub async fn upload_sources(&self, container: &ContainerId, package: &Package) -> anyhow::Result<()> {

        let sources = package.build_files().await
            .context("could not get sources tar from package")?;

        let options = UploadToContainerOptions{
            path: RUNNER_IMAGE_BUILD_IN,
            no_overwrite_dir_non_dir: "false"
        };

        self.docker.upload_to_container(container, Some(options), sources).await
            .context("could not upload sources to docker container")?;

        Ok(())
    }

    /// prepares a container for the build process
    /// either creates a new one or takes an old one which was already created
    pub async fn prepare(&self, package: &Package) -> anyhow::Result<ContainerId> {
        Ok(if let Some(id) = self.find_container(package).await? {
            id
        } else {
            self.create_container(package).await?
        })
    }

    /// finds an already created container for a package
    pub async fn find_container(&self, package: &Package) -> anyhow::Result<Option<ContainerId>> {
        let summary = self.docker.list_containers::<String>(Some(ListContainersOptions {
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

    /// creates a new build container for a package
    async fn create_container(&self, package: &Package) -> anyhow::Result<String> {
        let config = Config {
            image: Some(CONFIG.runner_image.clone()),
            ..Default::default()
        };

        let options = CreateContainerOptions {
            name: container_name(&package),
            ..Default::default()
        };

        Ok(self.docker.create_container(Some(options), config).await?.id)
    }

    pub async fn update_image(&self) -> anyhow::Result<()> {
        let results = self.docker.create_image(Some(CreateImageOptions {
            from_image: CONFIG.runner_image.clone(),
            ..Default::default()
        }), None, None)
            .collect::<Vec<Result<_, _>>>().await;

        // can this be directly collected into a result? probably... but streams suck
        let _statuses = results.into_iter().collect::<Result<Vec<_>, _>>()
            .context("failed to pull new docker image")?;
        
        // we just make sure the stream is finished, and don't process the results (yet?)
        
        Ok(())
    }

    /// cleans the container, i.e. removes it
    pub async fn clean(&self, container: &ContainerId) -> anyhow::Result<()> {
        self.docker.remove_container(&container, None).await
            .context("failed to remove container whilst cleaning")
    }
}

/// constructs the container name from package and configuration
fn container_name(package: &Package) -> String{
    format!("{}{}", CONFIG.container_prefix, &package.base)
}

/// creates the repository string which adds itself as a repository
pub fn repository_file() -> String {
    if let Some(s) = &CONFIG.own_repository_url {
        format!("[{}]\nSigLevel = Never\nServer = {}", &CONFIG.repository_name, s)
    } else { "".to_string() }
}