use super::*;

impl Drop for LockedFile {
    fn drop(&mut self) {
        let _ = flock(&self.file, FlockOperation::Unlock);
    }
}

impl LockedFile {
    pub fn file(&self) -> &File {
        &self.file
    }
}

pub fn ensure_private_directory(path: &Path) -> io::Result<OwnedFd> {
    let normalized = normalized_absolute(path)?;
    crate::lease::persistence::check_write(&normalized)?;
    let (mut current, relative) = directory_anchor(&normalized)?;
    for component in relative.components() {
        let Component::Normal(name) = component else {
            continue;
        };
        let next = match openat(
            &current,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        ) {
            Ok(fd) => fd,
            Err(rustix::io::Errno::NOENT) => {
                match mkdirat(&current, name, Mode::from_raw_mode(PRIVATE_DIRECTORY_MODE)) {
                    Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                    Err(error) => return Err(error.into()),
                }
                fsync(&current).map_err(io::Error::from)?;
                openat(
                    &current,
                    name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                    Mode::empty(),
                )
                .map_err(io::Error::from)?
            }
            Err(error) => return Err(error.into()),
        };
        current = next;
    }
    verify_owner_and_kind(&current, EntryKind::Directory)?;
    if rustix::fs::fstat(&current)
        .map_err(io::Error::from)?
        .st_mode
        & 0o7777
        != PRIVATE_DIRECTORY_MODE
    {
        fchmod(&current, Mode::from_raw_mode(PRIVATE_DIRECTORY_MODE)).map_err(io::Error::from)?;
        fsync(&current).map_err(io::Error::from)?;
    }
    verify_private_fd(&current, EntryKind::Directory, PRIVATE_DIRECTORY_MODE)?;
    Ok(current)
}

pub fn ensure_directories(path: &Path, mode: u32) -> io::Result<OwnedFd> {
    let normalized = normalized_absolute(path)?;
    crate::lease::persistence::check_write(&normalized)?;
    let (mut current, relative) = directory_anchor(&normalized)?;
    for component in relative.components() {
        let Component::Normal(name) = component else {
            continue;
        };
        let next = match openat(
            &current,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        ) {
            Ok(fd) => fd,
            Err(rustix::io::Errno::NOENT) => {
                match mkdirat(&current, name, Mode::from_raw_mode(mode)) {
                    Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                    Err(error) => return Err(error.into()),
                }
                fsync(&current).map_err(io::Error::from)?;
                openat(
                    &current,
                    name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                    Mode::empty(),
                )
                .map_err(io::Error::from)?
            }
            Err(error) => return Err(error.into()),
        };
        current = next;
    }
    Ok(current)
}

pub fn verify_private_directory(path: &Path) -> io::Result<()> {
    let fd = open_absolute_directory(path, true)?;
    verify_private_fd(&fd, EntryKind::Directory, PRIVATE_DIRECTORY_MODE)
}

