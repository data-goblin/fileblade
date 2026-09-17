#![allow(dead_code)]

use fileblade::core_modules::skills::discovery::{self, Environment};
use fileblade::core_modules::watch::WatchPlan;
use serde_json::{Map, Value};
use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub const TEMPLATE: &str =
    "---\nname: {name}\ndescription: {description}\n---\n\nBody for {name}.\n";

pub fn descriptor_text(name: &str, description: &str) -> String {
    TEMPLATE
        .replace("{name}", name)
        .replace("{description}", description)
}

pub fn write_skill(root: &Path, name: &str, description: &str) -> PathBuf {
    write_skill_raw(root, OsStr::new(name), name, description)
}

pub fn write_skill_raw(
    root: &Path,
    directory_name: &OsStr,
    name: &str,
    description: &str,
) -> PathBuf {
    let directory = root.join(directory_name);
    fs::create_dir_all(&directory).unwrap();
    let descriptor = directory.join("SKILL.md");
    fs::write(&descriptor, descriptor_text(name, description)).unwrap();
    descriptor
}

pub fn link_skill(root: &Path, name: &str, target: &Path) -> PathBuf {
    fs::create_dir_all(root).unwrap();
    let link = root.join(name);
    if fs::symlink_metadata(&link).is_err() {
        std::os::unix::fs::symlink(target, &link).unwrap();
    }
    link
}

pub fn environment(home: &Path) -> Environment {
    Environment::new(home)
}

pub fn collect(environment: &Environment) -> Map<String, Value> {
    let mut plan = WatchPlan::new();
    discovery::collect(&mut plan, environment)
}

pub fn collect_watched(environment: &Environment) -> (Map<String, Value>, Vec<PathBuf>, bool) {
    let mut plan = WatchPlan::new();
    let mut document = discovery::collect(&mut plan, environment);
    let paths = plan.paths();
    plan.finish(&mut document);
    let truncated = document
        .get("watchTruncated")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    (document, paths, truncated)
}

pub fn rows(document: &Map<String, Value>) -> Vec<&Value> {
    document
        .get("items")
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

pub fn names(document: &Map<String, Value>) -> Vec<String> {
    rows(document)
        .into_iter()
        .filter_map(|row| row.get("name").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

pub fn row_for<'a>(document: &'a Map<String, Value>, name: &str) -> Option<&'a Value> {
    rows(document)
        .into_iter()
        .find(|row| row.get("name").and_then(Value::as_str) == Some(name))
}

pub fn row_id(environment: &Environment, name: &str) -> String {
    row_for(&collect(environment), name)
        .and_then(|row| row.get("id"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

pub fn agents_of(environment: &Environment, name: &str) -> Vec<String> {
    row_for(&collect(environment), name)
        .and_then(|row| row.get("agents"))
        .and_then(Value::as_array)
        .map(|agents| {
            agents
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub fn field<'a>(row: &'a Value, key: &str) -> &'a str {
    row.get(key).and_then(Value::as_str).unwrap_or_default()
}

pub fn native_name(byte: u8) -> std::ffi::OsString {
    let mut bytes = b"native-".to_vec();
    bytes.push(byte);
    OsStr::from_bytes(&bytes).to_os_string()
}
