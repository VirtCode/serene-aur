use crate::package::srcinfo::SrcinfoWrapper;
use crate::runner::stats::CgroupStats;
use anyhow::{Context, anyhow};
use async_std::path::PathBuf;
use async_tar::{Archive, Builder, Entries, Header};
use futures_util::{AsyncRead, AsyncReadExt, StreamExt};
use hyper::Body;
use std::path::Path;
use std::str::FromStr;

const RUNNER_IMAGE_BUILD_ARCHIVE_SRCINFO: &str = "target/.SRCINFO";
const RUNNER_IMAGE_BUILD_STATS_BEFORE: &str = "target/.stats-before.json";
const RUNNER_IMAGE_BUILD_STATS_AFTER: &str = "target/.stats-after.json";
const RUNNER_IMAGE_BUILD_ARCHIVE_PACKAGE_DIR: &str = "target/";

/// this is an archive which can only be read from and is based on a stream
pub struct OutputArchive<R>
where
    R: AsyncRead + Unpin,
{
    entries: Entries<R>,
}

/// Order in which files are read by docker engine:
/// - https://github.com/moby/moby/blob/1b48d9602cbea7e18e570c5674644191e0275fa7/daemon/archive_unix.go#L75
/// - https://github.com/moby/go-archive/blob/263611f5f0914b2a153d86dae2042d13be6a88c4/archive.go#L693
/// - https://pkg.go.dev/path/filepath#WalkDir
///
/// Thus the order in which "things" need to be extracted from the archive
/// 1. .SRCINFO file
/// 2. .stats-before.json and .stats-after.json files
/// 3. any other file
impl<R: AsyncRead + Unpin> OutputArchive<R> {
    pub fn new(input: R) -> anyhow::Result<Self> {
        let archive = Archive::new(input);

        Ok(Self { entries: archive.entries().context("failed to start reading archive")? })
    }

    /// read the srcinfo from the archive, this should be called before
    /// [`OutputArchive::build_stats`] and before extracting files
    pub async fn srcinfo(&mut self) -> anyhow::Result<SrcinfoWrapper> {
        while let Some(Ok(mut entry)) = self.entries.next().await {
            if entry.path()?.to_string_lossy() == RUNNER_IMAGE_BUILD_ARCHIVE_SRCINFO {
                let mut srcinfo = String::new();
                entry
                    .read_to_string(&mut srcinfo)
                    .await
                    .context("could not read .SRCINFO file from archive from container")?;

                return SrcinfoWrapper::from_str(&srcinfo)
                    .context("failed to parse srcinfo returned from build container");
            }
        }

        Err(anyhow!("could not find .SRCINFO file in archive from container"))
    }

    /// read the build stats files from the archive, this should be called
    /// after [`OutputArchive::srcinfo`] and before extracting files
    pub async fn build_stats(&mut self) -> anyhow::Result<(CgroupStats, CgroupStats)> {
        let mut stats_before = None;
        let mut stats_after = None;

        let mut buffer = String::new();

        while let Some(Ok(mut entry)) = self.entries.next().await {
            let path = entry.path()?;
            match path.to_string_lossy().as_str() {
                RUNNER_IMAGE_BUILD_STATS_BEFORE => {
                    entry.read_to_string(&mut buffer).await.context(
                        "could not read .stats-before.json file from archive from container",
                    )?;
                    stats_before = Some(
                        serde_json::from_str::<CgroupStats>(&buffer)
                            .context("could not parse .stats-before.json file")?,
                    );
                    log::debug!("parsed before-build stats from archive from container");
                    buffer.clear();
                }
                RUNNER_IMAGE_BUILD_STATS_AFTER => {
                    entry.read_to_string(&mut buffer).await.context(
                        "could not read .stats-after.json file from archive from container",
                    )?;
                    stats_after = Some(
                        serde_json::from_str::<CgroupStats>(&buffer)
                            .context("could not parse .stats-after.json file")?,
                    );
                    log::debug!("parsed after-build stats from archive from container");
                    buffer.clear();
                }
                _ => {}
            }

            if stats_before.is_some() && stats_after.is_some() {
                // we need to first check with `is_some` because using `let Some(...)` we would
                // move the value in every iteration
                #[allow(clippy::unnecessary_unwrap)]
                return Ok((stats_before.expect("is checked"), stats_after.expect("is checked")));
            }
        }

        Err(anyhow!("could not find all stats files in archive from container"))
    }

    /// extract list of files to the given location
    pub async fn extract(&mut self, files: &[String], to: &Path) -> anyhow::Result<()> {
        let tar_dir = PathBuf::from(RUNNER_IMAGE_BUILD_ARCHIVE_PACKAGE_DIR);

        let mut paths: Vec<PathBuf> = files.iter().map(|s| tar_dir.join(s)).collect();

        while let Some(Ok(mut entry)) = self.entries.next().await {
            let path = entry.path()?.to_path_buf();

            if paths.iter().any(|p| p == &path) {
                entry
                    .unpack(to.join(path.file_name().expect("file must have name")))
                    .await
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
}

/// this is an archive you can write to, note however that its content are
/// directly stored in memory (when calling finish)
pub struct InputArchive {
    builder: Builder<Vec<u8>>,
}

impl InputArchive {
    pub fn new() -> Self {
        let buffer = vec![];
        Self { builder: Builder::new(buffer) }
    }

    /// write a string to a file in the archive
    pub async fn write_file(
        &mut self,
        text: &str,
        path: &Path,
        writeable: bool,
    ) -> anyhow::Result<()> {
        let data = text.as_bytes();

        let mut header = Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_cksum();
        header.set_mode(if writeable { 0o666 } else { 0o444 });

        self.builder
            .append_data(&mut header, path, data)
            .await
            .context("failed to create file in archive")
    }

    /// append a directory on the filesystem to the archive
    pub async fn append_directory(&mut self, src: &Path, dst: &Path) -> anyhow::Result<()> {
        self.builder
            .append_dir_all(dst, src)
            .await
            .context("failed to append directory to input archive")
    }

    /// append a file on the filesystem to the archive
    pub async fn append_file(&mut self, src: &Path, dst: &Path) -> anyhow::Result<()> {
        self.builder
            .append_path_with_name(src, dst)
            .await
            .context("failed to append file to input archive")
    }

    pub async fn finish(self) -> anyhow::Result<Body> {
        // this internally finishes the archive
        Ok(Body::from(self.builder.into_inner().await?))
    }
}
