use super::entry::Planned;
use std::path::Path;

pub fn plan(launcher: &Path, config: &Path, data: &Path) -> Vec<Planned> {
    let portals = config.join("xdg-desktop-portal");
    let preference = if portals.join("hyprland-portals.conf").is_file()
        && !portals.join("portals.conf").is_file()
    {
        portals.join("hyprland-portals.conf")
    } else {
        portals.join("portals.conf")
    };
    vec![
        Planned::whole(
            data.join("xdg-desktop-portal/portals/fileblade.portal"),
            "[portal]\nDBusName=org.freedesktop.impl.portal.desktop.fileblade\nInterfaces=org.freedesktop.impl.portal.FileChooser\nUseIn=Hyprland\n".to_string(),
        ),
        Planned::whole(
            data.join("dbus-1/services/org.freedesktop.impl.portal.desktop.fileblade.service"),
            format!(
                "[D-BUS Service]\nName=org.freedesktop.impl.portal.desktop.fileblade\nExec={} native portal\n",
                super::entry::exec_path(launcher)
            ),
        ),
        Planned::ini(
            preference,
            "preferred",
            "org.freedesktop.impl.portal.FileChooser",
            "fileblade",
        ),
    ]
}
