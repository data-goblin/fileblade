use super::*;

const MAX_RING: usize = 12;
const MAX_NODES: usize = 96;
const BUILTINS: &[&str] = &[
    "open",
    "open-with",
    "terminal",
    "review",
    "mux-open",
    "nvim-open",
    "app-open",
    "application",
    "copy-paths",
];
const DISPLAY_FIELDS: &[&str] = &["label", "key", "glyph", "icon", "description", "group"];

struct MergeContext<'a> {
    target: &'a Value,
    facts: &'a Value,
    budget: usize,
    diagnostics: Vec<String>,
}

pub fn merge(
    defaults: &[Value],
    document: &Value,
    target: &Value,
    facts: &Value,
) -> (Vec<Value>, Vec<String>) {
    let mut context = MergeContext {
        target,
        facts,
        budget: MAX_NODES,
        diagnostics: Vec::new(),
    };
    let mut base = defaults.to_vec();
    annotate_builtins(&mut base, "", "");
    if document.is_null() {
        return (base, context.diagnostics);
    }
    if !document.is_object() || document["version"] != 1 {
        return (
            base,
            vec!["dropWheel: unsupported configuration version; using defaults".into()],
        );
    }
    if let Some(custom) = array(document, "customActions", &mut context.diagnostics) {
        let mut seen = HashSet::new();
        for (index, entry) in custom.iter().take(MAX_RING).enumerate() {
            let location = format!("dropWheel.customActions[{index}]");
            let id = text_field(entry, "id");
            if !id.starts_with("custom:") || !seen.insert(id.clone()) {
                context
                    .diagnostics
                    .push(format!("{location}: requires a unique custom: id"));
                continue;
            }
            match custom_node(entry, 0, &base, &location, &mut context) {
                Ok(Some(row)) => base.push(row),
                Ok(None) => {}
                Err(error) => context.diagnostics.push(format!("{location}: {error}")),
            }
        }
        if custom.len() > MAX_RING {
            context
                .diagnostics
                .push("dropWheel.customActions: at most 12 actions".into());
        }
    }
    let catalogue = base.clone();
    let mut rows = match array(document, "actions", &mut context.diagnostics) {
        Some(overrides) => merge_rows(
            base,
            overrides,
            0,
            &catalogue,
            "dropWheel.actions",
            &mut context,
        ),
        None => base,
    };
    if rows.len() > MAX_RING {
        rows.truncate(MAX_RING);
        context
            .diagnostics
            .push("dropWheel: at most 12 visible actions".into());
    }
    finish_rows(&mut rows, &[]);
    (rows, context.diagnostics)
}

pub(super) fn load(defaults: &[Value], target: &Value, facts: &Value) -> (Vec<Value>, Vec<String>) {
    match crate::preferences::read() {
        Ok(settings) => merge(defaults, &settings["dropWheel"], target, facts),
        Err(error) => (defaults.to_vec(), vec![format!("dropWheel: {error}")]),
    }
}

fn array<'a>(entry: &'a Value, key: &str, diagnostics: &mut Vec<String>) -> Option<&'a Vec<Value>> {
    match entry.get(key) {
        None => None,
        Some(Value::Array(rows)) => Some(rows),
        _ => {
            diagnostics.push(format!("dropWheel.{key}: expected an array"));
            None
        }
    }
}

fn identity(row: &Value) -> String {
    let id = text_field(row, "id");
    let desktop = text_field(row, "desktop_id");
    if !desktop.is_empty() && (id.is_empty() || id == "application") {
        desktop
    } else {
        id
    }
}

fn annotate_builtins(rows: &mut [Value], action: &str, placement: &str) {
    for row in rows {
        let root = if action.is_empty() {
            text_field(row, "id")
        } else {
            action.into()
        };
        let child = if action.is_empty() {
            String::new()
        } else {
            text_field(row, "id")
        };
        row["builtin_action"] = json!(if root == "open-with" && !child.is_empty() {
            "application"
        } else {
            &root
        });
        row["builtin_placement"] = json!(if child.is_empty() { placement } else { &child });
        if let Some(children) = row.get_mut("placements").and_then(Value::as_array_mut) {
            annotate_builtins(children, &root, &child);
        }
    }
}

