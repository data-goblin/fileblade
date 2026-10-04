use crate::actions::{MAX_MANIFEST_BYTES, plugin_root, read_manifest};
use crate::command::{CommandSpec, which};
use crate::common::path_text;
use crate::filesystem::read_regular_file;
use crate::paths::xdg_home;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

const BLADE_SOCKET: &str = "data-goblin.fileblade/blade";
const HELPER_SOCKET: &str = "data-goblin.fileblade/helper";
const ACTION_SOCKET: &str = "data-goblin.fileblade/action";
const MAX_ENTRY_BYTES: usize = 512;
const MAX_ENTRY_SEGMENTS: usize = 8;
const CORE_ID: &str = "data-goblin.fileblade";
const MAX_ENTRIES: usize = 256;
const MAX_PROVIDERS: usize = 128;
const MAX_MANIFEST_TOTAL_BYTES: usize = 4 * 1024 * 1024;
const MAX_DIAGNOSTICS: usize = 32;
const MAX_ROWS: usize = 512;
const CLI_TIMEOUT: Duration = Duration::from_secs(2);
const CLI_STDOUT_LIMIT: usize = 128 * 1024;
const CLI_STDERR_LIMIT: usize = 4 * 1024;

pub(crate) fn legacy_activation(config_root: &std::path::Path) -> Result<Vec<String>, String> {
    let config_home = config_root
        .parent()
        .and_then(std::path::Path::parent)
        .ok_or("legacy config root has no XDG parent")?;
    let (activation, state) = enabled_ids_for(config_home);
    if state != "known" {
        return Err("legacy plugin activation is unknown".into());
    }
    let enabled: Vec<String> = activation
        .into_iter()
        .filter(|(id, enabled)| {
            *enabled
                && (id == CORE_ID
                    || crate::module_helpers::canonical_route(id, "inventory").is_some()
                        && !id.starts_with("fileblade.core."))
        })
        .map(|(id, _)| id)
        .collect();
    if enabled.is_empty() {
        let path = config_home.join("omarchy/shell.json");
        let bytes = read_regular_file(&path, SHELL_CONFIG_LIMIT)
            .map_err(|_| "legacy shell activation configuration is unavailable")?;
        let config: Value = serde_json::from_slice(&bytes)
            .map_err(|_| "legacy shell activation configuration is malformed")?;
        if !config.is_object()
            || !config["plugins"].is_array()
            || config["plugins"]
                .as_array()
                .is_some_and(|rows| rows.iter().any(|row| !shell_plugin_row(row)))
            || config.get("disabledPlugins").is_some_and(|value| {
                !value.is_array()
                    || value
                        .as_array()
                        .is_some_and(|rows| rows.iter().any(|id| !id.is_string()))
            })
        {
            return Err("legacy shell activation configuration is malformed".into());
        }
        let shell = ShellActivation::parse(&config);
        if shell.listed.iter().any(|id| {
            shell.enabled(id)
                && (id == CORE_ID
                    || crate::module_helpers::canonical_route(id, "inventory").is_some()
                        && !id.starts_with("fileblade.core."))
        }) {
            return Err("legacy CLI and shell activation evidence disagree".into());
        }
    }
    Ok(enabled)
}

