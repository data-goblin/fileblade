use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fmt::Write as _;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

pub fn sha256_hex(data: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(data);
    let result = digest.finalize();
    let mut text = String::with_capacity(result.len() * 2);
    for byte in result {
        let _ = write!(text, "{byte:02x}");
    }
    text
}

pub fn stable_id_bytes(parts: &[&[u8]]) -> String {
    sha256_hex(&parts.join(&0u8))[..16].to_string()
}

pub fn stable_id(parts: &[&str]) -> String {
    let bytes: Vec<&[u8]> = parts.iter().map(|part| part.as_bytes()).collect();
    stable_id_bytes(&bytes)
}

pub fn scoped_path_id(scope: &str, target: &Path) -> String {
    stable_id_bytes(&[scope.as_bytes(), target.as_os_str().as_bytes()])
}

pub fn path_id(target: &Path) -> String {
    stable_id_bytes(&[target.as_os_str().as_bytes()])
}

pub fn short_digest(value: &str) -> String {
    sha256_hex(value.as_bytes())[..8].to_string()
}

pub fn escape_ascii(text: &str, out: &mut String) {
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            ' '..='~' => out.push(character),
            _ => {
                let code = character as u32;
                if code < 0x10000 {
                    let _ = write!(out, "\\u{code:04x}");
                } else {
                    let offset = code - 0x10000;
                    let high = 0xd800 + (offset >> 10);
                    let low = 0xdc00 + (offset & 0x3ff);
                    let _ = write!(out, "\\u{high:04x}\\u{low:04x}");
                }
            }
        }
    }
    out.push('"');
}

pub fn escape_unicode(text: &str, out: &mut String) {
    out.push_str(&serde_json::to_string(text).expect("JSON string"));
}

pub fn python_float_repr(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value < 0.0 { "-Infinity" } else { "Infinity" }.to_string();
    }
    let sign = if value.is_sign_negative() { "-" } else { "" };
    let magnitude = value.abs();
    if magnitude == 0.0 {
        return format!("{sign}0.0");
    }
    let shortest = format!("{magnitude:e}");
    let (mantissa, exponent) = shortest.split_once('e').expect("exponential formatting");
    let exponent: i32 = exponent.parse().expect("decimal exponent");
    let digits: String = mantissa
        .chars()
        .filter(|character| *character != '.')
        .collect();
    let point = exponent + 1;
    if point <= -4 || point > 16 {
        let mut text = String::from(sign);
        text.push_str(&digits[..1]);
        if digits.len() > 1 {
            text.push('.');
            text.push_str(&digits[1..]);
        }
        let power = point - 1;
        let _ = write!(
            text,
            "e{}{:02}",
            if power < 0 { '-' } else { '+' },
            power.abs()
        );
        return text;
    }
    if point <= 0 {
        return format!("{sign}0.{}{digits}", "0".repeat(-point as usize));
    }
    let point = point as usize;
    if point >= digits.len() {
        return format!("{sign}{digits}{}.0", "0".repeat(point - digits.len()));
    }
    format!("{sign}{}.{}", &digits[..point], &digits[point..])
}

pub fn python_float_hex(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    if value.is_infinite() {
        return if value < 0.0 { "-inf" } else { "inf" }.to_string();
    }
    let sign = if value.is_sign_negative() { "-" } else { "" };
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let mantissa = bits & 0x000f_ffff_ffff_ffff;
    if exponent == 0 && mantissa == 0 {
        return format!("{sign}0x0.0p+0");
    }
    let (lead, power) = if exponent == 0 {
        (0, -1022)
    } else {
        (1, exponent - 1023)
    };
    format!(
        "{sign}0x{lead}.{mantissa:013x}p{}{}",
        if power < 0 { '-' } else { '+' },
        power.abs()
    )
}

fn number(value: &serde_json::Number, out: &mut String) {
    if let Some(integer) = value.as_i64() {
        let _ = write!(out, "{integer}");
    } else if let Some(integer) = value.as_u64() {
        let _ = write!(out, "{integer}");
    } else {
        out.push_str(&python_float_repr(value.as_f64().unwrap_or_default()));
    }
}

fn encode(value: &Value, sort_keys: bool, ascii: bool, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(item) => number(item, out),
        Value::String(text) => {
            if ascii {
                escape_ascii(text, out);
            } else {
                escape_unicode(text, out);
            }
        }
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                encode(item, sort_keys, ascii, out);
            }
            out.push(']');
        }
        Value::Object(entries) => {
            let mut keys: Vec<&String> = entries.keys().collect();
            if sort_keys {
                keys.sort();
            }
            out.push('{');
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                if ascii {
                    escape_ascii(key, out);
                } else {
                    escape_unicode(key, out);
                }
                out.push(':');
                encode(&entries[key], sort_keys, ascii, out);
            }
            out.push('}');
        }
    }
}

pub fn canonical_json(value: &Value) -> String {
    let mut out = String::new();
    encode(value, true, true, &mut out);
    out
}

pub fn compact_ascii_json(value: &Value) -> String {
    let mut out = String::new();
    encode(value, false, true, &mut out);
    out
}

pub fn compact_json(value: &Value) -> String {
    let mut out = String::new();
    encode(value, false, false, &mut out);
    out
}

pub fn fingerprint(value: &Value) -> String {
    sha256_hex(canonical_json(value).as_bytes())
}

pub const DUPLICATE_MARKER: &str = "fileblade-duplicate-json-key";

pub struct UniqueValue(pub Value);

struct UniqueVisitor;

impl<'de> Visitor<'de> for UniqueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("any valid JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_str<E>(self, value: &str) -> Result<Value, E>
    where
        E: de::Error,
    {
        Ok(Value::from(value.to_string()))
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_none<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueVisitor)
    }

    fn visit_seq<A>(self, mut access: A) -> Result<Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut items = Vec::new();
        while let Some(UniqueValue(item)) = access.next_element()? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A>(self, mut access: A) -> Result<Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut entries = Map::new();
        while let Some(key) = access.next_key::<String>()? {
            let UniqueValue(value) = access.next_value()?;
            if entries.contains_key(&key) {
                return Err(de::Error::custom(DUPLICATE_MARKER));
            }
            entries.insert(key, value);
        }
        Ok(Value::Object(entries))
    }
}

impl<'de> serde::Deserialize<'de> for UniqueValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueVisitor).map(UniqueValue)
    }
}
