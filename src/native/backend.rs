use crate::lease::transport;
use crate::{AppError, AppResult};
use fileblade_output::Output;
use serde_json::{Value, json};
use std::ffi::OsString;
use std::io::{self, BufRead, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

const VERSION: u64 = 1;
const MAX_FRAME_BYTES: usize = 32 * 1024 * 1024;
const DEADLINE_MS: u64 = 900_000;

pub fn run(arguments: Vec<OsString>, output: Arc<Output>) -> AppResult<bool> {
    let arguments = arguments
        .into_iter()
        .map(|argument| {
            argument
                .into_string()
                .map_err(|_| AppError::invalid("native backend arguments must be valid UTF-8"))
        })
        .collect::<AppResult<Vec<_>>>()?;
    request(
        &arguments,
        Duration::from_millis(DEADLINE_MS),
        &mut |value| {
            output.machine(value)?;
            Ok(())
        },
    )
}

pub(crate) fn request(
    arguments: &[String],
    timeout: Duration,
    emit: &mut dyn FnMut(&Value) -> AppResult<()>,
) -> AppResult<bool> {
    let command = arguments
        .first()
        .cloned()
        .filter(|command| !command.is_empty())
        .ok_or_else(|| AppError::invalid("native backend requires a command"))?;
    let request_arguments = &arguments[1..];
    let deadline_ms = timeout.as_millis().min(u128::from(DEADLINE_MS)) as u64;
    let root = crate::lease::selected_root()?.ok_or_else(|| {
        AppError::command("native owner-unavailable: native state root is not configured")
    })?;
    let mut stream = transport::connect(&root).map_err(owner_unavailable)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut reader = io::BufReader::new(stream.try_clone()?);
    write_frame(&mut stream, &json!({"v": VERSION, "type": "hello"}))?;
    let hello = read_frame(&mut reader)?.ok_or_else(|| {
        AppError::command("native owner-unavailable: native authority closed during handshake")
    })?;
    if hello.get("v").and_then(Value::as_u64) != Some(VERSION)
        || hello.get("type").and_then(Value::as_str) != Some("hello")
        || hello.get("ok") != Some(&Value::Bool(true))
        || hello.get("authority") != Some(&Value::Bool(true))
    {
        return Err(AppError::command(
            hello.get("error").and_then(Value::as_str).map_or_else(
                || {
                    "native owner-unavailable: native authority did not acknowledge readiness"
                        .to_string()
                },
                |error| format!("native owner-unavailable: {error}"),
            ),
        ));
    }
    stream.set_read_timeout(Some(Duration::from_millis(deadline_ms)))?;
    let id = format!("native-{}", Uuid::new_v4());
    let generation = Value::from(1_u64);
    write_frame(
        &mut stream,
        &json!({
            "v": VERSION,
            "type": "request",
            "id": id.clone(),
            "generation": generation.clone(),
            "command": command,
            "arguments": request_arguments,
            "deadline_ms": deadline_ms,
        }),
    )?;
    loop {
        let frame = read_frame(&mut reader)?.ok_or_else(|| {
            AppError::command("native authority closed before the backend request completed")
        })?;
        match frame.get("type").and_then(Value::as_str) {
            Some("accepted") => {
                validate_frame(&frame, &id, &generation)?;
            }
            Some("progress") => {
                validate_frame(&frame, &id, &generation)?;
                emit(frame.get("payload").unwrap_or(&Value::Null))?;
            }
            Some("response") => {
                validate_frame(&frame, &id, &generation)?;
                emit(frame.get("payload").unwrap_or(&Value::Null))?;
                if let Some(operation) = frame.get("op").and_then(Value::as_str) {
                    write_frame(
                        &mut stream,
                        &json!({"v": VERSION, "type": "operation", "action": "fetch",
                            "id": id, "generation": generation, "op": operation}),
                    )?;
                    let fetched = read_frame(&mut reader)?.ok_or_else(|| {
                        protocol_error("authority closed before acknowledging result fetch")
                    })?;
                    validate_frame(&fetched, &id, &generation)?;
                    if fetched["type"] != "operation"
                        || fetched["op"] != operation
                        || fetched["ok"] != true
                        || fetched["payload"]["complete"] != true
                    {
                        return Err(protocol_error(
                            "completed result fetch was not acknowledged",
                        ));
                    }
                }
                if frame.get("ok") != Some(&Value::Bool(true)) {
                    return Err(AppError::command(frame_error(&frame)));
                }
                let payload_ok = frame
                    .get("payload")
                    .and_then(Value::as_object)
                    .and_then(|payload| payload.get("ok"))
                    != Some(&Value::Bool(false));
                return Ok(payload_ok);
            }
            Some("error") => return Err(AppError::command(frame_error(&frame))),
            Some(kind) => {
                return Err(protocol_error(format!("unexpected frame type {kind}")));
            }
            None => return Err(protocol_error("frame type is required")),
        }
    }
}

fn owner_unavailable(error: io::Error) -> AppError {
    AppError::command(format!("native owner-unavailable: {error}"))
}

fn frame_error(frame: &Value) -> String {
    frame
        .get("error")
        .and_then(Value::as_str)
        .filter(|error| !error.is_empty())
        .unwrap_or("native backend request failed")
        .to_string()
}

fn protocol_error(message: impl Into<String>) -> AppError {
    AppError::command(format!("native authority protocol: {}", message.into()))
}

fn write_frame(stream: &mut UnixStream, frame: &Value) -> AppResult<()> {
    serde_json::to_writer(&mut *stream, frame)?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    Ok(())
}

fn validate_frame(frame: &Value, id: &str, generation: &Value) -> AppResult<()> {
    if frame.get("v").and_then(Value::as_u64) != Some(VERSION)
        || frame.get("id").and_then(Value::as_str) != Some(id)
        || frame.get("generation") != Some(generation)
    {
        return Err(protocol_error("frame does not match the backend request"));
    }
    Ok(())
}

fn read_frame(reader: &mut impl BufRead) -> AppResult<Option<Value>> {
    let mut frame = Vec::new();
    let count =
        Read::take(&mut *reader, (MAX_FRAME_BYTES + 1) as u64).read_until(b'\n', &mut frame)?;
    if count == 0 {
        return Ok(None);
    }
    if count > MAX_FRAME_BYTES {
        return Err(protocol_error("authority frame exceeds the response limit"));
    }
    if frame.last() != Some(&b'\n') {
        return Err(protocol_error("authority closed with an incomplete frame"));
    }
    frame.pop();
    serde_json::from_slice(&frame)
        .map(Some)
        .map_err(|error| protocol_error(error.to_string()))
}
