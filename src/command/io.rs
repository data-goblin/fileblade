use super::{CommandOutput, CommandSpec, guard, supervisor::Running};
use crate::{AppError, AppResult};
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, AsRawFd, RawFd};
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

struct Capture<R> {
    pipe: Option<R>,
    bytes: Vec<u8>,
    limit: usize,
    truncated: bool,
    tail: bool,
}

impl<R: Read + AsFd> Capture<R> {
    fn new(pipe: Option<R>, limit: usize, tail: bool) -> AppResult<Self> {
        let pipe = pipe.ok_or_else(|| AppError::command("command output pipe unavailable"))?;
        nonblocking(&pipe)?;
        Ok(Self {
            pipe: Some(pipe),
            bytes: Vec::with_capacity(limit.min(4096)),
            limit,
            truncated: false,
            tail,
        })
    }

    fn read(&mut self) -> io::Result<()> {
        let Some(pipe) = self.pipe.as_mut() else {
            return Ok(());
        };
        let mut chunk = [0_u8; 32 * 1024];
        match pipe.read(&mut chunk) {
            Ok(0) => self.pipe = None,
            Ok(count) => {
                if self.tail {
                    self.bytes.extend_from_slice(&chunk[..count]);
                    let excess = self.bytes.len().saturating_sub(self.limit);
                    if excess > 0 {
                        self.bytes.drain(..excess);
                        self.truncated = true;
                    }
                } else {
                    let accepted = count.min(self.limit.saturating_sub(self.bytes.len()));
                    self.bytes.extend_from_slice(&chunk[..accepted]);
                    self.truncated |= accepted < count;
                }
            }
            Err(error) if retryable(&error) => {}
            Err(error) => return Err(error),
        }
        Ok(())
    }

    fn fd(&self) -> RawFd {
        self.pipe
            .as_ref()
            .map_or(-1, |pipe| pipe.as_fd().as_raw_fd())
    }
}

fn retryable(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
    )
}

pub(super) fn nonblocking(fd: impl AsFd) -> io::Result<()> {
    let flags = rustix::fs::fcntl_getfl(&fd)?;
    Ok(rustix::fs::fcntl_setfl(
        fd,
        flags | rustix::fs::OFlags::NONBLOCK,
    )?)
}

pub(super) fn run(spec: &CommandSpec, cancelled: &AtomicBool) -> AppResult<CommandOutput> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(AppError::Cancelled);
    }
    let started = Instant::now();
    let mut running = Running::spawn(spec)?;
    let mut stdout = Capture::new(
        running.child.stdout.take(),
        spec.stdout_limit,
        spec.retain_tail,
    )?;
    let mut stderr = Capture::new(
        running.child.stderr.take(),
        spec.stderr_limit,
        spec.retain_tail,
    )?;
    let mut stdin = running.child.stdin.take();
    if let Some(pipe) = &stdin {
        nonblocking(pipe)?;
    }
    nonblocking(&running.result)?;
    let input = spec.stdin_data.as_deref().unwrap_or_default();
    let mut written = 0;
    let mut message = [0_u8; 8];
    let mut received = 0;
    let mut drain_until = None;
    let mut disk_checked = started;
    loop {
        if cancelled.load(Ordering::Relaxed) {
            running.cancel();
        }
        if let Some(budget) = &spec.directory_budget
            && disk_checked.elapsed() >= Duration::from_millis(50)
        {
            budget.check()?;
            disk_checked = Instant::now();
        }
        if written == input.len() {
            stdin = None;
        }
        if let Some(until) = drain_until {
            if stdout.pipe.is_none() && stderr.pipe.is_none() {
                break;
            }
            if Instant::now() >= until {
                stdout.truncated |= stdout.pipe.is_some();
                stderr.truncated |= stderr.pipe.is_some();
                break;
            }
        } else if started.elapsed() > spec.timeout.saturating_add(Duration::from_secs(2)) {
            return Err(AppError::command("command supervisor did not respond"));
        }
        let mut fds = [
            pollfd(stdin.as_ref().map_or(-1, AsRawFd::as_raw_fd), libc::POLLOUT),
            pollfd(stdout.fd(), libc::POLLIN),
            pollfd(stderr.fd(), libc::POLLIN),
            pollfd(
                if received == 8 {
                    -1
                } else {
                    running.result.as_raw_fd()
                },
                libc::POLLIN,
            ),
        ];
        // These owned pipes remain open for the poll; -1 explicitly disables an entry.
        if unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as _, 50) } < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.into());
        }
        if fds[0].revents != 0
            && let Some(pipe) = &mut stdin
        {
            match pipe.write(&input[written..input.len().min(written.saturating_add(32 * 1024))]) {
                Ok(0) => stdin = None,
                Ok(count) => written += count,
                Err(error) if error.kind() == io::ErrorKind::BrokenPipe => stdin = None,
                Err(error) if retryable(&error) => {}
                Err(error) => return Err(error.into()),
            }
        }
        if fds[1].revents != 0 {
            stdout.read()?;
        }
        if fds[2].revents != 0 {
            stderr.read()?;
        }
        if spec.stop_on_output_limit && (stdout.truncated || stderr.truncated) {
            return Err(AppError::command("command exceeded its output limit"));
        }
        if fds[3].revents != 0 {
            match running.result.read(&mut message[received..]) {
                Ok(0) => {
                    return Err(AppError::command(
                        "command supervisor closed without a result",
                    ));
                }
                Ok(count) => received += count,
                Err(error) if retryable(&error) => {}
                Err(error) => return Err(error.into()),
            }
            if received == 8 {
                stdin = None;
                drain_until = Some(Instant::now() + Duration::from_millis(150));
            }
        }
    }
    running.finish()?;
    if let Some(budget) = &spec.directory_budget {
        budget.check()?;
    }
    let status = i32::from_ne_bytes(message[..4].try_into().unwrap());
    match i32::from_ne_bytes(message[4..].try_into().unwrap()) {
        guard::NORMAL => Ok(CommandOutput {
            status: ExitStatus::from_raw(status),
            stdout: stdout.bytes,
            stderr: stderr.bytes,
            stdout_truncated: stdout.truncated,
            stderr_truncated: stderr.truncated,
        }),
        guard::CANCELLED => Err(AppError::Cancelled),
        guard::TIMED_OUT => Err(AppError::command(format!(
            "command did not respond within {} ms",
            spec.timeout.as_millis()
        ))),
        _ => Err(AppError::command(
            "command supervisor could not complete process-group cleanup",
        )),
    }
}

pub(super) fn pollfd(fd: RawFd, events: i16) -> libc::pollfd {
    libc::pollfd {
        fd,
        events,
        revents: 0,
    }
}
