use super::entry::Planned;
use std::path::Path;

pub fn plan(launcher: &Path, config: &Path) -> Vec<Planned> {
    vec![Planned::whole(
        config.join("autostart/fileblade.desktop"),
        format!(
            "[Desktop Entry]\nType=Application\nName=FileBlade\nExec=/usr/bin/env -- {}\nX-GNOME-Autostart-enabled=true\n",
            super::entry::exec_path(launcher).replace('%', "%%")
        ),
    )]
}
