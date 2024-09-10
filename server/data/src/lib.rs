use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub mod build;
pub mod package;
pub mod secret;

#[derive(Serialize, Deserialize)]
pub struct SereneInfo {
    /// version of the server
    pub version: String,
    /// start time of the server (used for calculating uptime)
    pub started: DateTime<Utc>,
    /// name of the repo
    pub name: String,
    /// architecture of the packages
    pub architecture: String,
    /// is the server readable without auth
    pub readable: bool,
    /// are the packages signed
    pub signed: bool
}