pub fn catalog() -> crate::AppResult<Value> {
    let mut diagnostics: Vec<Value> = Vec::new();
    let (plugins, native) = match plugins_dir() {
        Ok(selected) => selected,
        Err(error) => {
            return Ok(json!({
                "ok": true,
                "providers": [],
                "activation": "unknown",
                "truncated": false,
                "diagnostics": [json!({"source": "plugins", "error": bounded(&error, 256)})],
            }));
        }
    };
    let mut providers = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut ambiguous: HashSet<String> = HashSet::new();
    let mut bytes = 0usize;
    let mut truncated = false;
    let mut entries: Vec<PathBuf> = Vec::new();
    match std::fs::read_dir(&plugins) {
        Ok(reader) => {
            for entry in reader.take(MAX_ENTRIES + 1) {
                if entries.len() >= MAX_ENTRIES {
                    truncated = true;
                    break;
                }
                match entry {
                    Ok(entry) => entries.push(entry.path()),
                    Err(error) => {
                        truncated = true;
                        note(&mut diagnostics, "plugins", &error.to_string());
                    }
                }
            }
        }
        Err(error) if native && error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            truncated = true;
            note(&mut diagnostics, &path_text(&plugins), &error.to_string());
        }
    }
    entries.sort();
    let directory_names: Vec<_> = entries
        .iter()
        .filter(|path| path.is_dir())
        .filter_map(|path| path.file_name()?.to_str())
        .filter(|name| !name.starts_with('.') && *name != CORE_ID)
        .collect();
    for path in &entries {
        if providers.len() >= MAX_PROVIDERS || bytes >= MAX_MANIFEST_TOTAL_BYTES {
            truncated = true;
            break;
        }
        let name = match path.file_name().and_then(|value| value.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };
        if name.starts_with('.') || name == CORE_ID {
            continue;
        }
        if !path.is_dir() {
            continue;
        }
        let root = match plugin_root(&path.to_string_lossy()) {
            Ok(root) => root,
            Err(error) => {
                note(&mut diagnostics, &name, &error);
                continue;
            }
        };
        let manifest_path = root.join("manifest.json");
        let declared = match std::fs::symlink_metadata(&manifest_path) {
            Ok(metadata) if metadata.file_type().is_file() => metadata.len() as usize,
            Ok(_) => {
                note(
                    &mut diagnostics,
                    &name,
                    "manifest.json is not a regular file",
                );
                continue;
            }
            Err(_) => continue,
        };
        let readable = declared.min(MAX_MANIFEST_BYTES);
        if bytes.saturating_add(readable) > MAX_MANIFEST_TOTAL_BYTES {
            truncated = true;
            break;
        }
        bytes = bytes.saturating_add(readable);
        let manifest = match read_manifest(&root) {
            Ok(manifest) => manifest,
            Err(error) => {
                note(&mut diagnostics, &name, &error);
                continue;
            }
        };
        let id = manifest
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if id.is_empty() || id != name || id == CORE_ID || id.starts_with("omarchy.") {
            if !id.is_empty() && id != name {
                note(
                    &mut diagnostics,
                    &name,
                    "the manifest id does not match its installed directory",
                );
            }
            continue;
        }
        if !contributes(&manifest) {
            continue;
        }
        if let Err(error) = entries_are_confined(&manifest, &root) {
            note(&mut diagnostics, &id, &error);
            continue;
        }
        if !seen.insert(id.clone()) {
            ambiguous.insert(id.clone());
            note(&mut diagnostics, &id, "two installed plugins carry this id");
            continue;
        }
        providers.push(json!({
            "id": id,
            "dir": path_text(&root),
            "path": path_text(path),
            "manifest": manifest,
            "enabled": false,
        }));
    }
    let mut providers: Vec<Value> = providers
        .into_iter()
        .filter(|provider| {
            let id = provider
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            !ambiguous.contains(id)
        })
        .collect();
    let (activation, activation_state) = if native && truncated {
        (BTreeMap::new(), "unknown")
    } else {
        enabled_ids(native, &providers)
    };
    for provider in &mut providers {
        provider["enabled"] = json!(
            activation
                .get(provider["id"].as_str().unwrap())
                .copied()
                .unwrap_or(false)
        );
    }
    Ok(json!({
        "ok": true,
        "providers": providers,
        "directory_names": directory_names,
        "activation": if truncated { "unknown" } else { activation_state },
        "complete": !truncated,
        "truncated": truncated,
        "diagnostics": diagnostics,
    }))
}

fn contributes(manifest: &Value) -> bool {
    let extensions = match manifest.get("extensions") {
        Some(Value::Object(extensions)) => extensions,
        _ => return false,
    };
    [BLADE_SOCKET, HELPER_SOCKET, ACTION_SOCKET]
        .iter()
        .any(|socket| matches!(extensions.get(*socket), Some(Value::Array(entries)) if !entries.is_empty()))
}

