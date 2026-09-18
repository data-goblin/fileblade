use crate::{AppError, AppResult};
use chrono::Datelike;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

const MAX_ID_LENGTH: usize = 128;
const MAX_NAME_LENGTH: usize = 64;
const MAX_TEXT_LENGTH: usize = 160;
const MAX_REPOSITORY_LENGTH: usize = 512;
const MODULE_PREFIX: &str = "fileblade-";
const FALLBACK_MODULE: &str = "module";
const FALLBACK_AUTHOR: &str = "Your Name";
pub const HOST_ID: &str = "data-goblin.fileblade";

pub mod check;
pub mod image;

pub(crate) fn read_bounded(path: &Path, limit: u64) -> AppResult<Vec<u8>> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;

    let refuse =
        |reason: String| AppError::invalid(format!("cannot read {}: {reason}", path.display()));
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| refuse(error.to_string()))?;
    if !file
        .metadata()
        .map_err(|error| refuse(error.to_string()))?
        .is_file()
    {
        return Err(refuse("it is not a regular file".to_string()));
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| refuse(error.to_string()))?;
    if bytes.len() as u64 > limit {
        return Err(AppError::invalid(format!(
            "{} is larger than {limit} bytes",
            path.display()
        )));
    }
    Ok(bytes)
}

