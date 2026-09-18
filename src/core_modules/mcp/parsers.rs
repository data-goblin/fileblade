use super::value::{Cfg, SURROGATE_PLACEHOLDER_BASE};
use crate::core_modules::canonical::{DUPLICATE_MARKER, UniqueValue};
use std::fmt;

pub const MAX_NESTING: usize = 32;
pub const MAX_CONTAINER_ITEMS: usize = 4096;
pub const MAX_KEY_CHARS: usize = 1024;
pub const MAX_STRING_CHARS: usize = 1_048_576;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseFailure(pub String);

impl ParseFailure {
    pub fn code(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ParseFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn failure(code: &str) -> ParseFailure {
    ParseFailure(code.to_string())
}

fn bounded_shape(value: &Cfg, depth: usize) -> Result<(), ParseFailure> {
    if depth > MAX_NESTING {
        return Err(failure("too-deep"));
    }
    match value {
        Cfg::Table(entries) => {
            if entries.len() > MAX_CONTAINER_ITEMS {
                return Err(failure("too-many-items"));
            }
            for (key, child) in entries {
                if key.chars().count() > MAX_KEY_CHARS {
                    return Err(failure("invalid-key"));
                }
                bounded_shape(child, depth + 1)?;
            }
            Ok(())
        }
        Cfg::Array(items) => {
            if items.len() > MAX_CONTAINER_ITEMS {
                return Err(failure("too-many-items"));
            }
            for child in items {
                bounded_shape(child, depth + 1)?;
            }
            Ok(())
        }
        Cfg::Str(text) if text.chars().count() > MAX_STRING_CHARS => {
            Err(failure("oversized-string"))
        }
        _ => Ok(()),
    }
}

fn nonfinite_token(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut quoted = false;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
            index += 1;
            continue;
        }
        if byte == b'"' {
            quoted = true;
            index += 1;
            continue;
        }
        for token in ["NaN", "Infinity"] {
            if bytes[index..].starts_with(token.as_bytes()) {
                return true;
            }
        }
        index += 1;
    }
    false
}

fn hex_escape(characters: &[char], index: usize) -> Option<u32> {
    if characters.get(index) != Some(&'\\') || characters.get(index + 1) != Some(&'u') {
        return None;
    }
    let mut point = 0u32;
    for offset in 2..6 {
        let digit = characters.get(index + offset)?.to_digit(16)?;
        point = point * 16 + digit;
    }
    Some(point)
}

pub fn substitute_lone_surrogates(text: &str) -> Option<String> {
    let characters: Vec<char> = text.chars().collect();
    let mut output = String::with_capacity(text.len());
    let mut index = 0;
    let mut quoted = false;
    let mut replaced = false;
    while index < characters.len() {
        let character = characters[index];
        if !quoted {
            if character == '"' {
                quoted = true;
            }
            output.push(character);
            index += 1;
            continue;
        }
        if character == '"' {
            quoted = false;
            output.push(character);
            index += 1;
            continue;
        }
        if character != '\\' {
            output.push(character);
            index += 1;
            continue;
        }
        let Some(point) = hex_escape(&characters, index) else {
            output.push(character);
            output.extend(characters.get(index + 1));
            index += 2;
            continue;
        };
        if (0xD800..0xDC00).contains(&point)
            && hex_escape(&characters, index + 6).is_some_and(|low| (0xDC00..0xE000).contains(&low))
        {
            output.extend(&characters[index..index + 12]);
            index += 12;
            continue;
        }
        if (0xD800..0xE000).contains(&point) {
            let placeholder = char::from_u32(SURROGATE_PLACEHOLDER_BASE + point - 0xD800)?;
            output.push(placeholder);
            replaced = true;
            index += 6;
            continue;
        }
        output.extend(&characters[index..index + 6]);
        index += 6;
    }
    if replaced && !quoted {
        Some(output)
    } else {
        None
    }
}

pub fn parse_json(data: &[u8]) -> Result<Cfg, ParseFailure> {
    let text = std::str::from_utf8(data).map_err(|_| failure("invalid-json"))?;
    let substituted = match serde_json::from_str::<UniqueValue>(text) {
        Err(error) if error.to_string().contains("surrogate") => substitute_lone_surrogates(text),
        _ => None,
    };
    let text = substituted.as_deref().unwrap_or(text);
    let parsed = match serde_json::from_str::<UniqueValue>(text) {
        Ok(UniqueValue(value)) => value,
        Err(error) => {
            if error.to_string().contains(DUPLICATE_MARKER) {
                return Err(failure("duplicate-json-key"));
            }
            if nonfinite_token(text) {
                return Err(failure("nonfinite-json-number"));
            }
            return Err(failure("invalid-json"));
        }
    };
    if !parsed.is_object() {
        return Err(failure("root-not-object"));
    }
    let value = Cfg::from_json(&parsed);
    bounded_shape(&value, 0)?;
    Ok(value)
}