fn declared_entries(manifest: &Value) -> Vec<String> {
    let mut found = Vec::new();
    let extensions = match manifest.get("extensions") {
        Some(Value::Object(extensions)) => extensions,
        _ => return found,
    };
    for socket in [BLADE_SOCKET, HELPER_SOCKET, ACTION_SOCKET] {
        let Some(Value::Array(entries)) = extensions.get(socket) else {
            continue;
        };
        for entry in entries.iter().take(MAX_PROVIDERS) {
            for key in ["entry", "provider"] {
                match entry.get(key) {
                    Some(Value::String(value)) => found.push(value.clone()),
                    Some(Value::Null) | None => {}
                    Some(_) => found.push(String::new()),
                }
            }
        }
    }
    found
}

fn entries_are_confined(manifest: &Value, root: &std::path::Path) -> Result<(), String> {
    let canonical = std::fs::canonicalize(root).map_err(|error| error.to_string())?;
    for entry in declared_entries(manifest) {
        if entry.is_empty()
            || entry.len() > MAX_ENTRY_BYTES
            || entry.starts_with('/')
            || entry.chars().any(char::is_control)
        {
            return Err(format!("{entry:?} is not a usable entry path"));
        }
        let relative = std::path::Path::new(&entry);
        let segments = relative.components().count();
        if segments == 0 || segments > MAX_ENTRY_SEGMENTS {
            return Err(format!("{entry:?} is not a usable entry path"));
        }
        if relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err(format!("{entry:?} leaves the plugin directory"));
        }
        let target = canonical.join(relative);
        match std::fs::symlink_metadata(&target) {
            Ok(metadata) if metadata.file_type().is_file() => {}
            Ok(_) => return Err(format!("{entry:?} is not a regular file")),
            Err(error) => return Err(format!("{}: {error}", path_text(&target))),
        }
        let resolved = std::fs::canonicalize(&target)
            .map_err(|error| format!("{}: {error}", path_text(&target)))?;
        if !resolved.starts_with(&canonical) {
            return Err(format!("{entry:?} leaves the plugin directory"));
        }
    }
    Ok(())
}

pub fn native_extensions_selected() -> Result<bool, String> {
    if std::env::var_os("FILEBLADE_APP_ROOT").is_some_and(|value| !value.is_empty()) {
        crate::paths::app_root().map_err(|error| error.to_string())?;
        return Ok(true);
    }
    installed_native_selected()
}

pub fn installed_native_selected() -> Result<bool, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    receipt_selects_native(
        &executable,
        &xdg_home("XDG_DATA_HOME", "~/.local/share").join("fileblade/installation"),
    )
}

fn receipt_selects_native(executable: &Path, installation: &Path) -> Result<bool, String> {
    if !executable.starts_with(installation.join("versions")) {
        return Ok(false);
    }
    let active =
        std::fs::read_link(installation.join("active")).map_err(|error| error.to_string())?;
    let generation = active
        .to_str()
        .and_then(|path| path.strip_prefix("generations/generation."));
    if !generation.is_some_and(|name| {
        !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_alphanumeric())
    }) {
        return Err("native installation activation pointer is invalid".into());
    }
    let receipt_path = installation.join("active/receipt.json");
    let bytes = read_regular_file(&receipt_path, 16384).map_err(|error| error.to_string())?;
    let receipt: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let payload = receipt["payload"].as_str().unwrap_or_default();
    if receipt["schema"] != 1
        || receipt["owner"] != "direct"
        || receipt["installation"] != path_text(installation)
        || payload.len() != 64
        || !payload
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || !executable.starts_with(installation.join("versions").join(payload))
        || std::fs::read_link(installation.join("active/runtime")).ok()
            != Some(PathBuf::from(format!("../../versions/{payload}")))
    {
        return Err("native installation receipt does not name the running payload".into());
    }
    Ok(true)
}