pub fn verify_private_file(path: &Path) -> io::Result<()> {
    let parent = resolved_parent_nofollow(path)?;
    let fd = openat(
        &parent.directory,
        &parent.name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    verify_private_fd(&fd, EntryKind::File, PRIVATE_FILE_MODE)
}

pub fn open_private_lock(path: &Path) -> io::Result<LockedFile> {
    private_lock(path, false)?.ok_or_else(|| io::Error::other("lock unavailable"))
}

pub fn try_open_private_lock(path: &Path) -> io::Result<Option<LockedFile>> {
    private_lock(path, true)
}

fn private_lock(path: &Path, nonblocking: bool) -> io::Result<Option<LockedFile>> {
    let parent_path = path
        .parent()
        .ok_or_else(|| invalid_input(format!("{} has no parent", path.display())))?;
    let directory = ensure_private_directory(parent_path)?;
    let name = path
        .file_name()
        .ok_or_else(|| invalid_input("lock has no file name"))?;
    let fd = openat(
        &directory,
        name,
        OFlags::RDWR | OFlags::CREATE | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::from_raw_mode(PRIVATE_FILE_MODE),
    )
    .map_err(io::Error::from)?;
    verify_owner_and_kind(&fd, EntryKind::File)?;
    fchmod(&fd, Mode::from_raw_mode(PRIVATE_FILE_MODE)).map_err(io::Error::from)?;
    verify_private_fd(&fd, EntryKind::File, PRIVATE_FILE_MODE)?;
    let operation = if nonblocking {
        FlockOperation::NonBlockingLockExclusive
    } else {
        FlockOperation::LockExclusive
    };
    match flock(&fd, operation) {
        Ok(()) => {
            crate::lease::persistence::check_write(path)?;
            Ok(Some(LockedFile { file: fd.into() }))
        }
        Err(rustix::io::Errno::WOULDBLOCK) if nonblocking => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub fn read_private_bounded(path: &Path, limit: usize) -> io::Result<Option<Vec<u8>>> {
    let Some(mut file) = open_private_read(path)? else {
        return Ok(None);
    };
    let size = file.metadata()?.len().min(limit as u64) as usize;
    let mut data = Vec::with_capacity(size);
    Read::by_ref(&mut file)
        .take(limit.saturating_add(1) as u64)
        .read_to_end(&mut data)?;
    if data.len() > limit {
        return Err(invalid_data(format!(
            "{} exceeds {limit} bytes",
            path.display()
        )));
    }
    Ok(Some(data))
}

pub fn open_private_read(path: &Path) -> io::Result<Option<File>> {
    let parent = resolved_parent_nofollow(path)?;
    let fd = match openat(
        &parent.directory,
        &parent.name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    verify_private_fd(&fd, EntryKind::File, PRIVATE_FILE_MODE)?;
    Ok(Some(fd.into()))
}

pub fn open_private_append(path: &Path) -> io::Result<File> {
    crate::lease::persistence::check_write(path)?;
    let parent = resolved_parent_nofollow(path)?;
    let fd = openat(
        &parent.directory,
        &parent.name,
        OFlags::WRONLY
            | OFlags::APPEND
            | OFlags::CREATE
            | OFlags::CLOEXEC
            | OFlags::NOFOLLOW
            | OFlags::NONBLOCK,
        Mode::from_raw_mode(PRIVATE_FILE_MODE),
    )
    .map_err(io::Error::from)?;
    verify_private_fd(&fd, EntryKind::File, PRIVATE_FILE_MODE)?;
    Ok(fd.into())
}

pub fn read_bounded_nofollow(path: &Path, limit: usize) -> io::Result<Option<Vec<u8>>> {
    let parent = resolved_parent(path)?;
    let fd = match openat(
        &parent.directory,
        &parent.name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let stat = stat_value(fstat(&fd).map_err(io::Error::from)?);
    if stat.kind != EntryKind::File {
        return Err(invalid_data(format!(
            "{} is not a regular file",
            path.display()
        )));
    }
    if stat.size > limit as u64 {
        return Err(invalid_data(format!(
            "{} exceeds {limit} bytes",
            path.display()
        )));
    }
    let mut file: File = fd.into();
    let mut data = Vec::with_capacity(stat.size as usize);
    Read::by_ref(&mut file)
        .take(limit.saturating_add(1) as u64)
        .read_to_end(&mut data)?;
    if data.len() > limit {
        return Err(invalid_data(format!(
            "{} exceeds {limit} bytes",
            path.display()
        )));
    }
    Ok(Some(data))
}

pub fn write_private_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    let parent_path = path
        .parent()
        .ok_or_else(|| invalid_input(format!("{} has no parent", path.display())))?;
    let directory = ensure_private_directory(parent_path)?;
    let final_name = path
        .file_name()
        .ok_or_else(|| invalid_input("target has no file name"))?;
    match stat_in(&directory, final_name) {
        Ok(stat) if stat.kind == EntryKind::File && stat.uid == geteuid().as_raw() => {}
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "{} is not a private file owned by this user",
                    path.display()
                ),
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let temporary = OsString::from(format!(".fileblade-{}.tmp", Uuid::new_v4().simple()));
    let fd = openat(
        &directory,
        &temporary,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::from_raw_mode(PRIVATE_FILE_MODE),
    )
    .map_err(io::Error::from)?;
    let result = (|| {
        crate::lease::persistence::check_write(path)?;
        let mut file: File = fd.into();
        file.write_all(data)?;
        file.flush()?;
        file.sync_all()?;
        crate::lease::persistence::check_write(path)?;
        renameat(&directory, &temporary, &directory, final_name).map_err(io::Error::from)?;
        fsync(&directory).map_err(io::Error::from)
    })();
    if result.is_err() {
        let _ = unlinkat(&directory, &temporary, AtFlags::empty());
    }
    result
}

pub fn write_new_private(path: &Path, data: &[u8]) -> io::Result<()> {
    crate::lease::persistence::check_write(path)?;
    let parent = resolved_parent_nofollow(path)?;
    let fd = openat(
        &parent.directory,
        &parent.name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::from_raw_mode(PRIVATE_FILE_MODE),
    )?;
    let result = (|| {
        crate::lease::persistence::check_write(path)?;
        let mut file = File::from(fd);
        file.write_all(data)?;
        file.sync_all()?;
        fsync(&parent.directory).map_err(io::Error::from)
    })();
    if result.is_err() {
        let _ = unlinkat(&parent.directory, &parent.name, AtFlags::empty());
    }
    result
}

pub(super) fn verify_private_fd(
    fd: &impl AsFd,
    kind: EntryKind,
    expected_mode: u32,
) -> io::Result<()> {
    let stat = stat_value(fstat(fd).map_err(io::Error::from)?);
    if stat.kind != kind {
        return Err(invalid_data("private state has the wrong file type"));
    }
    if stat.uid != geteuid().as_raw() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private state is owned by another user",
        ));
    }
    if stat.mode & 0o077 != 0 || stat.mode & expected_mode != expected_mode {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private state has unsafe permissions",
        ));
    }
    Ok(())
}
