use super::{image, read_bounded};
use crate::{AppError, AppResult};
use regex::Regex;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const SOCKET: &str = "data-goblin.fileblade/blade";
const MANIFEST_LIMIT: u64 = 1024 * 1024;
const SOURCE_LIMIT: u64 = 1024 * 1024;
const SETTING_TYPES: [&str; 6] = ["string", "integer", "number", "boolean", "enum", "path"];
const CHECKS: [&str; 5] = [
    "manifest identity",
    "blade modules",
    "module settings",
    "host guard",
    "banner",
];

fn plugin_id() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^[a-z0-9_-]+\.[a-z0-9_-]+$").unwrap())
}

fn module_id() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$").unwrap())
}

fn setting_key() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_]{0,63}$").unwrap())
}

fn check(condition: bool, message: &str) -> AppResult<()> {
    if condition {
        Ok(())
    } else {
        Err(AppError::invalid(message.to_string()))
    }
}

fn manifest(root: &Path) -> AppResult<Value> {
    let bytes = read_bounded(&root.join("manifest.json"), MANIFEST_LIMIT)?;
    serde_json::from_slice(&bytes).map_err(|error| {
        AppError::invalid(format!(
            "cannot read {}: {error}",
            root.join("manifest.json").display()
        ))
    })
}

fn is_file(root: &Path, relative: &str) -> bool {
    root.join(relative).is_file()
}

fn manifest_identity(root: &Path, document: &Value) -> AppResult<()> {
    check(document["schemaVersion"] == 1, "schemaVersion must be 1")?;
    check(
        plugin_id().is_match(document["id"].as_str().unwrap_or_default()),
        "id must be publisher.name in lowercase",
    )?;
    check(
        !document["name"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .is_empty(),
        "name is required",
    )?;
    check(
        document["version"] == "0.1.0",
        "stay at 0.1.0 until the first release",
    )?;
    check(
        document["kinds"]
            .as_array()
            .is_some_and(|kinds| kinds.iter().any(|kind| kind == "service")),
        "kinds must include service",
    )?;
    let service = document["entryPoints"]["service"]
        .as_str()
        .unwrap_or_default();
    check(
        !service.is_empty() && is_file(root, service),
        "the service entry point must exist",
    )
}

fn blade_modules(root: &Path, document: &Value) -> AppResult<Vec<Value>> {
    let modules = document["extensions"][SOCKET].as_array().cloned();
    let modules = match modules {
        Some(modules) if (1..=128).contains(&modules.len()) => modules,
        _ => {
            return Err(AppError::invalid(
                "declare at least one blade module".to_string(),
            ));
        }
    };
    for module in &modules {
        check(
            module_id().is_match(module["id"].as_str().unwrap_or_default()),
            "module id is invalid",
        )?;
        let entry = module["entry"].as_str().unwrap_or("Module.qml");
        check(
            !entry.starts_with('/') && !entry.split('/').any(|part| part == ".."),
            "entry must be a safe relative path",
        )?;
        check(is_file(root, entry), &format!("entry {entry} must exist"))?;
        let contract = module
            .get("hostContract")
            .cloned()
            .unwrap_or(Value::from(1));
        check(
            contract == 1 || contract == 2,
            "hostContract must be 1 or 2",
        )?;
        let height = module
            .get("minHeight")
            .map_or(Some(0), Value::as_i64)
            .ok_or_else(|| AppError::invalid("minHeight must be 0 to 4096".to_string()))?;
        check((0..=4096).contains(&height), "minHeight must be 0 to 4096")?;
    }
    Ok(modules)
}

fn module_settings(modules: &[Value]) -> AppResult<()> {
    for module in modules {
        let settings = match module.get("settings") {
            Some(Value::Null) | None => continue,
            Some(settings) => settings,
        };
        let schema = settings["schema"].as_array().cloned().unwrap_or_default();
        let keys: Vec<&str> = schema
            .iter()
            .map(|row| row["key"].as_str().unwrap_or_default())
            .collect();
        let mut unique = keys.clone();
        unique.sort_unstable();
        unique.dedup();
        check(
            unique.len() == keys.len() && keys.len() <= 32,
            "setting keys must be unique and at most 32",
        )?;
        for row in &schema {
            check(
                setting_key().is_match(row["key"].as_str().unwrap_or_default()),
                "setting key is invalid",
            )?;
            let kind = row["type"].as_str().unwrap_or_default();
            check(
                SETTING_TYPES.contains(&kind),
                &format!("unknown setting type {kind}"),
            )?;
        }
        for key in settings["defaults"]
            .as_object()
            .into_iter()
            .flat_map(|defaults| defaults.keys())
        {
            check(
                keys.contains(&key.as_str()),
                &format!("default {key} has no schema row"),
            )?;
        }
    }
    Ok(())
}

fn host_guard(root: &Path) -> AppResult<()> {
    let bytes = read_bounded(&root.join("HostGuard.js"), SOURCE_LIMIT)?;
    let source = String::from_utf8_lossy(&bytes).to_string();
    check(
        source.contains(&format!("var HOST_ID = \"{}\"", super::HOST_ID)),
        "HostGuard.js must name the FileBlade host",
    )?;
    check(
        source.contains("https://github.com/data-goblin/fileblade"),
        "HostGuard.js must identify the FileBlade repository",
    )?;
    check(
        !source.contains("omarchy plugin add"),
        "the host guard must never install FileBlade",
    )?;
    check(
        is_file(root, "assets/fileblade-logo.png"),
        "the host guard needs assets/fileblade-logo.png",
    )
}

fn banner(document: &Value) -> AppResult<()> {
    let name = document["name"].as_str().unwrap_or_default().trim();
    let name = name.strip_prefix(image::NAME_PREFIX).unwrap_or(name);
    let svg = image::banner_svg(name, None)?;
    check(svg.contains("viewBox=\"0 0 960 272\""), "banner viewBox")?;
    check(
        svg.contains(&format!("id=\"{}\"", image::BANNER_MASK)),
        "banner mask id",
    )?;
    check(
        svg.contains(&format!("fill=\"{}\"", image::AMBER)),
        "banner subtitle colour",
    )?;
    check(
        svg.contains(&format!(
            "aria-label=\"FileBlade {} extension\"",
            image::escape(&name.to_lowercase())
        )),
        "banner label",
    )
}

pub fn run(directory: &Path) -> AppResult<Vec<&'static str>> {
    let root = absolute(directory)?;
    check(
        root.is_dir(),
        &format!("{} is not a directory", root.display()),
    )?;
    let document = manifest(&root)?;
    manifest_identity(&root, &document)?;
    let modules = blade_modules(&root, &document)?;
    module_settings(&modules)?;
    host_guard(&root)?;
    banner(&document)?;
    Ok(CHECKS.to_vec())
}

fn absolute(directory: &Path) -> AppResult<PathBuf> {
    if directory.is_absolute() {
        Ok(directory.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(directory))
    }
}
