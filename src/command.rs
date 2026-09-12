use crate::{AppError, AppResult};
use rustix::process::{Pid, Signal, kill_process_group};
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

mod disk;
mod guard;
mod io;
mod supervisor;

const MAX_DETACHED_CHILDREN: usize = 64;
static DETACHED_REAPER: Mutex<Option<mpsc::SyncSender<Child>>> = Mutex::new(None);
static DETACHED_CHILDREN: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone)]
pub struct CommandSpec {
    pub program: PathBuf,
    pub arguments: Vec<OsString>,
    pub environment: BTreeMap<OsString, OsString>,
    inherit_environment: bool,
    pub cwd: Option<PathBuf>,
    pub timeout: Duration,
    pub stdout_limit: usize,
    pub stderr_limit: usize,
    pub retain_tail: bool,
    stdin_data: Option<Vec<u8>>,
    directory_budget: Option<disk::Budget>,
    file_limit: Option<u64>,
    memory_limit: Option<u64>,
    stop_on_output_limit: bool,
}

#[derive(Debug)]
pub struct CommandOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

impl CommandSpec {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            arguments: Vec::new(),
            environment: BTreeMap::new(),
            inherit_environment: true,
            cwd: None,
            timeout: Duration::from_secs(3),
            stdout_limit: 1024 * 1024,
            stderr_limit: 256 * 1024,
            retain_tail: false,
            stdin_data: None,
            directory_budget: None,
            file_limit: None,
            memory_limit: None,
            stop_on_output_limit: false,
        }
    }

    pub fn args<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.arguments.extend(
            values
                .into_iter()
                .map(|value| value.as_ref().to_os_string()),
        );
        self
    }

    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.environment.insert(key.into(), value.into());
        self
    }

    pub fn env_clear(mut self) -> Self {
        self.inherit_environment = false;
        self
    }

    pub fn cwd(mut self, value: impl Into<PathBuf>) -> Self {
        self.cwd = Some(value.into());
        self
    }

    pub fn timeout(mut self, value: Duration) -> Self {
        self.timeout = value;
        self
    }

    pub fn limits(mut self, stdout: usize, stderr: usize) -> Self {
        self.stdout_limit = stdout;
        self.stderr_limit = stderr;
        self
    }

    pub fn directory_budget(mut self, path: &Path, bytes: u64, entries: usize) -> Self {
        self.directory_budget = Some(disk::budget(path, bytes, entries));
        self
    }

    pub fn resource_limits(mut self, file_bytes: u64, memory_bytes: u64) -> Self {
        self.file_limit = Some(file_bytes);
        self.memory_limit = Some(memory_bytes);
        self
    }

    pub fn stop_on_output_limit(mut self) -> Self {
        self.stop_on_output_limit = true;
        self
    }

    pub fn retain_tail(mut self, value: bool) -> Self {
        self.retain_tail = value;
        self
    }

    pub fn stdin(mut self, value: impl Into<Vec<u8>>) -> Self {
        self.stdin_data = Some(value.into());
        self
    }

    pub fn run(&self) -> AppResult<CommandOutput> {
        self.run_cancellable(&AtomicBool::new(false))
    }

    pub fn run_cancellable(&self, cancelled: &AtomicBool) -> AppResult<CommandOutput> {
        io::run(self, cancelled)
    }

    pub fn spawn_detached(&self) -> AppResult<u32> {
        let reaper = detached_reaper()?;
        reserve_detached_child()?;
        let mut command = self.command();
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                release_detached_child();
                return Err(AppError::command(format!(
                    "could not start {}: {error}",
                    self.program.display()
                )));
            }
        };
        let pid = child.id();
        if let Err(error) = reaper.try_send(child) {
            let (mut child, message) = match error {
                mpsc::TrySendError::Full(child) => (child, "detached child reaper is saturated"),
                mpsc::TrySendError::Disconnected(child) => {
                    (child, "detached child reaper is unavailable")
                }
            };
            terminate_group(&mut child);
            release_detached_child();
            return Err(AppError::command(message));
        }
        Ok(pid)
    }

    pub fn spawn_detached_with_stdin(&self, cancelled: &AtomicBool) -> AppResult<u32> {
        let Some(input) = self.stdin_data.clone() else {
            return self.spawn_detached();
        };
        let reaper = detached_reaper()?;
        reserve_detached_child()?;
        let mut command = self.command();
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                release_detached_child();
                return Err(AppError::command(format!(
                    "could not start {}: {error}",
                    self.program.display()
                )));
            }
        };
        let pid = child.id();
        if let Err(error) = write_detached_stdin(&mut child, &input, self.timeout, cancelled)
            .and_then(|()| settle_detached(&mut child))
        {
            terminate_group(&mut child);
            release_detached_child();
            return Err(error);
        }
        if let Err(error) = reaper.try_send(child) {
            let (mut child, message) = match error {
                mpsc::TrySendError::Full(child) => (child, "detached child reaper is saturated"),
                mpsc::TrySendError::Disconnected(child) => {
                    (child, "detached child reaper is unavailable")
                }
            };
            terminate_group(&mut child);
            release_detached_child();
            return Err(AppError::command(message));
        }
        Ok(pid)
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.arguments);
        if !self.inherit_environment {
            command.env_clear();
        }
        command.envs(&self.environment);
        if let Some(cwd) = &self.cwd {
            command.current_dir(cwd);
        }
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        command
    }
}

