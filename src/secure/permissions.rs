use super::*;

const SYS_FCHMODAT2: libc::c_long = 452;

pub fn set_permissions_matching(
    path: &Path,
    expected: EntryStat,
    mode: u32,
) -> io::Result<EntryStat> {
    if mode & !0o7777 != 0 {
        return Err(invalid_data(
            "permissions must be an octal mode between 0000 and 7777",
        ));
    }
    let parent = resolved_parent(path)?;
    let fd = openat(
        &parent.directory,
        &parent.name,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    let current = stat_value(fstat(&fd).map_err(io::Error::from)?);
    if !matches!(current.kind, EntryKind::File | EntryKind::Directory) {
        return Err(invalid_data(
            "permissions can only be changed on regular files and directories",
        ));
    }
    if current.identity() != expected.identity() || current.mode != expected.mode {
        return Err(invalid_data(
            "item identity or permissions changed; review it again",
        ));
    }
    let result = unsafe {
        libc::syscall(
            SYS_FCHMODAT2,
            fd.as_raw_fd(),
            c"".as_ptr(),
            mode as libc::mode_t,
            libc::AT_EMPTY_PATH,
        )
    };
    if result != 0 {
        let error = io::Error::last_os_error();
        return Err(if error.raw_os_error() == Some(libc::ENOSYS) {
            io::Error::new(
                io::ErrorKind::Unsupported,
                "permission editing requires Linux fchmodat2 support",
            )
        } else {
            error
        });
    }
    Ok(stat_value(fstat(&fd).map_err(io::Error::from)?))
}
