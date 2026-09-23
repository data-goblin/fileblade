use super::entry::Planned;
use std::path::Path;

pub const NAME: &str = "org.freedesktop.FileManager1";

pub fn plan(launcher: &Path, data: &Path) -> Vec<Planned> {
    vec![Planned::whole(
        data.join("dbus-1/services/org.freedesktop.FileManager1.service"),
        format!(
            "[D-BUS Service]\nName={NAME}\nExec={} native filemanager1\n",
            super::entry::exec_path(launcher)
        ),
    )]
}

pub fn conflict() -> String {
    let Some(holder) = owner() else {
        return String::new();
    };
    if holder.starts_with("fileblade") {
        return String::new();
    }
    format!("{holder} currently owns {NAME}; log out and in for FileBlade to take over")
}

fn owner() -> Option<String> {
    let connection = zbus::blocking::Connection::session().ok()?;
    let bus = zbus::blocking::fdo::DBusProxy::new(&connection).ok()?;
    let unique = bus.get_name_owner(NAME.try_into().ok()?).ok()?;
    let pid = bus
        .get_connection_unix_process_id(unique.as_ref().into())
        .ok();
    let name = pid
        .and_then(|pid| std::fs::read_to_string(format!("/proc/{pid}/comm")).ok())
        .map(|comm| comm.trim().to_string())
        .filter(|comm| !comm.is_empty());
    Some(match (name, pid) {
        (Some(name), Some(pid)) => format!("{name} (pid {pid})"),
        (None, Some(pid)) => format!("pid {pid}"),
        _ => unique.to_string(),
    })
}