fn reserve_detached_child() -> AppResult<()> {
    DETACHED_CHILDREN
        .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |count| {
            (count < MAX_DETACHED_CHILDREN).then_some(count + 1)
        })
        .map(|_| ())
        .map_err(|_| {
            AppError::command(format!(
                "detached child limit reached ({MAX_DETACHED_CHILDREN})"
            ))
        })
}

fn release_detached_child() {
    DETACHED_CHILDREN.fetch_sub(1, Ordering::AcqRel);
}

fn detached_reaper() -> AppResult<mpsc::SyncSender<Child>> {
    let mut reaper = DETACHED_REAPER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(sender) = reaper.as_ref() {
        return Ok(sender.clone());
    }
    let (sender, receiver) = mpsc::sync_channel(MAX_DETACHED_CHILDREN);
    thread::Builder::new()
        .name("fileblade-child-reaper".to_string())
        .spawn(move || reap_detached(receiver))
        .map_err(|error| AppError::command(format!("could not start child reaper: {error}")))?;
    *reaper = Some(sender.clone());
    Ok(sender)
}

fn reap_detached(receiver: mpsc::Receiver<Child>) {
    let mut children = Vec::new();
    let mut disconnected = false;
    loop {
        if disconnected {
            thread::sleep(Duration::from_millis(20));
        } else {
            match receiver.recv_timeout(Duration::from_millis(20)) {
                Ok(child) => children.push(child),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => disconnected = true,
            }
            while let Ok(child) = receiver.try_recv() {
                children.push(child);
            }
        }
        let mut index = 0;
        while index < children.len() {
            match children[index].try_wait() {
                Ok(None) => index += 1,
                Ok(Some(_)) | Err(_) => {
                    let mut child = children.swap_remove(index);
                    let _ = child.wait();
                    release_detached_child();
                }
            }
        }
        if disconnected && children.is_empty() {
            return;
        }
    }
}

pub fn which(name: &str) -> Option<PathBuf> {
    if name.contains('/') {
        let path = PathBuf::from(name);
        return executable(&path).then_some(path);
    }
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
        .map(|directory| directory.join(name))
        .find(|path| executable(path))
}

fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

fn settle_detached(child: &mut Child) -> AppResult<()> {
    let deadline = Instant::now() + Duration::from_millis(200);
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                return Err(AppError::command(format!(
                    "the detached command exited with {status}"
                )));
            }
            Ok(None) => thread::sleep(Duration::from_millis(5)),
            Err(error) => {
                return Err(AppError::command(format!(
                    "could not check the detached command: {error}"
                )));
            }
        }
    }
    Ok(())
}

fn write_detached_stdin(
    child: &mut Child,
    input: &[u8],
    timeout: Duration,
    cancelled: &AtomicBool,
) -> AppResult<()> {
    use std::io::Write;
    use std::os::fd::AsRawFd;

    let Some(mut pipe) = child.stdin.take() else {
        return Err(AppError::command("detached child has no standard input"));
    };
    if input.is_empty() {
        return Ok(());
    }
    io::nonblocking(&pipe).map_err(|error| {
        AppError::command(format!("could not prepare the detached input: {error}"))
    })?;
    let deadline = Instant::now() + timeout;
    let mut written = 0;
    while written < input.len() {
        if cancelled.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(AppError::command("detached input write timed out"));
        }
        let mut fds = [io::pollfd(pipe.as_raw_fd(), libc::POLLOUT)];
        if unsafe { libc::poll(fds.as_mut_ptr(), 1, 50) } < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(AppError::command(format!(
                "could not wait for the detached input: {error}"
            )));
        }
        match pipe.write(&input[written..]) {
            Ok(0) => return Err(AppError::command("detached child closed its input")),
            Ok(count) => written += count,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) => {}
            Err(error) => {
                return Err(AppError::command(format!(
                    "could not write the detached input: {error}"
                )));
            }
        }
    }
    Ok(())
}

fn terminate_group(child: &mut Child) {
    if let Some(pid) = Pid::from_raw(child.id() as i32) {
        let _ = kill_process_group(pid, Signal::TERM);
        let until = Instant::now() + Duration::from_millis(150);
        // Do not reap the leader before the final group signal: an exited leader
        // can leave live descendants, and its unreaped PID reserves the PGID.
        thread::sleep(until.saturating_duration_since(Instant::now()));
        let _ = kill_process_group(pid, Signal::KILL);
    } else {
        let _ = child.kill();
    }
    let _ = child.wait();
}

pub fn cancellation() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}