fn display(entry: &Value, row: &mut Value) -> Result<(), String> {
    for key in DISPLAY_FIELDS {
        if let Some(value) = entry.get(*key) {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{key} must be text"))?;
            if text.len() > 512 || text.chars().any(char::is_control) {
                return Err(format!("{key} is too long or contains control characters"));
            }
            if *key == "label" && text.trim().is_empty() {
                return Err("label is empty".into());
            }
            if *key == "key" && (text.len() > 1 || !text.bytes().all(|c| c.is_ascii_alphanumeric()))
            {
                return Err("key must be one ASCII letter or digit".into());
            }
            if *key == "icon" {
                if !text.is_empty()
                    && !text
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
                {
                    return Err("icon must be a theme icon name or bundled mark".into());
                }
                row["icon_source"] = json!("");
                row["icon_mask"] = json!("alpha");
                row["icon_override"] = json!(true);
            }
            if *key == "glyph" && entry.get("icon").is_none() {
                row["icon"] = json!("");
                row["icon_source"] = json!("");
                row["icon_override"] = json!(true);
            }
            row[*key] = value.clone();
        }
    }
    Ok(())
}

fn hidden(entry: &Value) -> Result<bool, String> {
    match entry.get("hidden") {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        _ => Err("hidden must be true or false".into()),
    }
}

fn merge_rows(
    mut base: Vec<Value>,
    overrides: &[Value],
    depth: usize,
    catalogue: &[Value],
    location: &str,
    context: &mut MergeContext<'_>,
) -> Vec<Value> {
    let mut rows = Vec::new();
    let mut seen = HashSet::new();
    for (index, entry) in overrides.iter().take(MAX_RING).enumerate() {
        let here = format!("{location}[{index}]");
        let id = identity(entry);
        if id.is_empty() || !seen.insert(id.clone()) {
            context
                .diagnostics
                .push(format!("{here}: missing or duplicate id"));
            continue;
        }
        let position = base.iter().position(|row| identity(row) == id);
        let result = if let Some(position) = position {
            let mut row = base.remove(position);
            let original = row.clone();
            let adjusted = (|| {
                if hidden(entry)? {
                    return Ok(None);
                }
                if entry.get("command").is_some()
                    || entry.get("builtin").is_some()
                    || entry.get("runMode").is_some()
                {
                    return Err("existing actions cannot be redefined; add a custom: action".into());
                }
                display(entry, &mut row)?;
                if let Some(children) = entry.get("placements") {
                    let children = children.as_array().ok_or("placements must be an array")?;
                    if depth >= 2 && !children.is_empty() {
                        return Err("only two placement layers are supported".into());
                    }
                    let defaults = row["placements"].as_array().cloned().unwrap_or_default();
                    let merged = merge_rows(
                        defaults,
                        children,
                        depth + 1,
                        catalogue,
                        &format!("{here}.placements"),
                        context,
                    );
                    if !children.is_empty() && merged.is_empty() {
                        return Ok(None);
                    }
                    row["placements"] = json!(merged);
                }
                Ok(Some(row))
            })();
            if adjusted.is_err() {
                base.insert(position, original);
            }
            adjusted
        } else if depth > 0 && id.starts_with("custom:") {
            custom_node(entry, depth, catalogue, &here, context)
        } else if BUILTINS.contains(&id.as_str()) || entry.get("desktop_id").is_some() {
            Ok(None)
        } else {
            Err("unknown action or placement id".into())
        };
        match result {
            Ok(Some(row)) => rows.push(row),
            Ok(None) => {}
            Err(error) => context.diagnostics.push(format!("{here}: {error}")),
        }
    }
    rows.extend(base);
    if overrides.len() > MAX_RING || rows.len() > MAX_RING {
        context
            .diagnostics
            .push(format!("{location}: at most 12 entries per ring"));
    }
    rows.truncate(MAX_RING);
    rows
}

