use super::{CommandSpec, guard};
use crate::{AppError, AppResult};
use std::fs::File;
use std::io::{self, Seek, Write};
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

pub(super) struct Running {
    pub child: Child,
    pub result: File,
    cancel: Option<File>,
    finished: bool,
}

fn above_stdio(fd: impl AsFd) -> io::Result<OwnedFd> {
    Ok(rustix::io::fcntl_dupfd_cloexec(fd, 3)?)
}

impl Running {
    pub fn spawn(spec: &CommandSpec) -> AppResult<Self> {
        let (cancel_read, cancel_write) = io::pipe()?;
        let (result_read, result_write) = io::pipe()?;
        let cancel_read = above_stdio(cancel_read)?;
        let cancel_write = above_stdio(cancel_write)?;
        let result_read = above_stdio(result_read)?;
        let result_write = above_stdio(result_write)?;
        let owner = above_stdio(
            rustix::process::pidfd_open(
                rustix::process::getpid(),
                rustix::process::PidfdFlags::empty(),
            )
            .map_err(io::Error::from)?,
        )?;
        let controls = guard::Controls {
            file_limit: spec.file_limit,
            memory_limit: spec.memory_limit,
            cancel: cancel_read.as_raw_fd(),
            result: result_write.as_raw_fd(),
            owner: owner.as_raw_fd(),
            deadline: guard::monotonic_ms()
                .saturating_add(spec.timeout.as_millis().min(i64::MAX as u128) as i64),
        };
        let mut command = spec.command();
        command
            .stdin(if spec.seekable_stdin {
                let mut input: File = rustix::fs::memfd_create(
                    c"fileblade-input",
                    rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
                )
                .map_err(io::Error::from)?
                .into();
                input.write_all(spec.stdin_data.as_deref().unwrap_or_default())?;
                input.rewind()?;
                rustix::fs::fcntl_add_seals(
                    &input,
                    rustix::fs::SealFlags::SEAL
                        | rustix::fs::SealFlags::SHRINK
                        | rustix::fs::SealFlags::GROW
                        | rustix::fs::SealFlags::WRITE,
                )
                .map_err(io::Error::from)?;
                Stdio::from(input)
            } else if spec.stdin_data.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        unsafe { command.pre_exec(move || guard::enter(controls)) };
        let child = command.spawn().map_err(|error| {
            AppError::command(format!(
                "could not start {}: {error}",
                spec.program.display()
            ))
        })?;
        Ok(Self {
            child,
            result: result_read.into(),
            cancel: Some(cancel_write.into()),
            finished: false,
        })
    }

    pub fn cancel(&mut self) {
        if let Some(mut pipe) = self.cancel.take() {
            let _ = pipe.write_all(&[1]);
        }
    }

    pub fn finish(&mut self) -> AppResult<()> {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut pidfd = None;
        loop {
            if let Some(status) = self.child.try_wait()? {
                self.finished = true;
                return if status.success() {
                    Ok(())
                } else {
                    Err(AppError::command("command supervisor exited unexpectedly"))
                };
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(AppError::command(
                    "command supervisor did not finish cleanup",
                ));
            }
            if pidfd.is_none() {
                pidfd = Some(
                    rustix::process::pidfd_open(
                        rustix::process::Pid::from_raw(self.child.id() as i32).unwrap(),
                        rustix::process::PidfdFlags::empty(),
                    )
                    .map_err(io::Error::from)?,
                );
            }
            let mut descriptors = [rustix::event::PollFd::new(
                pidfd.as_ref().unwrap(),
                rustix::event::PollFlags::IN,
            )];
            let timeout = rustix::event::Timespec {
                tv_sec: remaining.as_secs() as _,
                tv_nsec: remaining.subsec_nanos() as _,
            };
            match rustix::event::poll(&mut descriptors, Some(&timeout)) {
                Ok(_) | Err(rustix::io::Errno::INTR) => {}
                Err(error) => return Err(io::Error::from(error).into()),
            }
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if !self.finished {
            self.cancel();
            if self.finish().is_err() && !self.finished {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
    }
}
