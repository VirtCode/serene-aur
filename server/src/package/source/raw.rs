use crate::package::source::{SourceImpl, PKGBUILD};
use crate::package::srcinfo::SrcinfoWrapper;
use crate::runner::archive::InputArchive;
use serene_data::secret;
use std::path::Path;

/// this is a source which is based on a raw pkgbuild
pub struct RawSource {
    pkgbuild: String,
}

impl RawSource {
    pub fn new(pkgbuild: &str) -> Self {
        Self { pkgbuild: pkgbuild.to_owned() }
    }
}

impl SourceImpl for RawSource {
    async fn initialize(&mut self, _folder: &Path) -> anyhow::Result<()> {
        Ok(())
    }

    fn get_url(&self) -> Option<String> {
        None
    }

    fn get_type(&self) -> String {
        "raw pkgbuild".to_string()
    }

    fn get_state(&self) -> String {
        // yes this is technically for secrets
        secret::hash(&self.pkgbuild)
    }

    async fn update(&self, _folder: &Path) -> anyhow::Result<()> {
        Ok(())
    }

    async fn get_pkgbuild(&self, _folder: &Path) -> anyhow::Result<String> {
        Ok(self.pkgbuild.clone())
    }

    async fn get_srcinfo(&self, folder: &Path) -> anyhow::Result<Option<SrcinfoWrapper>> {
        Ok(None)
    }

    async fn load_build_files(
        &self,
        archive: &mut InputArchive,
        _folder: &Path,
    ) -> anyhow::Result<()> {
        archive.write_file(&self.pkgbuild, Path::new(PKGBUILD), true)
    }
}
