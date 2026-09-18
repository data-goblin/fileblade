use crate::core_modules::canonical::{compact_ascii_json, sha256_hex};
use serde_json::{Map, Number, Value};

#[derive(Clone, Debug)]
pub enum Cfg {
    Null,
    Bool(bool),
    Num(Number),
    Str(String),
    Stamp { kind: &'static str, text: String },
    Array(Vec<Cfg>),
    Table(Vec<(String, Cfg)>),
}

impl PartialEq for Cfg {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Bool(left), Self::Bool(right)) => left == right,
            (Self::Num(left), Self::Num(right)) => left == right,
            (Self::Str(left), Self::Str(right)) => left == right,
            (
                Self::Stamp { kind, text },
                Self::Stamp {
                    kind: other_kind,
                    text: other_text,
                },
            ) => kind == other_kind && text == other_text,
            (Self::Array(left), Self::Array(right)) => left == right,
            (Self::Table(left), Self::Table(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .all(|(key, value)| right_value(right, key) == Some(value))
            }
            _ => false,
        }
    }
}

fn right_value<'a>(entries: &'a [(String, Cfg)], key: &str) -> Option<&'a Cfg> {
    entries
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value)
}

impl Cfg {
    pub fn table() -> Self {
        Self::Table(Vec::new())
    }

    pub fn is_table(&self) -> bool {
        matches!(self, Self::Table(_))
    }

    pub fn as_table(&self) -> Option<&Vec<(String, Cfg)>> {
        match self {
            Self::Table(entries) => Some(entries),
            _ => None,
        }
    }

    pub fn as_table_mut(&mut self) -> Option<&mut Vec<(String, Cfg)>> {
        match self {
            Self::Table(entries) => Some(entries),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Cfg>> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(flag) => Some(*flag),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&Cfg> {
        self.as_table()
            .and_then(|entries| right_value(entries, key))
    }

    pub fn contains(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn keys(&self) -> Vec<String> {
        self.as_table()
            .map(|entries| entries.iter().map(|(key, _)| key.clone()).collect())
            .unwrap_or_default()
    }

    pub fn position(&self, key: &str) -> Option<usize> {
        self.as_table()
            .and_then(|entries| entries.iter().position(|(name, _)| name == key))
    }

    pub fn insert(&mut self, key: &str, value: Cfg) {
        let Some(entries) = self.as_table_mut() else {
            return;
        };
        match entries.iter_mut().find(|(name, _)| name == key) {
            Some(slot) => slot.1 = value,
            None => entries.push((key.to_string(), value)),
        }
    }

    pub fn insert_at(&mut self, position: usize, key: &str, value: Cfg) {
        let Some(entries) = self.as_table_mut() else {
            return;
        };
        let index = position.min(entries.len());
        entries.insert(index, (key.to_string(), value));
    }

    pub fn remove(&mut self, key: &str) -> Option<Cfg> {
        let entries = self.as_table_mut()?;
        let index = entries.iter().position(|(name, _)| name == key)?;
        Some(entries.remove(index).1)
    }

    pub fn from_json(value: &Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Bool(flag) => Self::Bool(*flag),
            Value::Number(number) => Self::Num(number.clone()),
            Value::String(text) => Self::Str(text.clone()),
            Value::Array(items) => Self::Array(items.iter().map(Self::from_json).collect()),
            Value::Object(entries) => Self::Table(
                entries
                    .iter()
                    .map(|(key, item)| (key.clone(), Self::from_json(item)))
                    .collect(),
            ),
        }
    }

    pub fn to_json(&self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Bool(flag) => Value::Bool(*flag),
            Self::Num(number) => Value::Number(number.clone()),
            Self::Str(text) => Value::String(text.clone()),
            Self::Stamp { text, .. } => Value::String(text.clone()),
            Self::Array(items) => Value::Array(items.iter().map(Self::to_json).collect()),
            Self::Table(entries) => {
                let mut map = Map::new();
                for (key, item) in entries {
                    map.insert(key.clone(), item.to_json());
                }
                Value::Object(map)
            }
        }
    }
}

fn typed(value: &Cfg) -> Value {
    match value {
        Cfg::Null => Value::Array(vec![Value::from("NoneType"), Value::Null]),
        Cfg::Bool(flag) => Value::Array(vec![Value::from("bool"), Value::Bool(*flag)]),
        Cfg::Num(number) => {
            if number.is_f64() {
                Value::Array(vec![
                    Value::from("float"),
                    Value::from(crate::core_modules::canonical::python_float_hex(
                        number.as_f64().unwrap_or_default(),
                    )),
                ])
            } else {
                Value::Array(vec![Value::from("int"), Value::Number(number.clone())])
            }
        }
        Cfg::Str(text) => Value::Array(vec![Value::from("str"), Value::from(text.clone())]),
        Cfg::Stamp { kind, text } => {
            Value::Array(vec![Value::from(*kind), Value::from(text.clone())])
        }
        Cfg::Array(items) => Value::Array(vec![
            Value::from("array"),
            Value::Array(items.iter().map(typed).collect()),
        ]),
        Cfg::Table(entries) => {
            let mut keys: Vec<&String> = entries.iter().map(|(key, _)| key).collect();
            keys.sort();
            let pairs = keys
                .into_iter()
                .map(|key| {
                    Value::Array(vec![
                        Value::from(key.clone()),
                        typed(right_value(entries, key).unwrap_or(&Cfg::Null)),
                    ])
                })
                .collect();
            Value::Array(vec![Value::from("object"), Value::Array(pairs)])
        }
    }
}

pub fn fingerprint(value: &Cfg) -> String {
    sha256_hex(compact_ascii_json(&typed(value)).as_bytes())
}

pub fn fingerprint_json(value: &Value) -> String {
    fingerprint(&Cfg::from_json(value))
}
