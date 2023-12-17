use std::error::Error;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use anyhow::{anyhow, Context};
use async_tar::Archive;
use bollard::container::{Config, CreateContainerOptions, DownloadFromContainerOptions, ListContainersOptions, LogsOptions, StartContainerOptions, UploadToContainerOptions, WaitContainerOptions};
use bollard::Docker;
use bollard::models::{Mount, MountTypeEnum};
use bollard::secret::{ContainerWaitResponse};
use bollard::secret::HostConfig;
use futures_util::{AsyncRead, AsyncReadExt, Stream, StreamExt};
use futures_util::stream::Map;
use hyper::Body;
use hyper::body::HttpBody;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::task::JoinHandle;
use tokio_util::bytes::Bytes;
use tokio_util::io::StreamReader;
use tokio_util::compat::{Compat, FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt};
use crate::package::Package;

const RUNNER_IMAGE: &str = "serene-aur-runner:latest";
const RUNNER_IMAGE_BUILD_IN: &str = "/app/build";
const RUNNER_IMAGE_BUILD_OUT: &str = "/app/build/serene-build";

#[derive(Debug)]
pub struct BuildStatus {
    pub success: bool,
    pub fatal: bool,
    pub logs: String,

    started: SystemTime,
    duration: Duration,
}

pub type ContainerId = String;

pub struct Builder {
    pub docker: Docker,
}

impl Builder {

    pub async fn build(&self, container: ContainerId) -> anyhow::Result<BuildStatus> {
        let start = SystemTime::now();

        // start container
        self.docker.start_container(&container, None::<StartContainerOptions<String>>).await?;

        // wait for container to exit and collect logs
        let result =
            self.docker.wait_container(&container,  None::<WaitContainerOptions<String>>).collect::<Vec<_>>().await;

        let end = SystemTime::now();

        // retrieve logs
        let log_options = LogsOptions {
            stdout: true, stderr: true,
            since: start.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64,
            ..Default::default()
        };

        let logs: Vec<String> = self.docker.logs::<String>(&container, Some(log_options)).filter_map(|r| async {
            r.ok().map(|c| c.to_string())
        }).collect::<Vec<_>>().await;

        Ok(BuildStatus {
            success: result.first().and_then(|r| r.as_ref().ok()).is_some(),
            fatal: false,
            logs: logs.join(""),
            started: start,
            duration: end.duration_since(start).expect("should work")
        })
    }

    pub async fn test(&self, container: &ContainerId) -> anyhow::Result<()> {
        self.docker.start_container(container, None::<StartContainerOptions<String>>).await?;

        // wait for container to exit and collect logs
        self.docker.wait_container(container,  None::<WaitContainerOptions<String>>).next().await;

        Ok(())
    }

    /// downloads the built directory from the container, in a stream.
    /// the files are in a tar archive, all in the `serene-build` folder. See RUNNER_IMAGE_BUILD_OUT
    pub async fn download_packages(&self, container: &ContainerId) -> anyhow::Result<Archive<impl AsyncRead>> {
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

        let sources = package.sources_tar().await
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
    async fn find_container(&self, package: &Package) -> anyhow::Result<Option<String>> {
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
            image: Some(RUNNER_IMAGE),
            ..Default::default()
        };

        let options = CreateContainerOptions {
            name: container_name(&package),
            ..Default::default()
        };

        Ok(self.docker.create_container(Some(options), config).await?.id)
    }
}

/*


 */

fn container_name(package: &Package) -> String{
    // TODO: Make configurable
    format!("serene-aur-runner-{}", &package.base)
}