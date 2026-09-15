use super::{Volume, list, mountinfo};
use crate::command::{CommandSpec, which};
use crate::{AppError, AppResult};
use serde_json::{Value, json};
use std::time::Duration;

const PROGRAM: &str = "udisksctl";
const NOT_AUTHORIZED: &str = "NotAuthorizedCanObtain";
const QUICK_TIMEOUT: Duration = Duration::from_secs(20);
const INTERACTIVE_TIMEOUT: Duration = Duration::from_secs(180);

pub fn available() -> bool {
    which(PROGRAM).is_some()
}

fn resolve(source: &str) -> AppResult<Volume> {
    list()?
        .into_iter()
        .find(|volume| volume.source == source)
        .ok_or_else(|| AppError::invalid(format!("{source} is not a known volume")))
}

fn invoke(verb: &str, source: &str, interactive: bool) -> AppResult<(bool, String)> {
    let program = which(PROGRAM)
        .ok_or_else(|| AppError::command("udisksctl is not installed".to_string()))?;
    let mut arguments = vec![verb.to_string(), "-b".to_string(), source.to_string()];
    if !interactive {
        arguments.push("--no-user-interaction".to_string());
    }
    let output = CommandSpec::new(program)
        .args(arguments)
        .timeout(if interactive {
            INTERACTIVE_TIMEOUT
        } else {
            QUICK_TIMEOUT
        })
        .run()?;
    let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Ok((output.status.success(), message))
}

fn tidy(message: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for line in message.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("dmesg(1)")
            || line.starts_with("Error creating textual authentication agent")
        {
            continue;
        }
        let mut text = line.to_string();
        while let Some(start) = text.find("GDBus.Error:") {
            let Some(length) = text[start..].find(": ") else {
                break;
            };
            text.replace_range(start..start + length + 2, "");
        }
        while text.starts_with("Error ") {
            let Some((_, rest)) = text.split_once(": ") else {
                break;
            };
            if rest.trim().is_empty() {
                break;
            }
            text = rest.trim().to_string();
        }
        parts.push(text);
    }
    parts.join(" ")
}

fn failure(verb: &str, source: &str, message: &str) -> AppError {
    let reason = tidy(message);
    if reason.is_empty() {
        AppError::command(format!("could not {verb} {source}"))
    } else {
        AppError::command(format!("could not {verb} {source}: {reason}"))
    }
}

fn authorize(verb: &str, source: &str) -> AppResult<Value> {
    let (succeeded, message) = invoke(verb, source, false)?;
    if succeeded {
        return Ok(json!({"authorized": false}));
    }
    if !message.contains(NOT_AUTHORIZED) {
        return Err(failure(verb, source, &message));
    }
    let (succeeded, message) = invoke(verb, source, true)?;
    if !succeeded {
        return Err(failure(verb, source, &message));
    }
    Ok(json!({"authorized": true}))
}

fn mountpoint_of(source: &str) -> AppResult<Option<String>> {
    let table = mountinfo::MountTable::read()?;
    let Some(number) = mountinfo::device_number_of(std::path::Path::new(source)) else {
        return Ok(None);
    };
    Ok(table
        .primary(&number)
        .map(|record| crate::common::path_text(&record.mountpoint)))
}

pub fn mount(source: &str) -> AppResult<Value> {
    let volume = resolve(source)?;
    if let Some(mountpoint) = volume.mountpoint {
        return Ok(json!({
            "ok": true,
            "source": source,
            "mountpoint": mountpoint,
            "changed": false,
        }));
    }
    let result = authorize("mount", source)?;
    Ok(json!({
        "ok": true,
        "source": source,
        "mountpoint": mountpoint_of(source)?,
        "authorized": result["authorized"],
        "changed": true,
    }))
}

pub fn unmount(source: &str) -> AppResult<Value> {
    let volume = resolve(source)?;
    if volume.mountpoint.is_none() {
        return Ok(json!({
            "ok": true,
            "source": source,
            "mountpoint": Value::Null,
            "changed": false,
        }));
    }
    let result = authorize("unmount", source)?;
    Ok(json!({
        "ok": true,
        "source": source,
        "mountpoint": mountpoint_of(source)?,
        "authorized": result["authorized"],
        "changed": true,
    }))
}

pub fn eject(source: &str) -> AppResult<Value> {
    let volume = resolve(source)?;
    if !volume.external {
        return Err(AppError::invalid(format!(
            "{source} is not a removable volume"
        )));
    }
    if volume.mountpoint.is_some() {
        authorize("unmount", source)?;
    }
    let result = authorize(
        if volume.image.is_some() {
            "loop-delete"
        } else {
            "power-off"
        },
        source,
    )?;
    Ok(json!({
        "ok": true,
        "source": source,
        "authorized": result["authorized"],
        "changed": true,
    }))
}

#[cfg(test)]
mod tests {
    use super::tidy;

    #[test]
    fn tidy_keeps_the_reason_mount_gave() {
        let raw = "Error creating textual authentication agent: Error opening current controlling terminal for the process (`/dev/tty'): No such device or address (polkit-error-quark, 0)\nError mounting /dev/sda3: GDBus.Error:org.freedesktop.UDisks2.Error.Failed: Error mounting system-managed device /dev/sda3: wrong fs type, bad option, bad superblock on /dev/sda3, missing codepage or helper program, or other error.\n       dmesg(1) may have more information after failed mount system call.\n";
        assert_eq!(
            tidy(raw),
            "wrong fs type, bad option, bad superblock on /dev/sda3, missing codepage or helper program, or other error."
        );
    }

    #[test]
    fn tidy_strips_the_dbus_error_name_but_keeps_plain_text() {
        let raw = "Error mounting /dev/sdb1: GDBus.Error:org.freedesktop.UDisks2.Error.NotAuthorizedCanObtain: Not authorized to perform operation";
        assert_eq!(tidy(raw), "Not authorized to perform operation");
        assert_eq!(tidy("  \n"), "");
        assert_eq!(tidy("Error: "), "Error:");
        assert_eq!(tidy("device is busy"), "device is busy");
    }
}
