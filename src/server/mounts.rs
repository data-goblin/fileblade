use super::wake::{Stop, wait};
use super::*;
use crate::mounts::mountinfo::{MOUNTINFO_PATH, MountTable};
use std::fs::File;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{Read, Seek, SeekFrom};

const DEVICE_LINK_DIRS: [&str; 2] = ["/dev/disk/by-id", "/run/media"];
const SETTLE_DELAY: Duration = Duration::from_millis(180);

pub(super) fn watch_mounts(
    key: &RequestKey,
    generation: Value,
    stop: &Stop,
    deadline_exceeded: &AtomicBool,
    output: &Output,
) {
    let result = mount_events(key, &generation, stop, output);
    let frame = closing_frame(
        key,
        generation,
        "mounts",
        result,
        &stop.cancelled,
        deadline_exceeded,
    );
    let _ = emit(output, &frame);
}

fn snapshot(file: &mut File) -> AppResult<Value> {
    let mut text = String::new();
    file.seek(SeekFrom::Start(0))?;
    file.read_to_string(&mut text)?;
    let volumes = crate::mounts::list_with(&MountTable::from_text(&text));
    let mut payload = crate::mounts::payload_with(&volumes);
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    payload["mount_table"] = Value::String(format!("{:016x}", hasher.finish()));
    Ok(payload)
}

fn device_watches(descriptor: &std::os::fd::OwnedFd) -> Vec<String> {
    let flags = WatchFlags::CREATE
        | WatchFlags::DELETE
        | WatchFlags::MOVED_FROM
        | WatchFlags::MOVED_TO
        | WatchFlags::ATTRIB
        | WatchFlags::ONLYDIR;
    DEVICE_LINK_DIRS
        .iter()
        .filter(|directory| inotify::add_watch(descriptor, **directory, flags).is_ok())
        .map(|directory| (*directory).to_string())
        .collect()
}

pub(super) fn mount_events(
    key: &RequestKey,
    generation: &Value,
    stop: &Stop,
    output: &Output,
) -> AppResult<()> {
    let mut file = File::open(MOUNTINFO_PATH)
        .map_err(|error| AppError::command(format!("could not watch {MOUNTINFO_PATH}: {error}")))?;
    let descriptor = inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK)
        .map_err(|error| AppError::command(format!("could not start mount watches: {error}")))?;
    let watched = device_watches(&descriptor);
    let mut current = snapshot(&mut file)?;
    emit(
        output,
        &json!({
            "v": VERSION,
            "type": "subscribed",
            "id": key.id,
            "generation": generation,
            "ok": true,
            "topic": "mounts",
            "paths": watched,
            "payload": current,
        }),
    )?;
    let mut storage = [MaybeUninit::<u8>::uninit(); 16 * 1024];
    while !stop.requested() {
        let mut fds = [
            PollFd::new(&file, PollFlags::PRI),
            PollFd::new(&descriptor, PollFlags::IN),
            PollFd::from_borrowed_fd(stop.wake.fd(), PollFlags::IN),
        ];
        wait(&mut fds, None)
            .map_err(|error| AppError::command(format!("mount subscription failed: {error}")))?;
        if fds[0].revents().is_empty() && fds[1].revents().is_empty() {
            continue;
        }
        let mut reader = inotify::Reader::new(&descriptor, &mut storage);
        while reader.next().is_ok() {}
        thread::sleep(SETTLE_DELAY);
        let next = snapshot(&mut file)?;
        if next == current {
            continue;
        }
        current = next;
        emit(
            output,
            &json!({
                "v": VERSION,
                "type": "event",
                "id": key.id,
                "generation": generation,
                "topic": "mounts",
                "payload": current,
            }),
        )?;
    }
    Ok(())
}
