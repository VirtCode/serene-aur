use std::path::Path;
use std::str::FromStr;
use anyhow::{anyhow, Context};
use async_std::path::PathBuf;
use async_tar::{Archive, Builder, Entries, Header};
use futures_util::{AsyncRead, AsyncReadExt, StreamExt};
use hyper::Body;
use crate::package::source::SrcinfoWrapper;

// TODO: Refactor this into some kind of ArchiveWrapper struct which may easily be passed around.
//       However, I cannot store an Archive<impl AsyncRead + Unpin> in a struct.
//       For now we will have to deal with passing that archive into and out of functions.
//       If you know how to do this properly, please let me know!

const RUNNER_IMAGE_BUILD_ARCHIVE_SRCINFO: &str = "target/.SRCINFO";
const RUNNER_IMAGE_BUILD_ARCHIVE_PACKAGE_DIR: &str = "target/";

pub fn begin_read(archive: Archive<impl AsyncRead + Unpin>) -> anyhow::Result<Entries<impl AsyncRead + Unpin + Sized>> {
    archive.entries().context("failed starting to read archive with build packages")
}

pub async fn read_srcinfo(entries: &mut Entries<impl AsyncRead + Unpin + Sized>) -> anyhow::Result<SrcinfoWrapper> {

    // we assume here that due to the filename of .SRCINFO, we will read this file first
    // however, we have read it just from VERSION for a long time, and it did still work - WTF?
    while let Some(Ok(mut entry)) = entries.next().await {
        if entry.path()?.to_string_lossy() == RUNNER_IMAGE_BUILD_ARCHIVE_SRCINFO {
            let mut srcinfo = String::new();
            entry.read_to_string(&mut srcinfo).await
                .context("could not read .SRCINFO file from archive from container")?;

            return SrcinfoWrapper::from_str(&srcinfo).context("failed to parse srcinfo returned from build container");
        }
    }

    Err(anyhow!("could not find .SRCINFO file in archive from container"))
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

pub fn begin_write() -> Builder<Vec<u8>> {
    let buffer = vec![];
    Builder::new(buffer)
}

pub async fn write_file(text: String, path: &str, writeable: bool, archive: &mut Builder<Vec<u8>>) -> anyhow::Result<()> {
    let data = text.as_bytes();

    let mut header = Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_cksum();
    header.set_mode(if writeable { 0o666 } else { 0o444 });

    archive.append_data(&mut header, path, data).await
        .context("failed to create file in archive")
}

pub async fn end_write(archive: Builder<Vec<u8>>) -> anyhow::Result<Body> {
    Ok(Body::from(archive.into_inner().await?))
}