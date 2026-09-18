use crate::command::{CommandSpec, which};
use serde::de::{MapAccess, Visitor};
use serde_json::Value;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const PROGRAM: &str = "sqlite3";
pub const TIMEOUT: Duration = Duration::from_secs(15);
pub const BUSY_MILLISECONDS: u64 = 5000;
pub const STDOUT_LIMIT: usize = 32 * 1024 * 1024;
pub const STDERR_LIMIT: usize = 64 * 1024;

#[derive(Debug)]
pub struct Error {
    message: String,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::new(error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug)]
pub enum Bound {
    Null,
    Integer(i64),
    Text(String),
    Blob(Vec<u8>),
}

impl From<i64> for Bound {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<&str> for Bound {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<String> for Bound {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<Vec<u8>> for Bound {
    fn from(value: Vec<u8>) -> Self {
        Self::Blob(value)
    }
}

impl From<Option<i64>> for Bound {
    fn from(value: Option<i64>) -> Self {
        value.map_or(Self::Null, Self::Integer)
    }
}

impl From<Option<&str>> for Bound {
    fn from(value: Option<&str>) -> Self {
        value.map_or(Self::Null, |value| Self::Text(value.to_string()))
    }
}

impl From<Option<String>> for Bound {
    fn from(value: Option<String>) -> Self {
        value.map_or(Self::Null, Self::Text)
    }
}

pub fn literal(value: &Bound) -> String {
    match value {
        Bound::Null => "NULL".to_string(),
        Bound::Integer(number) => number.to_string(),
        Bound::Text(text) => {
            if text.contains('\0') {
                let mut out = String::with_capacity(text.len() * 2 + 16);
                out.push_str("CAST(X'");
                for byte in text.as_bytes() {
                    out.push_str(&format!("{byte:02x}"));
                }
                out.push_str("' AS TEXT)");
                return out;
            }
            let mut out = String::with_capacity(text.len() + 2);
            out.push('\'');
            for character in text.chars() {
                if character == '\'' {
                    out.push('\'');
                }
                out.push(character);
            }
            out.push('\'');
            out
        }
        Bound::Blob(bytes) => {
            let mut out = String::with_capacity(bytes.len() * 2 + 3);
            out.push_str("X'");
            for byte in bytes {
                out.push_str(&format!("{byte:02x}"));
            }
            out.push('\'');
            out
        }
    }
}

pub fn bind(statement: &str, values: &[Bound]) -> Result<String> {
    let mut out = String::with_capacity(statement.len());
    let mut taken = 0usize;
    let mut quoted = false;
    for character in statement.chars() {
        if character == '\'' {
            quoted = !quoted;
            out.push(character);
            continue;
        }
        if character != '?' || quoted {
            out.push(character);
            continue;
        }
        let Some(value) = values.get(taken) else {
            return Err(Error::new("usage statement wants more values than given"));
        };
        out.push_str(&literal(value));
        taken += 1;
    }
    if taken != values.len() {
        return Err(Error::new("usage statement leaves values unused"));
    }
    Ok(out)
}

#[derive(Clone, Debug, Default)]
pub struct Row {
    names: Vec<String>,
    values: Vec<Value>,
}

impl Row {
    pub fn name(&self, index: usize) -> &str {
        self.names.get(index).map_or("", String::as_str)
    }

    pub fn width(&self) -> usize {
        self.values.len()
    }

    pub fn value(&self, index: usize) -> &Value {
        self.values.get(index).unwrap_or(&Value::Null)
    }

    pub fn integer(&self, index: usize) -> i64 {
        self.optional_integer(index).unwrap_or(0)
    }

    pub fn optional_integer(&self, index: usize) -> Option<i64> {
        match self.value(index) {
            Value::Number(number) => number
                .as_i64()
                .or_else(|| number.as_f64().map(|number| number as i64)),
            Value::String(text) => text.parse().ok(),
            _ => None,
        }
    }

    pub fn text(&self, index: usize) -> String {
        self.optional_text(index).unwrap_or_default()
    }

