use super::*;

#[derive(Clone, Debug, Default)]
pub(super) struct DesktopEntry {
    pub(super) desktop_id: String,
    pub(super) name: String,
    pub(super) icon: String,
    pub(super) icon_source: String,
    pub(super) icon_mask: String,
    pub(super) executable: String,
    pub(super) wm_class: String,
}

pub(super) fn desktop_entry_for_class(class: &str) -> Option<DesktopEntry> {
    if class.trim().is_empty() {
        return None;
    }
    let short = class.rsplit('.').next().unwrap_or(class).to_lowercase();
    for candidate in [
        format!("{class}.desktop"),
        format!("{}.desktop", class.to_lowercase()),
        format!("{short}.desktop"),
    ] {
        if let Some(path) = desktop_file_for(&candidate) {
            return parse_desktop_entry(&path);
        }
    }
    let deadline = Instant::now() + Duration::from_millis(25);
    for directory in data_directories() {
        let Ok(entries) = fs::read_dir(directory.join("applications")) else {
            continue;
        };
        for entry in entries.flatten().take(MAX_DESKTOP_FILES) {
            if Instant::now() >= deadline
                || entry.path().extension().and_then(|value| value.to_str()) != Some("desktop")
            {
                break;
            }
            if let Some(candidate) = parse_desktop_entry(&entry.path())
                && candidate.wm_class.eq_ignore_ascii_case(class)
            {
                return Some(candidate);
            }
        }
    }
    None
}

pub(super) fn applications_for(path: &str, hint: &str) -> Vec<DesktopEntry> {
    let mime = if crate::filesystem::valid_mime(hint) {
        hint.to_lowercase()
    } else {
        probe_mime(path)
            .as_str()
            .unwrap_or("application/octet-stream")
            .to_string()
    };
    let Some(gio) = which("gio") else {
        return Vec::new();
    };
    let output = CommandSpec::new(gio)
        .args(["mime", mime.as_str()])
        .env("LC_ALL", "C.UTF-8")
        .timeout(CONTROL_TIMEOUT)
        .limits(256 * 1024, 64 * 1024)
        .run();
    let Ok(output) = output else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    let mut registered = false;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let line = line.trim();
        if line.starts_with("Default application") && line.contains(':') {
            if let Some(value) = line
                .rsplit(':')
                .next()
                .filter(|value| value.ends_with(".desktop"))
            {
                ids.push(value.trim().to_string());
            }
            registered = false;
        } else if line == "Registered applications:" {
            registered = true;
        } else if line.ends_with("applications:") {
            registered = false;
        } else if registered && line.ends_with(".desktop") {
            ids.push(line.to_string());
        }
    }
    let mut seen = HashSet::new();
    ids.into_iter()
        .filter(|id| seen.insert(id.clone()))
        .filter_map(|id| {
            desktop_file_for(&id)
                .and_then(|path| parse_desktop_entry(&path))
                .or_else(|| {
                    let name = id.trim_end_matches(".desktop").to_string();
                    Some(DesktopEntry {
                        desktop_id: id.clone(),
                        name: name.clone(),
                        icon: name,
                        ..DesktopEntry::default()
                    })
                })
        })
        .collect()
}

pub(super) fn parse_desktop_entry(path: &Path) -> Option<DesktopEntry> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_DESKTOP_BYTES {
        return None;
    }
    let text = bounded_file_text(path);
    let mut in_entry = false;
    let mut values = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            values
                .entry(key.trim().to_string())
                .or_insert_with(|| value.trim().to_string());
        }
    }
    let desktop_id = path.file_name()?.to_str()?.to_string();
    let icon = values.remove("Icon").unwrap_or_default();
    Some(DesktopEntry {
        icon_mask: desktop_icon_mask(&desktop_id),
        desktop_id,
        name: values.remove("Name").unwrap_or_else(|| {
            path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        }),
        icon_source: desktop_icon_source(&icon),
        icon,
        executable: values.remove("Exec").unwrap_or_default(),
        wm_class: values.remove("StartupWMClass").unwrap_or_default(),
    })
}

fn desktop_icon_mask(desktop_id: &str) -> String {
    if desktop_id.to_ascii_lowercase().starts_with("dev.zed.zed") {
        "luminance".to_string()
    } else {
        String::new()
    }
}

fn desktop_icon_source(icon: &str) -> String {
    let roots = data_directories()
        .into_iter()
        .map(|directory| directory.join("icons"))
        .chain([PathBuf::from("/usr/share/pixmaps")])
        .collect::<Vec<_>>();
    trusted_icon_source(icon, &roots)
}

fn trusted_icon_source(icon: &str, roots: &[PathBuf]) -> String {
    let path = Path::new(icon);
    if !path.is_absolute() {
        return String::new();
    }
    let Ok(path) = fs::canonicalize(path) else {
        return String::new();
    };
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "png" | "webp" | "jpg" | "jpeg") {
        return String::new();
    }
    let Ok(metadata) = fs::metadata(&path) else {
        return String::new();
    };
    if !metadata.is_file() || metadata.len() > MAX_ICON_BYTES {
        return String::new();
    }
    let allowed = roots
        .iter()
        .any(|root| fs::canonicalize(root).is_ok_and(|root| path.starts_with(root)));
    if !allowed {
        return String::new();
    }
    let mut header = [0_u8; 12];
    let Ok(length) = File::open(&path).and_then(|mut file| file.read(&mut header)) else {
        return String::new();
    };
    let header = &header[..length];
    let raster = match extension.as_str() {
        "png" => header.starts_with(b"\x89PNG\r\n\x1a\n"),
        "jpg" | "jpeg" => header.starts_with(&[0xff, 0xd8, 0xff]),
        "webp" => header.starts_with(b"RIFF") && header.get(8..12) == Some(b"WEBP"),
        _ => false,
    };
    if !raster {
        return String::new();
    }
    Url::from_file_path(path)
        .ok()
        .map(|url| url.to_string())
        .unwrap_or_default()
}

pub(super) fn desktop_file_for(id: &str) -> Option<PathBuf> {
    if validate_desktop_id(id).is_err() {
        return None;
    }
    data_directories()
        .into_iter()
        .map(|directory| directory.join("applications").join(id))
        .find(|path| {
            fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file())
        })
}

pub(super) fn data_directories() -> Vec<PathBuf> {
    let mut result = vec![
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| expanded_path("~/.local/share")),
    ];
    result.extend(
        std::env::var("XDG_DATA_DIRS")
            .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string())
            .split(':')
            .map(PathBuf::from)
            .filter(|path| path.is_absolute()),
    );
    let zed = expanded_path("~/.local/zed.app/share");
    if zed.exists() {
        result.push(zed);
    }
    result
}
