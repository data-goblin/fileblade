use super::*;

pub fn copy_path_noreplace(
    source: &Path,
    destination: &Path,
    cancelled: &AtomicBool,
    progress: &mut dyn FnMut(&Path),
) -> io::Result<()> {
    check_cancelled(cancelled)?;
    let source_parent = resolved_parent(source)?;
    let destination_parent = resolved_parent(destination)?;
    copy_path_noreplace_resolved(&source_parent, &destination_parent, cancelled, progress)
}

pub fn copy_path_noreplace_resolved(
    source: &ResolvedParent,
    destination: &ResolvedParent,
    cancelled: &AtomicBool,
    progress: &mut dyn FnMut(&Path),
) -> io::Result<()> {
    check_cancelled(cancelled)?;
    copy_from_parent_noreplace(
        &source.directory,
        &source.name,
        &source.full_path(),
        destination,
        cancelled,
        progress,
    )
}

pub(super) fn copy_from_parent_noreplace(
    source_parent: &OwnedFd,
    source_name: &OsStr,
    source_path: &Path,
    destination_parent: &ResolvedParent,
    cancelled: &AtomicBool,
    progress: &mut dyn FnMut(&Path),
) -> io::Result<()> {
    if stat_in(&destination_parent.directory, &destination_parent.name).is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "{} already exists",
                destination_parent
                    .path
                    .join(&destination_parent.name)
                    .display()
            ),
        ));
    }
    let partial_name = OsString::from(format!(".fileblade-partial-{}", Uuid::new_v4().simple()));
    mkdirat(
        &destination_parent.directory,
        &partial_name,
        Mode::from_raw_mode(PRIVATE_DIRECTORY_MODE),
    )
    .map_err(io::Error::from)?;
    let partial_directory = match openat(
        &destination_parent.directory,
        &partial_name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    ) {
        Ok(directory) => directory,
        Err(error) => {
            let _ = remove_entry_from(&destination_parent.directory, &partial_name);
            return Err(error.into());
        }
    };
    let partial_intent = match write_intent(&serde_json::json!({
        "kind": "partial",
        "partial": crate::common::path_text(&destination_parent.path.join(&partial_name)),
        "pid": std::process::id(),
    })) {
        Ok(intent) => intent,
        Err(error) => {
            let _ = remove_entry_from(&destination_parent.directory, &partial_name);
            return Err(error);
        }
    };
    let partial_item = OsStr::new("item");
    let result = copy_entry(
        source_parent,
        source_name,
        source_path,
        &partial_directory,
        partial_item,
        cancelled,
        progress,
    )
    .and_then(|()| rustix::fs::syncfs(&partial_directory).map_err(io::Error::from))
    .and_then(|()| {
        renameat_with(
            &partial_directory,
            partial_item,
            &destination_parent.directory,
            &destination_parent.name,
            RenameFlags::NOREPLACE,
        )
        .map_err(io::Error::from)
    })
    .and_then(|()| {
        unlinkat(
            &destination_parent.directory,
            &partial_name,
            AtFlags::REMOVEDIR,
        )
        .map_err(io::Error::from)
    })
    .and_then(|()| fsync(&destination_parent.directory).map_err(io::Error::from));
    if result.is_err() {
        let _ = remove_entry_from(&destination_parent.directory, &partial_name);
    }
    let _ = partial_intent.clear();
    result
}

pub fn fsync_path_parent(path: &Path) -> io::Result<()> {
    let parent = resolved_parent(path)?;
    fsync(&parent.directory).map_err(io::Error::from)
}

