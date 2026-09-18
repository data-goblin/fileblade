use super::value::Cfg;
use crate::core_modules::canonical::python_float_repr;
use regex::Regex;
use std::fmt::Write as _;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TomlWriteFailure(pub String);

impl std::fmt::Display for TomlWriteFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn failure(code: &str) -> TomlWriteFailure {
    TomlWriteFailure(code.to_string())
}

fn bare_key() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^[A-Za-z0-9_-]+$").expect("bare key pattern"))
}

fn table_header() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^\s*\[").expect("table header pattern"))
}

pub fn quote_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\t' => escaped.push_str("\\t"),
            '\r' => escaped.push_str("\\r"),
            _ if (character as u32) < 0x20 || character == '\u{7f}' => {
                let _ = write!(escaped, "\\u{:04X}", character as u32);
            }
            _ => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}

pub fn render_key(key: &str) -> String {
    if bare_key().is_match(key) {
        key.to_string()
    } else {
        quote_string(key)
    }
}

pub fn render_value(value: &Cfg) -> Result<String, TomlWriteFailure> {
    match value {
        Cfg::Bool(flag) => Ok(if *flag { "true" } else { "false" }.to_string()),
        Cfg::Num(number) => {
            if number.is_f64() {
                Ok(python_float_repr(number.as_f64().unwrap_or_default()))
            } else {
                Ok(number.to_string())
            }
        }
        Cfg::Str(text) => Ok(quote_string(text)),
        Cfg::Array(items) => {
            let mut parts: Vec<String> = Vec::new();
            for item in items {
                parts.push(render_value(item)?);
            }
            Ok(format!("[{}]", parts.join(", ")))
        }
        Cfg::Table(entries) => {
            let mut parts: Vec<String> = Vec::new();
            for (key, item) in entries {
                parts.push(format!("{} = {}", render_key(key), render_value(item)?));
            }
            Ok(format!("{{ {} }}", parts.join(", ")))
        }
        _ => Err(failure("unsupported-value")),
    }
}

pub fn render_server_table(name: &str, values: &Cfg) -> Result<String, TomlWriteFailure> {
    let entries = values.as_table().cloned().unwrap_or_default();
    let mut lines: Vec<String> = vec![format!("[mcp_servers.{}]", render_key(name))];
    let mut nested: Vec<(String, Vec<(String, Cfg)>)> = Vec::new();
    for (key, value) in &entries {
        if let Cfg::Table(inner) = value
            && !inner.is_empty()
            && inner.iter().all(|(_, item)| matches!(item, Cfg::Str(_)))
        {
            nested.push((key.clone(), inner.clone()));
            continue;
        }
        lines.push(format!("{} = {}", render_key(key), render_value(value)?));
    }
    for (key, table) in nested {
        if table.is_empty() {
            continue;
        }
        lines.push(String::new());
        lines.push(format!(
            "[mcp_servers.{}.{}]",
            render_key(name),
            render_key(&key)
        ));
        for (inner_key, inner_value) in table {
            lines.push(format!(
                "{} = {}",
                render_key(&inner_key),
                render_value(&inner_value)?
            ));
        }
    }
    Ok(lines.join("\n") + "\n")
}

fn name_forms(name: &str) -> String {
    let mut forms: Vec<String> = Vec::new();
    if bare_key().is_match(name) {
        forms.push(regex::escape(name));
    }
    forms.push(regex::escape(&quote_string(name)));
    forms.push(regex::escape(&format!("'{name}'")));
    forms.join("|")
}

fn header_pattern(name: &str) -> Regex {
    Regex::new(&format!(
        r"^\s*\[\s*mcp_servers\s*\.\s*(?:{})\s*\]\s*(?:#.*)?$",
        name_forms(name)
    ))
    .expect("server header pattern")
}

fn subtable_pattern(name: &str) -> Regex {
    Regex::new(&format!(
        r"^\s*\[\s*mcp_servers\s*\.\s*(?:{})\s*\.",
        name_forms(name)
    ))
    .expect("server subtable pattern")
}

pub fn split_lines(text: &str) -> Vec<String> {
    const BREAKS: [char; 8] = [
        '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}',
    ];
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        current.push(character);
        if BREAKS.contains(&character) || character == '\u{2028}' || character == '\u{2029}' {
            if character == '\r' && characters.peek() == Some(&'\n') {
                current.push(characters.next().unwrap_or('\n'));
            }
            lines.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn without_terminator(line: &str) -> &str {
    line.trim_end_matches([
        '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}',
        '\u{2029}',
    ])
}

pub fn locate_server_block(
    text: &str,
    name: &str,
) -> Result<Option<(usize, usize)>, TomlWriteFailure> {
    let lines = split_lines(text);
    let header = header_pattern(name);
    let subtable = subtable_pattern(name);
    let starts: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| header.is_match(without_terminator(line)))
        .map(|(index, _)| index)
        .collect();
    if starts.is_empty() {
        return Ok(None);
    }
    if starts.len() > 1 {
        return Err(failure("ambiguous-table"));
    }
    let start = starts[0];
    let mut end = lines.len();
    for (index, line) in lines.iter().enumerate().skip(start + 1) {
        if table_header().is_match(line) && !subtable.is_match(line) {
            end = index;
            break;
        }
    }
    while end > start + 1 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    Ok(Some((start, end)))
}

pub fn remove_server_block(text: &str, name: &str) -> Result<String, TomlWriteFailure> {
    let Some((mut start, end)) = locate_server_block(text, name)? else {
        return Err(failure("table-not-found"));
    };
    let lines = split_lines(text);
    while start > 0 && lines[start - 1].trim().is_empty() {
        start -= 1;
    }
    let mut result = String::new();
    for line in lines.iter().take(start) {
        result.push_str(line);
    }
    for line in lines.iter().skip(end) {
        result.push_str(line);
    }
    if !result.is_empty() && !result.ends_with('\n') {
        result.push('\n');
    }
    Ok(result)
}

pub fn append_server_block(text: &str, block: &str) -> String {
    let mut result = text.to_string();
    if !result.is_empty() && !result.ends_with('\n') {
        result.push('\n');
    }
    if !result.is_empty() && !result.ends_with("\n\n") {
        result.push('\n');
    }
    result.push_str(block);
    result
}
