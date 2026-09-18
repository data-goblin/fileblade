use crate::secure::FileVersion;
use crate::{AppError, AppResult};
use base64::{Engine, prelude::BASE64_STANDARD};
use rustix::fs::{Mode, OFlags};
use std::io;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const MAX_CONTENT: usize = 8 * 1024 * 1024;

pub struct Snapshot {
    pub logical: PathBuf,
    pub resolved: PathBuf,
    pub data: Option<Vec<u8>>,
    pub version: Option<FileVersion>,
}

fn failed(message: &str) -> io::Error {
    io::Error::other(message)
}

fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .join(path)
    }
}

impl Snapshot {
    pub fn read(path: &Path, limit: usize) -> io::Result<Self> {
        if limit > MAX_CONTENT {
            return Err(failed("configuration read limit is invalid"));
        }
        let logical = absolute(path);
        let resolved = super::path::realpath(&logical);
        let descriptor = match rustix::fs::open(
            &resolved,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        ) {
            Ok(descriptor) => descriptor,
            Err(rustix::io::Errno::NOENT) => {
                return Ok(Self {
                    logical,
                    resolved,
                    data: None,
                    version: None,
                });
            }
            Err(error) => return Err(io::Error::from(error)),
        };
        let before = rustix::fs::fstat(&descriptor)?;
        if before.st_mode & libc::S_IFMT != libc::S_IFREG || before.st_size as usize > limit {
            return Err(failed("configuration is not a bounded regular file"));
        }
        let mut data: Vec<u8> = Vec::new();
        let mut file = std::fs::File::from(descriptor);
        Read::by_ref(&mut file)
            .take(limit as u64 + 1)
            .read_to_end(&mut data)?;
        let after = rustix::fs::fstat(&file)?;
        let unchanged = (
            before.st_size,
            before.st_mtime,
            before.st_mtime_nsec,
            before.st_ctime,
            before.st_ctime_nsec,
        ) == (
            after.st_size,
            after.st_mtime,
            after.st_mtime_nsec,
            after.st_ctime,
            after.st_ctime_nsec,
        );
        if data.len() > limit
            || !unchanged
            || std::fs::canonicalize(&logical).ok().as_deref() != Some(resolved.as_path())
        {
            return Err(failed("configuration changed while it was read"));
        }
        let version = FileVersion {
            dev: before.st_dev,
            ino: before.st_ino,
            data: BASE64_STANDARD.encode(&data),
        };
        Ok(Self {
            logical,
            resolved,
            data: Some(data),
            version: Some(version),
        })
    }

    pub fn write(&self, data: &[u8]) -> AppResult<()> {
        if data.len() > MAX_CONTENT {
            return Err(AppError::invalid(
                "updated configuration exceeds its byte limit",
            ));
        }
        crate::companion_mutations::write(
            &self.logical,
            &self.resolved,
            self.version.as_ref(),
            data,
        )
    }
}
