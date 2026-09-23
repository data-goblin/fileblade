use serde::Serialize;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::Shutdown;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub enum Output {
    Stdio(Arc<fileblade_output::Output>),
    Socket(Mutex<Option<UnixStream>>),
}

impl Output {
    pub fn socket(stream: UnixStream) -> io::Result<Self> {
        stream.set_write_timeout(Some(Duration::from_millis(200)))?;
        Ok(Self::Socket(Mutex::new(Some(stream))))
    }

    pub fn machine<T: Serialize>(&self, value: &T) -> io::Result<()> {
        match self {
            Self::Stdio(output) => output.machine(value),
            Self::Socket(socket) => {
                let mut guard = socket
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let Some(stream) = guard.as_mut() else {
                    return Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "subscriber detached",
                    ));
                };
                let mut bytes = serde_json::to_vec(value).map_err(io::Error::other)?;
                bytes.push(b'\n');
                if let Err(error) = stream.write_all(&bytes) {
                    if let Some(stream) = guard.take() {
                        let _ = stream.shutdown(Shutdown::Both);
                    }
                    return Err(error);
                }
                Ok(())
            }
        }
    }

    pub fn detach(&self) {
        if let Self::Socket(socket) = self
            && let Some(stream) = socket
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
        {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

pub fn connect(root: &Path) -> io::Result<UnixStream> {
    let directory = std::fs::File::open(std::fs::canonicalize(root)?)?;
    UnixStream::connect(format!(
        "/proc/self/fd/{}/authority.sock",
        directory.as_raw_fd()
    ))
}

pub fn probe(root: &Path) -> io::Result<()> {
    let mut stream = connect(root)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(b"{\"v\":1,\"type\":\"hello\"}\n")?;
    let mut line = String::new();
    BufReader::new(stream).take(4096).read_line(&mut line)?;
    let response: serde_json::Value = serde_json::from_str(&line).map_err(io::Error::other)?;
    if response["ok"] != true || response["authority"] != true {
        return Err(io::Error::other(
            "native authority did not acknowledge readiness",
        ));
    }
    Ok(())
}

pub fn bridge(root: &Path) -> io::Result<()> {
    let mut stream = connect(root)
        .map_err(|error| io::Error::other(format!("native owner-unavailable: {error}")))?;
    let mut sender = stream.try_clone()?;
    std::thread::spawn(move || {
        let _ = relay(&mut io::stdin().lock(), &mut sender);
        let _ = sender.shutdown(Shutdown::Write);
    });
    relay(&mut stream, &mut io::stdout().lock())?;
    Ok(())
}

fn relay(reader: &mut impl Read, writer: &mut impl Write) -> io::Result<()> {
    let mut buffer = [0; 16 * 1024];
    loop {
        let count = match reader.read(&mut buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            return Ok(());
        }
        writer.write_all(&buffer[..count])?;
        writer.flush()?;
    }
}
