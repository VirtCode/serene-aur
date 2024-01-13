use serde::{Deserialize, Serialize};
use serene_data::build::BuildInfo;
use serene_data::package::{PackageInfo, PackagePeek};
use crate::build::BuildSummary;
use crate::package::Package;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PackageAddRequest {
    Aur { name: String },
    Custom { url: String, devel: bool }
}

impl Package {
    pub fn as_peek(&self) -> PackagePeek {
        PackagePeek {
            base: self.base.clone(),
            enabled: self.enabled,
            devel: self.source.is_devel(),
            version: self.version.clone(),
            build: self.get_builds().iter()
                .max_by_key(|p| p.started)
                .map(BuildSummary::as_info)
        }
    }

    pub fn as_info(&self) -> PackageInfo {
        PackageInfo {
            base: self.base.clone(),
            members: self.source.get_packages(),
            version: self.version.clone(),
            devel: self.source.is_devel(),
            enabled: self.enabled,
            clean: self.clean,
            schedule: self.get_schedule(),
            added: self.added,
            builds: self.get_builds().iter()
                .map(BuildSummary::as_info)
                .collect()
        }
    }
}

impl BuildSummary {
    pub fn as_info(&self) -> BuildInfo {
        BuildInfo {
            version: self.version.clone(),
            state: self.state.clone(),
            started: self.started.clone(),
            ended: self.ended.clone()
        }
    }
}