pub fn strip_jsonc(text: &str) -> Result<String, ParseFailure> {
    let characters: Vec<char> = text.chars().collect();
    let mut output: Vec<char> = Vec::with_capacity(characters.len());
    let mut index = 0;
    let mut quoted = false;
    let mut escaped = false;
    while index < characters.len() {
        let character = characters[index];
        let following = characters.get(index + 1).copied().unwrap_or('\0');
        if quoted {
            output.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
            index += 1;
            continue;
        }
        if character == '"' {
            quoted = true;
            output.push(character);
            index += 1;
            continue;
        }
        if character == '/' && following == '/' {
            output.push(' ');
            output.push(' ');
            index += 2;
            while index < characters.len() && characters[index] != '\r' && characters[index] != '\n'
            {
                output.push(' ');
                index += 1;
            }
            continue;
        }
        if character == '/' && following == '*' {
            output.push(' ');
            output.push(' ');
            index += 2;
            let mut closed = false;
            while index < characters.len() {
                if characters[index] == '*' && characters.get(index + 1) == Some(&'/') {
                    output.push(' ');
                    output.push(' ');
                    index += 2;
                    closed = true;
                    break;
                }
                output.push(if characters[index] == '\n' { '\n' } else { ' ' });
                index += 1;
            }
            if !closed {
                return Err(failure("invalid-jsonc"));
            }
            continue;
        }
        output.push(character);
        index += 1;
    }

    let mut quoted = false;
    let mut escaped = false;
    let mut index = 0;
    while index < output.len() {
        let character = output[index];
        if quoted {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
        } else if character == '"' {
            quoted = true;
        } else if character == ',' {
            let mut lookahead = index + 1;
            while lookahead < output.len() && output[lookahead].is_whitespace() {
                lookahead += 1;
            }
            if lookahead < output.len() && (output[lookahead] == '}' || output[lookahead] == ']') {
                output[index] = ' ';
            }
        }
        index += 1;
    }
    Ok(output.into_iter().collect())
}

pub fn parse_jsonc(data: &[u8]) -> Result<Cfg, ParseFailure> {
    let text = std::str::from_utf8(data).map_err(|_| failure("invalid-utf8"))?;
    parse_json(strip_jsonc(text)?.as_bytes())
}

fn stamp(value: &toml::value::Datetime) -> Cfg {
    let date = value.date;
    let time = value.time;
    let mut text = String::new();
    if let Some(date) = date {
        text.push_str(&format!(
            "{:04}-{:02}-{:02}",
            date.year, date.month, date.day
        ));
    }
    if let Some(time) = time {
        if date.is_some() {
            text.push('T');
        }
        text.push_str(&format!(
            "{:02}:{:02}:{:02}",
            time.hour, time.minute, time.second
        ));
        let microseconds = time.nanosecond / 1000;
        if microseconds > 0 {
            text.push_str(&format!(".{microseconds:06}"));
        }
    }
    let kind = match (date.is_some(), time.is_some()) {
        (true, true) => "datetime",
        (true, false) => "date",
        _ => "time",
    };
    if let Some(offset) = value.offset {
        let minutes = match offset {
            toml::value::Offset::Z => 0,
            toml::value::Offset::Custom { minutes } => minutes,
        };
        let sign = if minutes < 0 { '-' } else { '+' };
        let absolute = minutes.unsigned_abs() as u32;
        text.push_str(&format!("{sign}{:02}:{:02}", absolute / 60, absolute % 60));
    }
    Cfg::Stamp { kind, text }
}

fn from_toml(value: &toml::Value) -> Cfg {
    match value {
        toml::Value::String(text) => Cfg::Str(text.clone()),
        toml::Value::Integer(number) => Cfg::Num((*number).into()),
        toml::Value::Float(number) => match serde_json::Number::from_f64(*number) {
            Some(number) => Cfg::Num(number),
            None if number.is_nan() => Cfg::Nonfinite("nan"),
            None if *number < 0.0 => Cfg::Nonfinite("-inf"),
            None => Cfg::Nonfinite("inf"),
        },
        toml::Value::Boolean(flag) => Cfg::Bool(*flag),
        toml::Value::Datetime(value) => stamp(value),
        toml::Value::Array(items) => Cfg::Array(items.iter().map(from_toml).collect()),
        toml::Value::Table(entries) => Cfg::Table(
            entries
                .iter()
                .map(|(key, item)| (key.clone(), from_toml(item)))
                .collect(),
        ),
    }
}

pub fn parse_toml(data: &[u8]) -> Result<Cfg, ParseFailure> {
    let text = std::str::from_utf8(data).map_err(|_| failure("invalid-toml"))?;
    let table: toml::Table = text.parse().map_err(|_| failure("invalid-toml"))?;
    let value = from_toml(&toml::Value::Table(table));
    bounded_shape(&value, 0)?;
    Ok(value)
}
