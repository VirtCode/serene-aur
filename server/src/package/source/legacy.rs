use serene_data::secret;
use std::collections::HashMap;

pub enum LegacySource {
    SereneCliSource {
        last_commit: String,
    },
    DevelGitSource {
        repository: String,
        last_commit: String,
        last_source_commits: HashMap<String, String>,
    },
    NormalSource {
        repository: String,
        last_commit: String,
    },
    SingleSource {
        pkgbuild: String,
        srcinfo: String,

        devel: bool,
        last_source_commits: HashMap<String, String>,
    },
}

impl LegacySource {
    pub fn get_state(&self) -> String {
        match self {
            LegacySource::SereneCliSource { last_commit } => last_commit.clone(),
            LegacySource::DevelGitSource { last_commit, last_source_commits, .. } => {
                let mut string = last_commit.clone();

                for commit in last_source_commits.values() {
                    string.push_str(commit);
                }

                string
            }
            LegacySource::NormalSource { last_commit, .. } => last_commit.clone(),
            LegacySource::SingleSource { pkgbuild, devel, last_source_commits, .. } => {
                // yes, this is technically for secrets
                let mut string = secret::hash(&pkgbuild);

                if *devel {
                    for commit in last_source_commits.values() {
                        string.push_str(commit);
                    }
                }

                string
            }
        }
    }
}
