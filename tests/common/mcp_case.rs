#![allow(dead_code)]

use fileblade::core_modules::mcp::inventory::{Environ, Inventory, Settings};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub const SENTINEL: &str = "SECRET_SENTINEL_DO_NOT_EMIT";

pub struct Case {
    pub _temporary: tempfile::TempDir,
    pub base: PathBuf,
    pub home: PathBuf,
    pub project: PathBuf,
    pub config: PathBuf,
    pub etc: PathBuf,
    pub codex_home: Option<PathBuf>,
    pub environment: Environ,
    pub scope: String,
}

pub fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

pub fn json_write(path: &Path, value: &Value) {
    write(path, &value.to_string());
}

impl Case {
    pub fn new() -> Self {
        let temporary = tempfile::Builder::new()
            .prefix("fileblade-mcp-")
            .tempdir()
            .unwrap();
        let base = fs::canonicalize(temporary.path()).unwrap();
        let home = base.join("home");
        let project = base.join("workspace").join("project");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&home).unwrap();
        Self {
            _temporary: temporary,
            config: home.join(".config"),
            etc: base.join("etc"),
            base,
            home,
            project,
            codex_home: None,
            environment: Environ::new(),
            scope: "all".to_string(),
        }
    }

    pub fn settings(&self) -> Settings {
        Settings {
            project: self.project.clone(),
            home: self.home.clone(),
            config_home: self.config.clone(),
            etc_root: self.etc.clone(),
            codex_home: self
                .codex_home
                .clone()
                .unwrap_or_else(|| self.home.join(".codex")),
            system_owner_uid: rustix::process::getuid().as_raw(),
            scope: self.scope.clone(),
            environment: self.environment.clone(),
        }
    }

    pub fn inventory(&self) -> Inventory {
        Inventory::new(self.settings())
    }

    pub fn scan(&self) -> serde_json::Map<String, Value> {
        self.inventory().scan()
    }

    pub fn definitions(&self) -> Vec<Value> {
        self.scan()
            .get("definitions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    }
}

pub fn field<'a>(row: &'a Value, key: &str) -> &'a Value {
    row.get(key).unwrap_or(&Value::Null)
}

pub fn text<'a>(row: &'a Value, key: &str) -> &'a str {
    field(row, key).as_str().unwrap_or_default()
}

pub fn named<'a>(rows: &'a [Value], agent: &str, name: &str) -> Vec<&'a Value> {
    rows.iter()
        .filter(|row| text(row, "agent") == agent && text(row, "name") == name)
        .collect()
}

pub static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|error| error.into_inner())
}