#[derive(Clone, Debug, Default)]
pub struct Request {
    pub id: String,
    pub name: Option<String>,
    pub module: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub repository: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Scaffold {
    pub id: String,
    pub publisher: String,
    pub short: String,
    pub name: String,
    pub module: String,
    pub author: String,
    pub description: String,
    pub repository: String,
    pub year: i32,
}

#[derive(Clone, Debug)]
pub struct Rendered {
    pub path: &'static str,
    pub contents: Vec<u8>,
    pub executable: bool,
}

enum Body {
    Text(&'static str),
    Bytes(&'static [u8]),
}

struct Source {
    path: &'static str,
    body: Body,
    executable: bool,
}

macro_rules! template {
    ($path:literal) => {
        Source {
            path: $path,
            body: Body::Text(include_str!(concat!("extension_template/files/", $path))),
            executable: false,
        }
    };
    ($path:literal, executable) => {
        Source {
            path: $path,
            body: Body::Text(include_str!(concat!("extension_template/files/", $path))),
            executable: true,
        }
    };
}

const SOURCES: &[Source] = &[
    template!("manifest.json"),
    template!("Service.qml"),
    template!("Provider.qml"),
    template!("HostGuard.qml"),
    template!("HostGuard.js"),
    template!("blades/Module.qml"),
    template!("README.md"),
    template!("ARCHITECTURE.md"),
    template!("docs/agent-guidelines.md"),
    template!("LICENSE"),
    Source {
        path: ".gitignore",
        body: Body::Text(include_str!("extension_template/files/gitignore")),
        executable: false,
    },
    template!("docs/agent-written/README.md"),
    template!("tests/run", executable),
    template!("tests/tst_host_guard.qml"),
    template!("tests/tst_module.qml"),
    template!("tests/imports/qs/Commons/qmldir"),
    template!("tests/imports/qs/Commons/Style.qml"),
    template!("tests/imports/qs/Commons/Color.qml"),
    template!("tests/imports/qs/Commons/Util.qml"),
    Source {
        path: "assets/fileblade-logo.png",
        body: Body::Bytes(include_bytes!("../assets/fileblade-logo.png")),
        executable: false,
    },
];

pub fn scaffold(request: &Request) -> AppResult<Scaffold> {
    let id = request.id.trim();
    let (publisher, short) = split_id(id)?;
    let module = match &request.module {
        Some(value) => module_id(value.trim())?,
        None => default_module(short),
    };
    let name = match &request.name {
        Some(value) => text("name", value, MAX_NAME_LENGTH)?,
        None => title_case(&module),
    };
    let author = match &request.author {
        Some(value) => text("author", value, MAX_NAME_LENGTH)?,
        None => default_author(),
    };
    let description = match &request.description {
        Some(value) => text("description", value, MAX_TEXT_LENGTH)?,
        None => format!("Adds a {name} blade to FileBlade."),
    };
    let repository = match &request.repository {
        Some(value) => repository(value)?,
        None => format!("https://github.com/{publisher}/{short}.git"),
    };
    Ok(Scaffold {
        id: id.to_string(),
        publisher: publisher.to_string(),
        short: short.to_string(),
        name,
        module,
        author,
        description,
        repository,
        year: chrono::Local::now().year(),
    })
}

pub fn render(scaffold: &Scaffold) -> AppResult<Vec<Rendered>> {
    let year = scaffold.year.to_string();
    let replacements = [
        ("{{PLUGIN_ID}}", scaffold.id.as_str()),
        ("{{PUBLISHER}}", scaffold.publisher.as_str()),
        ("{{PLUGIN_SHORT}}", scaffold.short.as_str()),
        ("{{PLUGIN_NAME}}", scaffold.name.as_str()),
        ("{{MODULE_ID}}", scaffold.module.as_str()),
        ("{{AUTHOR}}", scaffold.author.as_str()),
        ("{{DESCRIPTION}}", scaffold.description.as_str()),
        ("{{REPOSITORY}}", scaffold.repository.as_str()),
        ("{{YEAR}}", year.as_str()),
    ];
    SOURCES
        .iter()
        .map(|source| {
            let contents = match source.body {
                Body::Bytes(bytes) => bytes.to_vec(),
                Body::Text(text) => {
                    let mut rendered = text.to_string();
                    for (placeholder, value) in &replacements {
                        rendered = rendered.replace(placeholder, value);
                    }
                    if let Some(left) = leftover(&rendered) {
                        return Err(AppError::command(format!(
                            "template {} has an unknown placeholder {left}",
                            source.path
                        )));
                    }
                    rendered.into_bytes()
                }
            };
            Ok(Rendered {
                path: source.path,
                contents,
                executable: source.executable,
            })
        })
        .collect()
}

pub fn write(directory: &Path, files: &[Rendered], force: bool) -> AppResult<Vec<String>> {
    match fs::metadata(directory) {
        Ok(metadata) if !metadata.is_dir() => {
            return Err(AppError::invalid(format!(
                "{} exists and is not a directory",
                directory.display()
            )));
        }
        Ok(_) => {
            if !force && fs::read_dir(directory)?.next().is_some() {
                return Err(AppError::invalid(format!(
                    "{} is not empty; pass --force to write the template files into it anyway",
                    directory.display()
                )));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir_all(directory)?,
        Err(error) => return Err(error.into()),
    }
    let mut written = Vec::with_capacity(files.len());
    for file in files {
        let target = directory.join(file.path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&target, &file.contents)?;
        let mode = if file.executable { 0o755 } else { 0o644 };
        fs::set_permissions(&target, fs::Permissions::from_mode(mode))?;
        written.push(file.path.to_string());
    }
    Ok(written)
}

pub fn next_steps(scaffold: &Scaffold, directory: &str) -> Vec<String> {
    vec![
        format!("cd {directory}"),
        "fileblade extension image --png".to_string(),
        "edit manifest.json, README.md and blades/Module.qml".to_string(),
        "git init".to_string(),
        "omarchy plugin validate .".to_string(),
        format!(
            "ln -s \"$PWD\" ~/.config/omarchy/plugins/{id} && omarchy plugin enable {id} && omarchy restart shell",
            id = scaffold.id
        ),
        format!(
            "fileblade blade add right {}/{}",
            scaffold.id, scaffold.module
        ),
        "tests/run".to_string(),
    ]
}

fn split_id(id: &str) -> AppResult<(&str, &str)> {
    let shape = || {
        AppError::invalid(
            "the plugin id must be publisher.name in lowercase letters, digits, - and _, for example acme.fileblade-weather",
        )
    };
    if id.chars().count() > MAX_ID_LENGTH {
        return Err(AppError::invalid(format!(
            "the plugin id is longer than {MAX_ID_LENGTH} characters"
        )));
    }
    let (publisher, short) = id.split_once('.').ok_or_else(shape)?;
    if !is_id_part(publisher) || !is_id_part(short) {
        return Err(shape());
    }
    Ok((publisher, short))
}

fn is_id_part(part: &str) -> bool {
    !part.is_empty()
        && part.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '-' | '_')
        })
}

fn module_id(value: &str) -> AppResult<String> {
    let mut characters = value.chars();
    let valid = matches!(characters.next(), Some(first) if first.is_ascii_alphanumeric())
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
        && value.chars().count() <= MAX_ID_LENGTH;
    if valid {
        Ok(value.to_string())
    } else {
        Err(AppError::invalid(
            "the module id must start with a letter or digit and use only letters, digits, . _ and -",
        ))
    }
}

fn default_module(short: &str) -> String {
    let stripped = short.strip_prefix(MODULE_PREFIX).unwrap_or(short);
    module_id(stripped)
        .or_else(|_| module_id(short))
        .unwrap_or_else(|_| FALLBACK_MODULE.to_string())
}

fn title_case(module: &str) -> String {
    module
        .split(['-', '_', '.'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut characters = word.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn default_author() -> String {
    std::env::var("USER")
        .ok()
        .and_then(|user| text("author", &user, MAX_NAME_LENGTH).ok())
        .unwrap_or_else(|| FALLBACK_AUTHOR.to_string())
}

fn text(field: &str, value: &str, limit: usize) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::invalid(format!("the {field} is empty")));
    }
    if value.chars().count() > limit {
        return Err(AppError::invalid(format!(
            "the {field} is longer than {limit} characters"
        )));
    }
    if value
        .chars()
        .any(|character| character.is_control() || matches!(character, '"' | '\\' | '{' | '}'))
    {
        return Err(AppError::invalid(format!(
            "the {field} must not contain quotes, backslashes, braces or control characters"
        )));
    }
    Ok(value.to_string())
}

fn repository(value: &str) -> AppResult<String> {
    let value = value.trim();
    let valid = !value.is_empty()
        && value.chars().count() <= MAX_REPOSITORY_LENGTH
        && !value.chars().any(|character| {
            character.is_whitespace()
                || character.is_control()
                || matches!(character, '"' | '\\' | '{' | '}' | '<' | '>')
        });
    if valid {
        Ok(value.to_string())
    } else {
        Err(AppError::invalid(
            "the repository must be one URL without spaces, quotes, braces or angle brackets",
        ))
    }
}

fn leftover(text: &str) -> Option<&str> {
    let mut offset = 0;
    while let Some(start) = text[offset..].find("{{") {
        let start = offset + start;
        let rest = &text[start + 2..];
        let end = rest.find("}}")?;
        let inner = &rest[..end];
        if !inner.is_empty()
            && inner
                .chars()
                .all(|character| character.is_ascii_uppercase() || character == '_')
        {
            return Some(&text[start..start + 4 + end]);
        }
        offset = start + 2;
    }
    None
}
