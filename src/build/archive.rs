use std::path::Path;
use actix_web::dev::ResourcePath;
use anyhow::{anyhow, Context};
use async_std::path::PathBuf;
use async_tar::{Archive, Entries};
use futures_util::{AsyncRead, AsyncReadExt, StreamExt};

// TODO: Refactor this into some kind of ArchiveWrapper struct which may easily be passed around.
//       However, I cannot store an Archive<impl AsyncRead + Unpin> in a struct.
//       For now we will have to deal with passing that archive into and out of functions.
//       If you know how to do this properly, please let me know!

const RUNNER_IMAGE_BUILD_ARCHIVE_VERSION: &str = "serene-build/.VERSION";
const RUNNER_IMAGE_BUILD_ARCHIVE_PACKAGE_DIR: &str = "serene-build/";

pub fn begin_read(archive: Archive<impl AsyncRead + Unpin>) -> anyhow::Result<Entries<impl AsyncRead + Unpin + Sized>> {
    archive.entries().context("failed starting to read archive with build packages")
}

pub async fn read_version(entries: &mut Entries<impl AsyncRead + Unpin + Sized>) -> anyhow::Result<String> {

    // we assume here that due to the filename of .VERSION, we will read this file first
    while let Some(Ok(mut entry)) = entries.next().await {
        if entry.path()?.to_string_lossy() == RUNNER_IMAGE_BUILD_ARCHIVE_VERSION {
            let mut version = String::new();
            entry.read_to_string(&mut version).await
                .context("could not read .VERSION file from archive from container")?;

            return Ok(version);
        }
    }

    Err(anyhow!("could not find .VERSION file in archive from container"))
}

pub async fn extract_files(entries: &mut Entries<impl AsyncRead + Unpin + Sized>, which: &Vec<String>, to: &Path) -> anyhow::Result<()> {
    let tar_dir = PathBuf::from(RUNNER_IMAGE_BUILD_ARCHIVE_PACKAGE_DIR);

    let mut paths: Vec<PathBuf> = which.into_iter().map(|s| tar_dir.join(s)).collect();

    while let Some(Ok(mut entry)) = entries.next().await {
        let path = entry.path()?.to_path_buf();

        if paths.iter().any(|p| p == &path) {
            entry.unpack(to.join(path.file_name().expect("file must have name"))).await
                .context("failed to extract package form archive")?;

            paths.retain(|p| p != &path);
        }
    }

    if !paths.is_empty() {
        Err(anyhow!("could not find all expected built packages: {paths:?}"))
    } else {
        Ok(())
    }
}