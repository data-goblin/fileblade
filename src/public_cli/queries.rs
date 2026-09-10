use super::*;

pub(super) fn search(options: SearchArgs) -> AppResult<PublicResult> {
    ipc("setSearchLayout", &[options.tree.to_string()])?;
    ipc(
        "setSearchOptions",
        &[
            options.case_sensitive.to_string(),
            options.regex.to_string(),
        ],
    )?;
    let mode = if options.shallow { "shallow" } else { "deep" };
    let response = ipc("searchWith", &[options.query.clone(), mode.to_string()])?;
    if !options.wait {
        return Ok(PublicResult::one(response));
    }
    let result = poll(
        options.timeout,
        || {
            let state = object_response("status", &[])?;
            if text_field(&state, "searchQuery") != options.query
                || state
                    .get("quickNavActive")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            {
                return Err(AppError::command("Search was replaced before it completed"));
            }
            if state
                .get("searchBusy")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || nonempty(&state, "activeSearchQuery")
            {
                return Ok(None);
            }
            let result = Value::Object(object_response(
                "searchResults",
                &[options.limit.to_string()],
            )?);
            if nonempty_value(&result, "error") {
                return Err(AppError::command(value_text(&result, "error")));
            }
            Ok(Some(result))
        },
        "Timed out waiting for search",
    )?;
    Ok(PublicResult::many(vec![Value::String(response), result]))
}

pub(super) fn log(options: LogArgs) -> AppResult<PublicResult> {
    let document = crate::audit::read(
        usize::try_from(options.limit).unwrap_or(0).clamp(1, 10_000),
        &options.since,
        &options.command,
    );
    if !document["ok"].as_bool().unwrap_or(false) {
        return Err(AppError::command(value_text(&document, "error")));
    }
    let lines = document["entries"]
        .as_array()
        .map(|entries| entries.iter().map(log_line).collect::<Vec<_>>())
        .unwrap_or_default();
    Ok(PublicResult::lines(lines, document))
}

pub(super) fn list(options: ListArgs) -> AppResult<PublicResult> {
    let mut text = String::new();
    if options.from == "-" {
        io::stdin().read_to_string(&mut text)?;
    } else {
        text = std::fs::read_to_string(parse_path(&options.from)?)?;
    }
    let base = std::env::current_dir()?;
    let paths = text
        .split(['\n', '\0'])
        .filter(|line| !line.is_empty())
        .take(10_000)
        .map(|line| parse_path(line).map(|path| path_text(&base.join(path))))
        .collect::<std::io::Result<Vec<_>>>()?;
    if paths.is_empty() {
        return Err(AppError::invalid("no paths on stdin"));
    }
    let directory = crate::search::lists_directory();
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    if let Ok(entries) = std::fs::read_dir(&directory) {
        for entry in entries.flatten() {
            let stale = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .map(|modified| modified.elapsed().is_ok_and(|age| age.as_secs() > 3600))
                .unwrap_or(false);
            if stale {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    let file = directory.join(format!("list-{}-{stamp}.json", std::process::id()));
    let document = json!({"title": options.title, "base": path_text(&base), "paths": paths});
    crate::secure::write_private_atomic(&file, document.to_string().as_bytes())?;
    let response = ipc("showList", &[path_text(&file)])?;
    Ok(PublicResult::one(
        json!({"response": response, "count": document["paths"].as_array().map_or(0, Vec::len), "file": path_text(&file)}),
    ))
}

pub(super) fn modules() -> AppResult<PublicResult> {
    let document = Value::Object(object_response("bladeModules", &[])?);
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for module in document["modules"]
        .as_array()
        .into_iter()
        .flatten()
        .take(MAX_MODULE_ROWS)
    {
        let category: String = module["category"]
            .as_str()
            .unwrap_or("")
            .chars()
            .filter(|c| !c.is_control())
            .take(MAX_CATEGORY_CHARS)
            .collect();
        let category = if category.is_empty() {
            "Module".to_string()
        } else {
            category
        };
        let line = module_line(module);
        match groups.iter_mut().find(|(known, _)| *known == category) {
            Some((_, rows)) => rows.push(line),
            None => groups.push((category, vec![line])),
        }
    }
    let mut lines = Vec::new();
    for (category, rows) in groups {
        lines.push(category.to_uppercase());
        lines.extend(rows);
    }
    Ok(PublicResult::lines(lines, document))
}

const MAX_MODULE_ROWS: usize = 256;
const MAX_CATEGORY_CHARS: usize = 32;

fn module_line(module: &Value) -> String {
    let id = value_text(module, "id");
    let name = value_text(module, "name");
    let mut line = format!("  {id}");
    if !name.is_empty() && name != id {
        line.push_str(&format!("  {name}"));
    }
    if module["placed"].is_object() {
        line.push_str("  (placed)");
    }
    let description = value_text(module, "description");
    if !description.is_empty() {
        line.push_str(&format!("  {description}"));
    }
    line.chars().filter(|c| !c.is_control()).collect()
}

pub(super) fn recent(options: RecentArgs) -> AppResult<PublicResult> {
    let mut arguments = vec![
        "frecency-list".to_string(),
        "--limit".to_string(),
        options.limit.to_string(),
        "--query".to_string(),
        options.query,
    ];
    if options.show_hidden {
        arguments.push("--show-hidden".to_string());
    }
    let document = backend_json(&arguments)?;
    let lines = document["entries"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .map(|entry| value_text(entry, "path"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(PublicResult::lines(lines, document))
}

pub(super) fn log_line(entry: &Value) -> String {
    let arguments = entry["arguments"]
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(crate::shell_init::shell_quote)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    format!(
        "{}  {}  {}/{}  {}  {}{}",
        value_text(entry, "ts"),
        if entry["ok"].as_bool().unwrap_or(false) {
            "ok "
        } else {
            "ERR"
        },
        value_text(entry, "via"),
        value_text(entry, "actor"),
        value_text(entry, "command"),
        arguments,
        entry["error"]
            .as_str()
            .filter(|error| !error.is_empty())
            .map(|error| format!("  ({error})"))
            .unwrap_or_default()
    )
}

pub(super) fn pick(options: PickArgs) -> AppResult<PublicResult> {
    let mode = match options.mode {
        PickMode::Open => "open",
        PickMode::Save => "save",
        PickMode::Folder => "folder",
    };
    let request = json!({
        "mode": mode,
        "multiple": options.multiple,
        "extensions": options.extensions,
        "title": options.title,
        "suggestedName": options.suggested_name,
        "root": options.root.as_deref().map(absolute_path),
    });
    let id = ipc("pick", &[request.to_string()])?;
    match options.query.as_deref().filter(|query| !query.is_empty()) {
        Some(query) => {
            ipc("search", &[query.to_string()])?;
            ipc("focusSearch", &[])?;
        }
        None => {
            ipc("focusTree", &[])?;
        }
    }
    let result = poll(
        options.timeout,
        || {
            let state = object_response("pickerResult", std::slice::from_ref(&id))?;
            match text_field(&state, "status").as_str() {
                "pending" => Ok(None),
                "accepted" => Ok(Some(Value::Object(state))),
                status => Err(AppError::command(format!("picker {status}"))),
            }
        },
        "Timed out waiting for the picker",
    )?;
    let paths = result["paths"]
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let lines = if options.shell_quote {
        vec![
            paths
                .iter()
                .map(|path| crate::shell_init::shell_quote(path))
                .collect::<Vec<_>>()
                .join(" "),
        ]
    } else {
        paths.clone()
    };
    Ok(PublicResult::lines(
        lines,
        json!({"mode": mode, "paths": paths}),
    ))
}