fn plugins_dir() -> Result<(PathBuf, bool), String> {
    let native = native_extensions_selected()?;
    if native {
        return Ok((crate::lease::native_extension_root(), true));
    }
    let home = std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .ok_or("HOME is not set")?;
    let plugins = PathBuf::from(home).join(".config/omarchy/plugins");
    std::fs::canonicalize(&plugins)
        .map(|path| (path, false))
        .map_err(|error| format!("{}: {error}", path_text(&plugins)))
}

fn enabled_ids(native: bool, providers: &[Value]) -> (BTreeMap<String, bool>, &'static str) {
    if native {
        return match native_enabled_ids(providers) {
            Ok(activation) => (activation, "known"),
            Err(_) => (BTreeMap::new(), "unknown"),
        };
    }
    enabled_ids_for(&xdg_home("XDG_CONFIG_HOME", "~/.config"))
}

fn native_enabled_ids(providers: &[Value]) -> crate::AppResult<BTreeMap<String, bool>> {
    let mut settings = crate::preferences::read_document()?;
    let (activation, changed) = native_activation(&mut settings, providers)?;
    if !changed {
        return Ok(activation);
    }
    let path = crate::lease::native_config_root().join("settings.json");
    crate::lease::persistence::check_write(&path)?;
    let _directory = crate::secure::ensure_private_directory(path.parent().unwrap())?;
    let _lock =
        crate::secure::try_open_private_lock(&crate::paths::state_dir().join("preferences.lock"))?
            .ok_or_else(|| crate::AppError::invalid("preferences are busy; retry discovery"))?;
    let mut settings = crate::preferences::read_document()?;
    let (activation, changed) = native_activation(&mut settings, providers)?;
    if changed {
        let encoded = serde_json::to_vec_pretty(&settings)?;
        if encoded.len() > 64 * 1024 {
            return Err(crate::AppError::invalid(
                "extension receipts exceed settings capacity",
            ));
        }
        crate::lease::durable::write_private_atomic(&path, &encoded)?;
    }
    Ok(activation)
}

fn native_activation(
    settings: &mut Value,
    providers: &[Value],
) -> crate::AppResult<(BTreeMap<String, bool>, bool)> {
    let mut activation = BTreeMap::new();
    if providers.is_empty() {
        return Ok((activation, false));
    }
    let entries = settings
        .as_object_mut()
        .unwrap()
        .entry("extensions")
        .or_insert(json!({}))
        .as_object_mut()
        .ok_or_else(|| crate::AppError::invalid("extension activation is malformed"))?;
    let mut changed = false;
    for provider in providers {
        let id = provider["id"].as_str().unwrap();
        if !entries.contains_key(id) {
            entries.insert(
                id.to_string(),
                json!({"enabled":true,"receipt":{"version":1,"source":provider["dir"]}}),
            );
            changed = true;
        }
        let enabled = entries[id]["enabled"]
            .as_bool()
            .ok_or_else(|| crate::AppError::invalid("extension enabled choice is not boolean"))?;
        activation.insert(id.to_string(), enabled);
    }
    Ok((activation, changed))
}

fn enabled_ids_for(config_home: &std::path::Path) -> (BTreeMap<String, bool>, &'static str) {
    let program = match which("omarchy") {
        Some(program) => program,
        None => return (BTreeMap::new(), "unknown"),
    };
    let output = CommandSpec::new(program)
        .args(["plugin", "list", "--json"])
        .env("XDG_CONFIG_HOME", config_home)
        .env("LC_ALL", "C")
        .timeout(CLI_TIMEOUT)
        .limits(CLI_STDOUT_LIMIT, CLI_STDERR_LIMIT)
        .run();
    let output = match output {
        Ok(output) if output.status.success() && !output.stdout_truncated => output,
        _ => return (BTreeMap::new(), "unknown"),
    };
    let rows = match serde_json::from_slice::<Value>(&output.stdout) {
        Ok(Value::Array(rows)) => rows,
        _ => return (BTreeMap::new(), "unknown"),
    };
    if rows.len() > MAX_ROWS {
        return (BTreeMap::new(), "unknown");
    }
    match merge_activation(&rows, shell_activation(config_home).as_ref()) {
        Some(activation) => (activation, "known"),
        None => (BTreeMap::new(), "unknown"),
    }
}

