use serde::{Deserialize, Serialize};

pub mod build;
pub mod package;
pub mod secret;

#[derive(Serialize, Deserialize)]
pub struct SereneInfo {
    version: String,
    repo_name: String
}