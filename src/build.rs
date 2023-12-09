use std::collections::HashMap;
use std::error::{Error, };
use std::fmt::format;
use std::path::PathBuf;
use bollard::container::{Config, CreateContainerOptions};
use bollard::Docker;
use bollard::models::{Mount, MountTypeEnum};
use bollard::secret::HostConfig;

const RUNNER_IMAGE: &str = "serene-aur-runner:latest";

struct BuildStatus {
    success: bool,
    logs: String,
    file: Option<PathBuf>
}

async fn build(docker: &Docker, repository: &str, id: &str, clean: bool) -> Result<BuildStatus, Box<dyn Error>> {
    todo!()
}

pub async fn create(docker: &Docker, repository: &str, id: &str) -> Result<String, Box<dyn Error>> {

    let repository_env = format!("REPOSITORY={}", repository);
    let id_env = format!("ID={}", id);

    let env = vec![repository_env.as_str(), id_env.as_str()];

    let volume = Mount {
        target: Some("/app".to_string()),
        source: Some(std::env::var("VOLUME_HOST")?),
        typ: Some(MountTypeEnum::BIND),
        ..Default::default()
    };

    let host_config = HostConfig {
        mounts: Some(vec![volume]),
        ..Default::default()
    };

    let config = Config {
        image: Some("serene-aur-runner:latest"),
        env: Some(env),
        host_config: Some(host_config),
        ..Default::default()
    };

    let options = CreateContainerOptions {
        name: format!("serene-aur-runner-{id}"),
        ..Default::default()
    };

    Ok(docker.create_container(Some(options), config).await?.id)
}