fn custom_node(
    entry: &Value,
    depth: usize,
    catalogue: &[Value],
    location: &str,
    context: &mut MergeContext<'_>,
) -> Result<Option<Value>, String> {
    if !entry.is_object() {
        return Err("entry must be an object".into());
    }
    if context.budget == 0 {
        return Err("configuration exceeds 96 nodes".into());
    }
    context.budget -= 1;
    if hidden(entry)? {
        return Ok(None);
    }
    let id = text_field(entry, "id");
    if id.is_empty()
        || id.len() > 96
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_:".contains(&c))
        || BUILTINS.contains(&id.as_str())
    {
        return Err("invalid or reserved id".into());
    }
    let mut row = json!({"id": id, "label": "", "key": "", "glyph": "", "description": "", "group": "custom", "placements": []});
    display(entry, &mut row)?;
    if text_field(&row, "label").is_empty() {
        return Err("label is required".into());
    }
    if !applicable(entry, context.target, context.facts)? {
        return Ok(None);
    }
    if let Some(reference) = entry.get("builtin") {
        if entry.get("command").is_some() {
            return Err("choose builtin or command, not both".into());
        }
        row["configured_builtin"] = json!(true);
        let action = text_field(reference, "action");
        let placement = text_field(reference, "placement");
        let mut source = catalogue
            .iter()
            .find(|row| row["id"] == action && row.get("builtin_action").is_some())
            .ok_or("built-in action is unavailable for this target")?;
        if !placement.is_empty() {
            source = source["placements"]
                .as_array()
                .and_then(|rows| rows.iter().find(|row| identity(row) == placement))
                .ok_or("built-in placement is unavailable")?;
        }
        if source["builtin_action"] == "open-with"
            || source["placements"]
                .as_array()
                .is_some_and(|rows| !rows.is_empty())
        {
            return Err("builtin reference must identify an executable placement".into());
        }
        for key in [
            "builtin_action",
            "builtin_placement",
            "desktop_id",
            "enabled",
        ] {
            if let Some(value) = source.get(key) {
                row[key] = value.clone();
            }
        }
        if entry.get("glyph").is_none() {
            if let Some(value) = source.get("glyph") {
                row["glyph"] = value.clone();
            }
            if entry.get("icon").is_none() {
                for key in ["icon", "icon_source", "icon_mask"] {
                    if let Some(value) = source.get(key) {
                        row[key] = value.clone();
                    }
                }
            }
        }
    }
    if let Some(command) = entry.get("command") {
        validate_template(command)?;
        row["command"] = command.clone();
        let mode = match entry.get("runMode") {
            None => "detached",
            Some(value) => value.as_str().ok_or("runMode must be text")?,
        };
        if !matches!(mode, "detached" | "terminal" | "multiplexer") {
            return Err("invalid runMode".into());
        }
        row["runMode"] = json!(mode);
        if mode == "multiplexer" {
            let Some(mux) = resolved_multiplexer(context.target) else {
                return Ok(None);
            };
            let placement = text_field(entry, "placement");
            let choices = if mux == "herdr" {
                &HERDR_PLACEMENTS[..]
            } else {
                &TMUX_PLACEMENTS[..]
            };
            if !choices.iter().any(|choice| choice.0 == placement) {
                return Err("invalid multiplexer placement".into());
            }
            row["placement"] = json!(placement);
        }
    }
    if let Some(children) = entry.get("placements") {
        let children = children.as_array().ok_or("placements must be an array")?;
        if depth >= 2 && !children.is_empty() {
            return Err("only two placement layers are supported".into());
        }
        let mut merged = Vec::new();
        let mut seen = HashSet::new();
        for (index, child) in children.iter().take(MAX_RING).enumerate() {
            let here = format!("{location}.placements[{index}]");
            if !seen.insert(identity(child)) {
                context.diagnostics.push(format!("{here}: duplicate id"));
                continue;
            }
            match custom_node(child, depth + 1, catalogue, &here, context) {
                Ok(Some(row)) => merged.push(row),
                Ok(None) => {}
                Err(error) => context.diagnostics.push(format!("{here}: {error}")),
            }
        }
        if children.len() > MAX_RING {
            context
                .diagnostics
                .push(format!("{location}: at most 12 placements"));
        }
        row["placements"] = json!(merged);
    }
    if row.get("command").is_none()
        && row.get("builtin_action").is_none()
        && row["placements"].as_array().is_none_or(Vec::is_empty)
    {
        return Err("requires a command, builtin reference, or valid placements".into());
    }
    Ok(Some(row))
}