    pub fn optional_text(&self, index: usize) -> Option<String> {
        match self.value(index) {
            Value::Null => None,
            Value::String(text) => Some(text.clone()),
            other => Some(other.to_string()),
        }
    }

    pub fn text_bytes(&self, index: usize) -> String {
        self.optional_text_bytes(index).unwrap_or_default()
    }

    pub fn optional_text_bytes(&self, index: usize) -> Option<String> {
        match self.value(index) {
            Value::Null => None,
            Value::String(_) => Some(String::from_utf8_lossy(&self.blob(index)).into_owned()),
            other => Some(other.to_string()),
        }
    }

    pub fn blob(&self, index: usize) -> Vec<u8> {
        match self.value(index) {
            Value::String(text) => text
                .chars()
                .map(|character| (character as u32 & 0xff) as u8)
                .collect(),
            _ => Vec::new(),
        }
    }
}

impl<'de> serde::Deserialize<'de> for Row {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Shape;

        impl<'de> Visitor<'de> for Shape {
            type Value = Row;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a sqlite result row")
            }

            fn visit_map<A>(self, mut entries: A) -> std::result::Result<Row, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut names = Vec::new();
                let mut values = Vec::new();
                while let Some((name, value)) = entries.next_entry::<String, Value>()? {
                    names.push(name);
                    values.push(value);
                }
                Ok(Row { names, values })
            }
        }

        deserializer.deserialize_map(Shape)
    }
}

#[derive(Clone)]
pub struct Sql {
    program: PathBuf,
    path: PathBuf,
    readonly: bool,
    timeout: Duration,
    busy: u64,
}

impl Sql {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        Self::locate(path, false)
    }

    pub fn open_readonly(path: impl Into<PathBuf>) -> Result<Self> {
        Self::locate(path, true)
    }

    fn locate(path: impl Into<PathBuf>, readonly: bool) -> Result<Self> {
        let program = which(PROGRAM).ok_or_else(|| Error::new("sqlite3 is not installed"))?;
        Ok(Self {
            program,
            path: path.into(),
            readonly,
            timeout: TIMEOUT,
            busy: BUSY_MILLISECONDS,
        })
    }

    pub fn with_timeout(mut self, value: Duration) -> Self {
        self.timeout = value.max(Duration::from_secs(1));
        self
    }

    pub fn with_busy(mut self, milliseconds: u64) -> Self {
        self.busy = milliseconds;
        self
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn execute(&self, script: &str) -> Result<()> {
        self.invoke(script).map(|_| ())
    }

    pub fn query(&self, script: &str) -> Result<Vec<Row>> {
        let stdout = self.invoke(script)?;
        let text = std::str::from_utf8(&stdout)
            .map_err(|_| Error::new("usage store returned invalid text"))?;
        let mut last: Vec<Row> = Vec::new();
        for set in serde_json::Deserializer::from_str(text).into_iter::<Vec<Row>>() {
            last = set.map_err(|error| Error::new(format!("usage store answer: {error}")))?;
        }
        Ok(last)
    }

    pub fn query_one(&self, script: &str) -> Result<Option<Row>> {
        Ok(self.query(script)?.into_iter().next())
    }

    fn invoke(&self, script: &str) -> Result<Vec<u8>> {
        let mut arguments: Vec<&str> = vec!["-batch", "-bail", "-json"];
        if self.readonly {
            arguments.push("-readonly");
        }
        let body = format!(".timeout {}\n{script}\n", self.busy);
        let path = self.path.as_os_str();
        let spec = CommandSpec::new(&self.program)
            .args(arguments)
            .args([path])
            .timeout(self.timeout)
            .limits(STDOUT_LIMIT, STDERR_LIMIT)
            .stdin(body.into_bytes());
        let output = spec
            .run()
            .map_err(|error| Error::new(format!("usage store: {error}")))?;
        if !output.status.success() {
            let message = String::from_utf8_lossy(&output.stderr);
            let message = message.trim();
            return Err(Error::new(if message.is_empty() {
                "usage store refused the statement".to_string()
            } else {
                message.to_string()
            }));
        }
        if output.stdout_truncated {
            return Err(Error::new("usage store answer is too large"));
        }
        Ok(output.stdout)
    }
}
