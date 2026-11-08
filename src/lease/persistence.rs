use super::{Authority, WriteMode};
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

static ACTIVE: Mutex<Weak<Authority>> = Mutex::new(Weak::new());

pub struct PersistenceSession {
    authority: Arc<Authority>,
}

impl PersistenceSession {
    pub fn open(authority: Arc<Authority>) -> io::Result<Self> {
        authority.verify().map_err(io::Error::other)?;
        let mut active = ACTIVE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if active.upgrade().is_some() {
            return Err(io::Error::other(
                "persistence authority is already registered",
            ));
        }
        *active = Arc::downgrade(&authority);
        Ok(Self { authority })
    }
}

impl Drop for PersistenceSession {
    fn drop(&mut self) {
        let mut active = ACTIVE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if active.ptr_eq(&Arc::downgrade(&self.authority)) {
            *active = Weak::new();
        }
    }
}

fn authority() -> io::Result<Option<Arc<Authority>>> {
    let authority = ACTIVE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .upgrade();
    if authority.is_none()
        && (std::env::var_os("FILEBLADE_NATIVE_STATE_ROOT").is_some()
            || crate::plugin_catalog::installed_native_selected().map_err(io::Error::other)?)
    {
        return Err(io::Error::other(
            "owner-unavailable: native persistence authority is not registered",
        ));
    }
    Ok(authority)
}

pub fn storage_anchor(path: &Path) -> io::Result<Option<(File, PathBuf)>> {
    match authority()? {
        Some(authority) => authority.storage_anchor(path),
        None => Ok(None),
    }
}

pub fn check_write(path: &Path) -> io::Result<()> {
    if let Some(authority) = authority()? {
        authority.persistence_anchor(path)?;
    }
    Ok(())
}

pub fn write_mode() -> io::Result<WriteMode> {
    Ok(authority()?.map_or(WriteMode::Full, |authority| authority.write_mode()))
}

pub fn native() -> io::Result<bool> {
    Ok(authority()?.is_some())
}

pub fn verify() -> io::Result<()> {
    if let Some(authority) = authority()? {
        authority.verify().map_err(io::Error::other)?;
    }
    Ok(())
}

pub fn record_parent(path: &Path) -> io::Result<crate::secure::ResolvedParent> {
    check_write(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("record has no parent"))?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("record has no name"))?;
    let directory = match storage_anchor(parent)? {
        Some((directory, relative)) => {
            let mut directory: rustix::fd::OwnedFd = directory.into();
            for component in relative.components() {
                let std::path::Component::Normal(name) = component else {
                    return Err(io::Error::other("invalid record path"));
                };
                match rustix::fs::mkdirat(&directory, name, rustix::fs::Mode::from_raw_mode(0o700))
                {
                    Ok(()) => rustix::fs::fsync(&directory)?,
                    Err(rustix::io::Errno::EXIST) => {}
                    Err(error) => return Err(error.into()),
                }
                directory = rustix::fs::openat(
                    &directory,
                    name,
                    rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::DIRECTORY
                        | rustix::fs::OFlags::CLOEXEC
                        | rustix::fs::OFlags::NOFOLLOW,
                    rustix::fs::Mode::empty(),
                )?;
                let stat = rustix::fs::fstat(&directory)?;
                if stat.st_uid != unsafe { libc::geteuid() } {
                    return Err(io::Error::other(
                        "record directory is owned by another user",
                    ));
                }
                if crate::secure::ensure_mode(&directory, 0o700)? {
                    rustix::fs::fsync(&directory)?;
                }
            }
            directory
        }
        None if native()? => return Err(io::Error::other("unbound native persistence path")),
        None => crate::secure::ensure_private_directory(parent)?,
    };
    crate::secure::resolved_child(&directory, parent, name)
}