fn applicable(entry: &Value, target: &Value, facts: &Value) -> Result<bool, String> {
    if let Some(kinds) = entry.get("targetKinds") {
        let kinds = kinds.as_array().ok_or("targetKinds must be an array")?;
        if kinds.is_empty()
            || kinds.iter().any(|kind| {
                !matches!(
                    kind.as_str(),
                    Some("desktop" | "terminal" | "editor" | "window" | "blade")
                )
            })
        {
            return Err("invalid targetKinds".into());
        }
        let kind = text_field(target, "kind");
        let normalized = if matches!(kind.as_str(), "app" | "browser") {
            "window"
        } else {
            &kind
        };
        if !kinds.iter().any(|kind| kind == normalized) {
            return Ok(false);
        }
    }
    if let Some(conditions) = entry.get("conditions") {
        if !conditions.is_object() {
            return Err("conditions must be an object".into());
        }
        for key in ["mime", "path"] {
            if let Some(patterns) = conditions.get(key) {
                let patterns = patterns
                    .as_array()
                    .ok_or_else(|| format!("conditions.{key} must be an array"))?;
                if patterns.is_empty()
                    || patterns.len() > 12
                    || patterns.iter().any(|pattern| {
                        pattern
                            .as_str()
                            .is_none_or(|s| s.is_empty() || s.len() > 256)
                    })
                {
                    return Err(format!("invalid conditions.{key}"));
                }
                let patterns = patterns
                    .iter()
                    .map(|pattern| wildcard(pattern.as_str().unwrap()))
                    .collect::<Result<Vec<_>, _>>()?;
                let paths = string_array(facts, "paths");
                if paths.is_empty()
                    || paths.iter().any(|path| {
                        let value = if key == "mime" {
                            facts["mimes"][path].as_str().unwrap_or("")
                        } else {
                            path.as_str()
                        };
                        value.is_empty() || !patterns.iter().any(|pattern| pattern.is_match(value))
                    })
                {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

fn wildcard(pattern: &str) -> Result<regex::Regex, String> {
    let mut expression = String::from("^");
    for part in pattern.split_inclusive('*') {
        let literal = part.strip_suffix('*').unwrap_or(part);
        expression.push_str(&regex::escape(literal));
        if part.ends_with('*') {
            expression.push_str(".*");
        }
    }
    expression.push('$');
    regex::RegexBuilder::new(&expression)
        .size_limit(64 * 1024)
        .build()
        .map_err(|error| format!("invalid condition pattern: {error}"))
}

fn validate_template(command: &Value) -> Result<(), String> {
    let args = command
        .as_array()
        .ok_or("command must be an argument array")?;
    if args.is_empty() || args.len() > 128 {
        return Err("command requires 1 to 128 arguments".into());
    }
    for (index, arg) in args.iter().enumerate() {
        let arg = arg.as_str().ok_or("command arguments must be strings")?;
        if arg.len() > 4096 || arg.contains('\0') {
            return Err("command argument is invalid or too long".into());
        }
        let substitution = matches!(arg, "{paths}" | "{path}" | "{cwd}" | "{git_root}");
        if (arg.contains('{') || arg.contains('}')) && !substitution {
            return Err("substitutions must occupy whole arguments".into());
        }
        if index == 0 && (arg.is_empty() || substitution) {
            return Err("command requires a literal executable".into());
        }
    }
    Ok(())
}

pub fn expand(command: &Value, facts: &Value) -> AppResult<Vec<OsString>> {
    validate_template(command).map_err(AppError::invalid)?;
    let paths = string_array(facts, "paths");
    let mut result = Vec::new();
    for argument in command.as_array().unwrap() {
        let argument = argument.as_str().unwrap();
        match argument {
            "{paths}" => {
                if paths.is_empty() {
                    return Err(AppError::invalid("{paths} requires selected paths"));
                }
                for path in &paths {
                    result.push(parse_path(path)?.into_os_string());
                }
            }
            "{path}" => {
                if paths.len() != 1 {
                    return Err(AppError::invalid(
                        "{path} requires exactly one selected path",
                    ));
                }
                result.push(parse_path(&paths[0])?.into_os_string());
            }
            "{cwd}" | "{git_root}" => {
                let value = text_field(
                    facts,
                    if argument == "{cwd}" {
                        "folder"
                    } else {
                        "git_root"
                    },
                );
                if value.is_empty() {
                    return Err(AppError::invalid(format!("{argument} is unavailable")));
                }
                result.push(parse_path(&value)?.into_os_string());
            }
            _ => result.push(argument.into()),
        }
    }
    Ok(result)
}

fn finish_rows(rows: &mut [Value], parent: &[String]) {
    assign_keys(rows);
    for row in rows {
        let mut route = parent.to_vec();
        route.push(identity(row));
        if row.get("command").is_some() || row["configured_builtin"] == true {
            row["command_route"] = json!(route);
        }
        if let Some(children) = row.get_mut("placements").and_then(Value::as_array_mut) {
            finish_rows(children, &route);
        }
    }
}

pub(super) fn resolve<'a>(rows: &'a [Value], route: &[String]) -> Option<&'a Value> {
    let (head, tail) = route.split_first()?;
    let row = rows.iter().find(|row| identity(row) == *head)?;
    if tail.is_empty() {
        Some(row)
    } else {
        resolve(row["placements"].as_array()?, tail)
    }
}