pub fn set_mode_and_times(
    path: &Path,
    mode: u32,
    atime: (i64, i64),
    mtime: (i64, i64),
) -> io::Result<()> {
    let parent = resolved_parent(path)?;
    let fd = openat(
        &parent.directory,
        &parent.name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    apply_mode_and_times(&fd, mode, atime, mtime)
}

pub fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    let parent = resolved_parent(path)?;
    let fd = openat(
        &parent.directory,
        &parent.name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    fchmod(&fd, Mode::from_raw_mode(mode)).map_err(io::Error::from)
}

pub(super) fn copy_entry(
    source_parent: &OwnedFd,
    source_name: &OsStr,
    source_path: &Path,
    destination_parent: &OwnedFd,
    destination_name: &OsStr,
    cancelled: &AtomicBool,
    progress: &mut dyn FnMut(&Path),
) -> io::Result<()> {
    check_cancelled(cancelled)?;
    let stat = stat_in(source_parent, source_name)?;
    progress(source_path);
    match stat.kind {
        EntryKind::File => copy_regular(
            source_parent,
            source_name,
            source_path,
            destination_parent,
            destination_name,
            stat,
            cancelled,
        ),
        EntryKind::Directory => copy_directory(
            (source_parent, source_name, source_path),
            (destination_parent, destination_name),
            stat,
            cancelled,
            progress,
        ),
        EntryKind::Symlink => {
            let target =
                readlinkat(source_parent, source_name, Vec::new()).map_err(io::Error::from)?;
            let target = OsStr::from_bytes(target.to_bytes());
            symlinkat(target, destination_parent, destination_name).map_err(io::Error::from)
        }
        EntryKind::Other => Err(invalid_data(format!(
            "{} is neither a file, a symlink, nor a directory",
            source_path.display()
        ))),
    }
}

pub(super) fn copy_regular(
    source_parent: &OwnedFd,
    source_name: &OsStr,
    source_path: &Path,
    destination_parent: &OwnedFd,
    destination_name: &OsStr,
    before: EntryStat,
    cancelled: &AtomicBool,
) -> io::Result<()> {
    let source_fd = openat(
        source_parent,
        source_name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    let opened = stat_value(fstat(&source_fd).map_err(io::Error::from)?);
    if opened.kind != EntryKind::File || opened.dev != before.dev || opened.ino != before.ino {
        return Err(invalid_data(format!(
            "{} changed before it was copied",
            source_path.display()
        )));
    }
    let destination_fd = openat(
        destination_parent,
        destination_name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::from_raw_mode(PRIVATE_FILE_MODE),
    )
    .map_err(io::Error::from)?;
    let mut source_file: File = source_fd.into();
    let mut destination_file: File = destination_fd.into();
    let mut buffer = [0_u8; 64 * 1024];
    let mut written = 0_u64;
    loop {
        check_cancelled(cancelled)?;
        let count = source_file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        destination_file.write_all(&buffer[..count])?;
        written = written.saturating_add(count as u64);
    }
    let after = stat_value(fstat(&source_file).map_err(io::Error::from)?);
    if written != before.size
        || after.dev != before.dev
        || after.ino != before.ino
        || after.size != before.size
        || after.mtime != before.mtime
        || after.mtime_nsec != before.mtime_nsec
    {
        return Err(invalid_data(format!(
            "{} changed while it was copied",
            source_path.display()
        )));
    }
    copy_xattrs(&source_file, &destination_file);
    apply_mode_and_times(
        &destination_file,
        before.mode,
        (before.atime, before.atime_nsec),
        (before.mtime, before.mtime_nsec),
    )
}

pub(super) fn copy_directory(
    source: (&OwnedFd, &OsStr, &Path),
    destination: (&OwnedFd, &OsStr),
    before: EntryStat,
    cancelled: &AtomicBool,
    progress: &mut dyn FnMut(&Path),
) -> io::Result<()> {
    let (source_parent, source_name, source_path) = source;
    let (destination_parent, destination_name) = destination;
    let source_fd = openat(
        source_parent,
        source_name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    let opened = stat_value(fstat(&source_fd).map_err(io::Error::from)?);
    if opened.kind != EntryKind::Directory || opened.dev != before.dev || opened.ino != before.ino {
        return Err(invalid_data(format!(
            "{} changed before it was copied",
            source_path.display()
        )));
    }
    mkdirat(
        destination_parent,
        destination_name,
        Mode::from_raw_mode(PRIVATE_DIRECTORY_MODE),
    )
    .map_err(io::Error::from)?;
    let destination_fd = openat(
        destination_parent,
        destination_name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    for name in directory_names_from(&source_fd)? {
        check_cancelled(cancelled)?;
        copy_entry(
            &source_fd,
            &name,
            &source_path.join(&name),
            &destination_fd,
            &name,
            cancelled,
            progress,
        )?;
    }
    let source_file: File = source_fd.into();
    let destination_file: File = destination_fd.into();
    let after = stat_value(fstat(&source_file).map_err(io::Error::from)?);
    if after.dev != before.dev
        || after.ino != before.ino
        || after.mtime != before.mtime
        || after.mtime_nsec != before.mtime_nsec
    {
        return Err(invalid_data(format!(
            "{} changed while it was copied",
            source_path.display()
        )));
    }
    copy_xattrs(&source_file, &destination_file);
    apply_mode_and_times(
        &destination_file,
        before.mode,
        (before.atime, before.atime_nsec),
        (before.mtime, before.mtime_nsec),
    )
}

pub(super) fn copy_xattrs(source: &File, destination: &File) {
    let Ok(names) = source.list_xattr() else {
        return;
    };
    for name in names {
        if let Ok(Some(value)) = source.get_xattr(&name) {
            let _ = destination.set_xattr(&name, &value);
        }
    }
}

pub(super) fn apply_mode_and_times(
    fd: &impl AsFd,
    mode: u32,
    atime: (i64, i64),
    mtime: (i64, i64),
) -> io::Result<()> {
    fchmod(fd, Mode::from_raw_mode(mode)).map_err(io::Error::from)?;
    let timestamps = Timestamps {
        last_access: Timespec {
            tv_sec: atime.0,
            tv_nsec: atime.1,
        },
        last_modification: Timespec {
            tv_sec: mtime.0,
            tv_nsec: mtime.1,
        },
    };
    rustix::fs::futimens(fd, &timestamps).map_err(io::Error::from)
}
