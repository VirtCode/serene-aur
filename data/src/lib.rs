use serde::{Deserialize, Serialize};

pub mod build;
pub mod package;


#[derive(Serialize, Deserialize)]
pub struct SereneInfo {
}