use super::*;
use rustix::event::{EventfdFlags, eventfd};
use rustix::fd::{AsFd, BorrowedFd, OwnedFd};

pub(super) struct Wake(OwnedFd);

impl Wake {
    pub(super) fn new() -> io::Result<Self> {
        eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)
            .map(Self)
            .map_err(io::Error::from)
    }

    pub(super) fn notify(&self) {
        let _ = rustix::io::write(&self.0, &1u64.to_ne_bytes());
    }

    pub(super) fn clear(&self) {
        let mut counter = [0u8; 8];
        let _ = rustix::io::read(&self.0, &mut counter);
    }

    pub(super) fn fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

pub(super) struct Stop {
    pub(super) cancelled: Arc<AtomicBool>,
    pub(super) wake: Arc<Wake>,
}

impl Stop {
    pub(super) fn requested(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

pub(super) fn wait(descriptors: &mut [PollFd<'_>], timeout: Option<Duration>) -> io::Result<()> {
    let timeout = timeout.map(|value| Timespec {
        tv_sec: value.as_secs().min(i64::MAX as u64) as i64,
        tv_nsec: i64::from(value.subsec_nanos()),
    });
    match poll(descriptors, timeout.as_ref()) {
        Ok(_) | Err(Errno::INTR) => Ok(()),
        Err(error) => Err(error.into()),
    }
}
