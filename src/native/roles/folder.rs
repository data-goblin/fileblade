use super::entry::Planned;
use std::path::Path;

pub fn plan(launcher: &Path, config: &Path, data: &Path) -> Vec<Planned> {
    vec![
        Planned::whole(
            data.join("applications/fileblade.desktop"),
            format!(
                "[Desktop Entry]\nType=Application\nName=FileBlade\nIcon=fileblade\nExec=/usr/bin/env -- {} native open %U\nMimeType=inode/directory;\nNoDisplay=true\nCategories=System;FileTools;\n",
                super::entry::exec_path(launcher).replace('%', "%%")
            ),
        ),
        Planned::ini(
            config.join("mimeapps.list"),
            "Default Applications",
            "inode/directory",
            "fileblade.desktop",
        ),
    ]
}
