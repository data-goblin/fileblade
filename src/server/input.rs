use super::wake::{Wake, wait};
use super::*;
use std::io::Read;
use std::os::fd::{AsFd, AsRawFd};
use std::sync::atomic::AtomicI32;

static INTERRUPTED: AtomicBool = AtomicBool::new(false);
static SIGNAL_WAKE: AtomicI32 = AtomicI32::new(-1);

extern "C" fn interrupt(_: libc::c_int) {
    INTERRUPTED.store(true, Ordering::Relaxed);
    let descriptor = SIGNAL_WAKE.load(Ordering::Relaxed);
    if descriptor >= 0 {
        let one = 1u64.to_ne_bytes();
        unsafe {
            libc::write(descriptor, one.as_ptr().cast(), one.len());
        }
    }
}

pub(super) fn interrupted() -> bool {
    INTERRUPTED.load(Ordering::Relaxed)
}

pub(super) struct InputSignals {
    term: libc::sigaction,
    int: libc::sigaction,
    wake: Wake,
}

impl InputSignals {
    pub(super) fn install() -> io::Result<Self> {
        INTERRUPTED.store(false, Ordering::Relaxed);
        let wake = Wake::new()?;
        SIGNAL_WAKE.store(wake.fd().as_raw_fd(), Ordering::Relaxed);
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = interrupt as *const () as usize;
            libc::sigemptyset(&mut action.sa_mask);
            let mut previous = Self {
                term: std::mem::zeroed(),
                int: std::mem::zeroed(),
                wake,
            };
            if libc::sigaction(libc::SIGTERM, &action, &mut previous.term) != 0 {
                SIGNAL_WAKE.store(-1, Ordering::Relaxed);
                return Err(io::Error::last_os_error());
            }
            if libc::sigaction(libc::SIGINT, &action, &mut previous.int) != 0 {
                let error = io::Error::last_os_error();
                libc::sigaction(libc::SIGTERM, &previous.term, std::ptr::null_mut());
                SIGNAL_WAKE.store(-1, Ordering::Relaxed);
                return Err(error);
            }
            Ok(previous)
        }
    }
}

impl InputSignals {
    pub(super) fn wake(&self) -> &Wake {
        &self.wake
    }
}

impl Drop for InputSignals {
    fn drop(&mut self) {
        unsafe {
            libc::sigaction(libc::SIGTERM, &self.term, std::ptr::null_mut());
            libc::sigaction(libc::SIGINT, &self.int, std::ptr::null_mut());
        }
        SIGNAL_WAKE.store(-1, Ordering::Relaxed);
    }
}

pub(super) struct InterruptibleStdin<'a> {
    signals: &'a Wake,
}

impl<'a> InterruptibleStdin<'a> {
    pub(super) fn new(signals: &'a InputSignals) -> Self {
        Self {
            signals: signals.wake(),
        }
    }
}

impl Read for InterruptibleStdin<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let stdin = io::stdin();
        loop {
            if buffer.is_empty() || interrupted() {
                return Ok(0);
            }
            let mut descriptors = [
                PollFd::new(&stdin, PollFlags::IN),
                PollFd::from_borrowed_fd(self.signals.fd(), PollFlags::IN),
            ];
            wait(&mut descriptors, None)?;
            if descriptors[0].revents().is_empty() {
                continue;
            }
            match rustix::io::read(stdin.as_fd(), &mut *buffer) {
                Err(Errno::INTR) => continue,
                result => return result.map_err(io::Error::from),
            }
        }
    }
}