const SHELL_CONFIG_LIMIT: usize = 1024 * 1024;
const BAR_SECTIONS: [&str; 3] = ["left", "center", "right"];

struct ShellActivation {
    listed: HashSet<String>,
    disabled: HashSet<String>,
}

impl ShellActivation {
    fn parse(config: &Value) -> Self {
        let mut listed = HashSet::new();
        let mut disabled = HashSet::new();
        for row in config["plugins"].as_array().into_iter().flatten() {
            if let Some(id) = row["id"].as_str() {
                listed.insert(id.to_string());
            }
        }
        for section in BAR_SECTIONS {
            for row in config["bar"]["layout"][section]
                .as_array()
                .into_iter()
                .flatten()
            {
                if let Some(id) = row.as_str().or_else(|| row["id"].as_str())
                    && !id.is_empty()
                {
                    listed.insert(id.to_string());
                }
            }
        }
        for id in config["disabledPlugins"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            disabled.insert(id.to_string());
        }
        Self { listed, disabled }
    }

    fn enabled(&self, id: &str) -> bool {
        !self.disabled.contains(id) && self.listed.contains(id)
    }
}

fn shell_plugin_row(row: &Value) -> bool {
    match row {
        Value::String(id) => !id.is_empty(),
        Value::Object(fields) => fields.get("id").is_some_and(Value::is_string),
        _ => false,
    }
}

fn shell_activation(config_home: &std::path::Path) -> Option<ShellActivation> {
    let path = config_home.join("omarchy/shell.json");
    let bytes = read_regular_file(&path, SHELL_CONFIG_LIMIT).ok()?;
    let config: Value = serde_json::from_slice(&bytes).ok()?;
    config.is_object().then(|| ShellActivation::parse(&config))
}

fn listed_by_bar_only(row: &Value) -> bool {
    let kinds: Vec<&str> = row["kinds"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    kinds.contains(&"bar-widget") && kinds.iter().any(|kind| *kind != "bar-widget")
}

fn merge_activation(
    rows: &[Value],
    shell: Option<&ShellActivation>,
) -> Option<BTreeMap<String, bool>> {
    let mut activation = BTreeMap::new();
    for row in rows {
        let id = row.get("id").and_then(Value::as_str).unwrap_or_default();
        let enabled = row.get("enabled").and_then(Value::as_bool);
        let (false, Some(listed)) = (id.is_empty(), enabled) else {
            return None;
        };
        let enabled =
            listed || (listed_by_bar_only(row) && shell.is_some_and(|config| config.enabled(id)));
        if activation.insert(id.to_string(), enabled).is_some() {
            return None;
        }
    }
    Some(activation)
}

fn note(diagnostics: &mut Vec<Value>, source: &str, error: &str) {
    if diagnostics.len() >= MAX_DIAGNOSTICS {
        return;
    }
    diagnostics.push(json!({
        "source": bounded(source, 128),
        "error": bounded(error, 256),
    }));
}

fn bounded(value: &str, limit: usize) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect()
}

pub fn require_enabled(provider: &str, directory: &std::path::Path) -> crate::AppResult<()> {
    let snapshot = catalog()?;
    let enabled = snapshot["complete"] == true
        && snapshot["activation"] == "known"
        && snapshot["providers"].as_array().is_some_and(|rows| {
            rows.iter().any(|row| {
                row["id"] == provider
                    && row["enabled"] == true
                    && row["dir"]
                        .as_str()
                        .and_then(|path| crate::common::parse_path(path).ok())
                        .is_some_and(|path| path == directory)
            })
        });
    if enabled {
        Ok(())
    } else {
        Err(crate::AppError::invalid(
            "the helper provider must be installed and explicitly enabled; enable its Omarchy plugin and retry",
        ))
    }
}
