use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

const MAX_SYMLINKS: usize = 40;

pub fn realpath(path: &Path) -> PathBuf {
    let mut pending: Vec<OsString> = Vec::new();
    for part in path
        .as_os_str()
        .as_bytes()
        .split(|byte| *byte == b'/')
        .rev()
    {
        if !part.is_empty() {
            pending.push(OsString::from_vec(part.to_vec()));
        }
    }
    let mut resolved = PathBuf::from("/");
    let mut followed = 0usize;
    while let Some(part) = pending.pop() {
        if part == *"." {
            continue;
        }
        if part == *".." {
            resolved.pop();
            continue;
        }
        let candidate = resolved.join(&part);
        match std::fs::read_link(&candidate) {
            Ok(target) if followed < MAX_SYMLINKS => {
                followed += 1;
                if target.is_absolute() {
                    resolved = PathBuf::from("/");
                }
                for piece in target
                    .as_os_str()
                    .as_bytes()
                    .split(|byte| *byte == b'/')
                    .rev()
                {
                    if !piece.is_empty() {
                        pending.push(OsString::from_vec(piece.to_vec()));
                    }
                }
            }
            _ => resolved = candidate,
        }
    }
    resolved
}
