use serde::{Deserialize, Serialize};
use serene_data::build::BuildInfo;
use serene_data::package::{PackageInfo, PackagePeek};
use crate::build::BuildSummary;
use crate::database::Database;
use crate::package::Package;



impl Package {
    pub async fn to_peek(&self, db: &Database) -> anyhow::Result<PackagePeek> {
        Ok(PackagePeek {
            base: self.base.clone(),
            enabled: self.enabled,
            devel: self.source.is_devel(),
            version: self.version.clone().unwrap_or("unknown".to_string()), // TODO: fix
            build: BuildSummary::find_latest_for_package(&self, &db).await?
                .as_ref().map(BuildSummary::as_info)
        })
    }

    pub async fn to_info(&self, db: &Database) -> anyhow::Result<PackageInfo> {
        Ok(PackageInfo {
            base: self.base.clone(),
            members: self.get_packages(),
            version: self.version.clone().unwrap_or("unknown".to_string()),
            devel: self.source.is_devel(),
            enabled: self.enabled,
            clean: self.clean,
            schedule: self.get_schedule(),
            added: self.added,
            builds: BuildSummary::find_all_for_package(&self, &db).await?.iter()
                .map(BuildSummary::as_info)
                .collect(),
            prepare_commands: self.prepare.clone()
        })
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

