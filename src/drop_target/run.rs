use super::*;

pub fn drop_run(options: &DropRunOptions) -> Value {
    let facts = file_facts(&options.paths);
    let mut runner = Runner::new(options.dry_run);
    let target = parse_target(&options.target_json);
    if facts.get("count").and_then(Value::as_u64).unwrap_or(0) == 0 {
        return json!({
            "ok": false,
            "error": "Nothing to drop",
            "action": options.action,
            "commands": [],
        });
    }
    if let Err(error) = revalidate_target(&target) {
        return json!({
            "ok": false,
            "error": error.to_string(),
            "action": options.action,
            "placement": options.placement,
            "commands": [],
            "dry_run": options.dry_run,
        });
    }
    let result = if options.action == "configured" {
        run_configured(&mut runner, &options.placement, &target, &facts)
    } else if options.action == "application" {
        run_application(&mut runner, &options.desktop_id, &facts)
    } else if options.action == "review" {
        run_review(&mut runner, &options.placement, &target, &facts)
    } else if matches!(
        options.action.as_str(),
        "open" | "review" | "terminal" | "copy-paths"
    ) {
        run_generic(&mut runner, &options.action, &facts)
    } else {
        run_target(
            &mut runner,
            &options.action,
            &options.placement,
            &target,
            &facts,
        )
    };
    extend_result(
        result.unwrap_or_else(|error| json!({"ok": false, "error": error.to_string()})),
        json!({
            "action": options.action,
            "placement": options.placement,
            "commands": runner.commands,
            "dry_run": options.dry_run,
        }),
    )
}

fn run_configured(
    runner: &mut Runner,
    route: &str,
    target: &Value,
    facts: &Value,
) -> AppResult<Value> {
    if route.len() > 1024 {
        return Err(AppError::invalid("Invalid configured action route"));
    }
    let route: Vec<String> = serde_json::from_str(route)?;
    if route.is_empty() || route.len() > 3 {
        return Err(AppError::invalid("Invalid configured action depth"));
    }
    let (rows, _) = config::load(&actions_for(target, facts), target, facts);
    let row = config::resolve(&rows, &route)
        .ok_or_else(|| AppError::invalid("This configured action is no longer available"))?;
    if row["placements"]
        .as_array()
        .is_some_and(|rows| !rows.is_empty())
    {
        return Err(AppError::invalid("Choose a placement first"));
    }
    if row["configured_builtin"] == true {
        let action = text_field(row, "builtin_action");
        let placement = text_field(row, "builtin_placement");
        return match action.as_str() {
            "application" => run_application(runner, &text_field(row, "desktop_id"), facts),
            "review" => run_review(runner, &placement, target, facts),
            "open" | "terminal" | "copy-paths" => run_generic(runner, &action, facts),
            _ => run_target(runner, &action, &placement, target, facts),
        };
    }
    let command = config::expand(&row["command"], facts)?;
    let cwd = text_field(facts, "folder");
    let folder = parse_path(&cwd)?;
    let initial_folder = folder.ancestors().find_map(Path::to_str).unwrap_or("/");
    match text_field(row, "runMode").as_str() {
        "detached" => runner.detached(command, Some(&cwd))?,
        "terminal" => {
            let mut launch = vec![
                OsString::from("xdg-terminal-exec"),
                native_directory_arg(initial_folder)?,
                OsString::from("-e"),
            ];
            launch.extend(
                configured_launcher(&command, &cwd)?
                    .into_iter()
                    .map(OsString::from),
            );
            runner.detached(wrapped(launch), None)?;
        }
        "multiplexer" => {
            if let Some(reason) = ambiguous_reason(target) {
                return Err(AppError::invalid(reason));
            }
            revalidate_title_target(target).map_err(AppError::invalid)?;
            let command = configured_launcher(&command, &cwd)?;
            let result = run_multiplexer_command(
                runner,
                &text_field(row, "placement"),
                target,
                initial_folder,
                &command,
            )?;
            if result["ok"] == true {
                focus_target(runner, target)?;
            }
            return Ok(result);
        }
        _ => return Err(AppError::invalid("Invalid configured run mode")),
    }
    Ok(json!({"ok": true, "error": ""}))
}

pub fn drop_paste(options: &DropPasteOptions) -> Value {
    let context = drop_context(options.x, options.y, &options.paths, &options.blade_titles);
    if !context.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        return json!({
            "ok": false,
            "error": context.get("error").and_then(Value::as_str).unwrap_or("Could not resolve the drop target"),
            "form": options.form,
            "commands": [],
        });
    }
    let mut runner = Runner::new(options.dry_run);
    let target = context
        .get("target")
        .cloned()
        .unwrap_or_else(|| json!({"kind": "desktop"}));
    let facts = context.get("files").cloned().unwrap_or_else(|| json!({}));
    let result = run_paste(&mut runner, &target, &facts, &options.form)
        .unwrap_or_else(|error| json!({"ok": false, "error": error.to_string()}));
    extend_result(
        result,
        json!({
            "form": options.form,
            "target": target.get("label").and_then(Value::as_str).unwrap_or_default(),
            "commands": runner.commands,
            "dry_run": options.dry_run,
        }),
    )
}

pub(super) fn run_target(
    runner: &mut Runner,
    action: &str,
    placement: &str,
    target: &Value,
    facts: &Value,
) -> AppResult<Value> {
    if let Some(reason) = ambiguous_reason(target) {
        return Ok(
            json!({"ok": false, "error": format!("{reason}; open a new terminal window instead")}),
        );
    }
    if action == "mux-open" {
        if let Err(reason) = revalidate_title_target(target) {
            return Ok(
                json!({"ok": false, "error": format!("{reason}; open a new terminal window instead")}),
            );
        }
        let result = run_multiplexer(runner, placement, target, facts)?;
        if result.get("ok").and_then(Value::as_bool) == Some(true) {
            focus_target(runner, target)?;
        }
        return Ok(result);
    }
    match action {
        "nvim-open" => run_nvim(runner, placement, target, facts),
        "app-open" => {
            let result = run_application(
                runner,
                target
                    .get("app")
                    .and_then(|app| app.get("desktop_id"))
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                facts,
            )?;
            if result.get("ok").and_then(Value::as_bool) == Some(true) {
                focus_target(runner, target)?;
            }
            Ok(result)
        }
        _ => Ok(json!({"ok": false, "error": format!("Unknown drop action: {action}")})),
    }
}

pub(super) fn run_generic(runner: &mut Runner, action: &str, facts: &Value) -> AppResult<Value> {
    let paths = string_array(facts, "paths");
    let folder = text_field(facts, "folder");
    match action {
        "open" => {
            for path in paths {
                runner.detached(launch_command(&parse_path(&path)?, "default", "", 0)?, None)?;
            }
            Ok(json!({"ok": true, "error": ""}))
        }
        "review" => {
            let cwd = review_cwd(facts);
            let mut command = vec![
                OsString::from("xdg-terminal-exec"),
                OsString::from("--app-id=org.omarchy.hunk"),
                native_directory_arg(&cwd)?,
                OsString::from("-e"),
            ];
            command.extend(
                review_command(facts)
                    .into_iter()
                    .map(|value| {
                        if value.starts_with("file://") {
                            parse_path(&value).map(PathBuf::into_os_string)
                        } else {
                            Ok(value.into())
                        }
                    })
                    .collect::<std::io::Result<Vec<_>>>()?,
            );
            runner.detached(wrapped(command), None)?;
            Ok(json!({"ok": true, "error": ""}))
        }
        "terminal" => {
            let mut command = vec![
                OsString::from("xdg-terminal-exec"),
                native_directory_arg(&folder)?,
            ];
            if has_files(facts) {
                command.extend([
                    OsString::from("-e"),
                    OsString::from("nvim"),
                    OsString::from("--"),
                ]);
                command.extend(
                    string_array(facts, "files")
                        .iter()
                        .map(|path| parse_path(path).map(PathBuf::into_os_string))
                        .collect::<std::io::Result<Vec<_>>>()?,
                );
            }
            runner.detached(wrapped(command), None)?;
            Ok(json!({"ok": true, "error": ""}))
        }
        "copy-paths" => {
            let (code, _, error) = runner.capture_input(
                vec![
                    "wl-copy".to_string(),
                    "--type".to_string(),
                    "text/plain".to_string(),
                ],
                None,
                Some(quoted_paths(&paths).into_bytes()),
            )?;
            Ok(json!({"ok": code == 0, "error": error.trim()}))
        }
        _ => Ok(json!({"ok": false, "error": format!("Unknown drop action: {action}")})),
    }
}

pub(super) fn run_multiplexer(
    runner: &mut Runner,
    placement: &str,
    target: &Value,
    facts: &Value,
) -> AppResult<Value> {
    run_multiplexer_command(
        runner,
        placement,
        target,
        &text_field(facts, "folder"),
        &multiplexer_command(facts),
    )
}

fn run_review(
    runner: &mut Runner,
    placement: &str,
    target: &Value,
    facts: &Value,
) -> AppResult<Value> {
    if !review_possible(facts) {
        return Ok(json!({"ok": false, "error": "No Git status changes in the selected paths"}));
    }
    if matches!(placement, "" | "window") {
        return run_generic(runner, "review", facts);
    }
    if let Some(reason) = ambiguous_reason(target) {
        return Ok(
            json!({"ok": false, "error": format!("{reason}; open a new terminal window instead")}),
        );
    }
    if let Err(reason) = revalidate_title_target(target) {
        return Ok(
            json!({"ok": false, "error": format!("{reason}; open a new terminal window instead")}),
        );
    }
    let Some(multiplexer) = resolved_multiplexer(target) else {
        return Ok(json!({"ok": false, "error": "This target has no resolved multiplexer"}));
    };
    let destination = match (multiplexer, placement) {
        (_, "pane" | "right" | "down") => placement,
        ("tmux", "tab") => "window",
        ("tmux", "workspace") => "session",
        (_, "tab" | "workspace") => placement,
        _ => return Ok(json!({"ok": false, "error": "Unknown review placement"})),
    };
    let result = run_multiplexer_command(
        runner,
        destination,
        target,
        &review_cwd(facts),
        &review_command(facts),
    )?;
    if result["ok"] == true {
        focus_target(runner, target)?;
    }
    Ok(result)
}

fn run_multiplexer_command(
    runner: &mut Runner,
    placement: &str,
    target: &Value,
    cwd: &str,
    command: &[String],
) -> AppResult<Value> {
    match target
        .get("terminal")
        .and_then(|terminal| terminal.get("multiplexer"))
        .and_then(Value::as_str)
        .unwrap_or("none")
    {
        "herdr" => run_herdr(runner, placement, target, cwd, command),
        "tmux" => run_tmux(runner, placement, target, cwd, command),
        _ => Ok(json!({"ok": false, "error": "This terminal runs no supported multiplexer"})),
    }
}

pub(super) fn run_paste(
    runner: &mut Runner,
    target: &Value,
    facts: &Value,
    form: &str,
) -> AppResult<Value> {
    let text = paste_text(target, facts, form);
    if let Some(reason) = ambiguous_reason(target) {
        return Ok(
            json!({"ok": false, "error": format!("{reason}; open a new terminal window instead"), "text": text}),
        );
    }
    if target.get("kind").and_then(Value::as_str) != Some("terminal")
        && text.chars().any(|character| character.is_control())
    {
        return Ok(json!({
            "ok": false,
            "error": "a path contains control characters; refusing to type it into a window",
            "text": text,
        }));
    }
    let terminal = target.get("terminal").unwrap_or(&Value::Null);
    let target_focused = focus_target(runner, target)?;
    match terminal
        .get("multiplexer")
        .and_then(Value::as_str)
        .unwrap_or("none")
    {
        "herdr" if !text_field(terminal, "pane_id").is_empty() => {
            if let Err(reason) = revalidate_title_target(target) {
                return Ok(json!({"ok": false, "error": reason, "text": text}));
            }
            let (ok, _, error) = herdr_call(
                runner,
                &["pane", "send-text", &text_field(terminal, "pane_id"), &text],
                &text_field(terminal, "socket"),
            )?;
            return Ok(json!({"ok": ok, "error": if ok { "" } else { &error }, "text": text}));
        }
        "tmux" if !text_field(terminal, "session").is_empty() => {
            let mut command = tmux_command(&text_field(terminal, "socket"));
            command.extend([
                "send-keys".to_string(),
                "-t".to_string(),
                text_field(terminal, "session"),
                "-l".to_string(),
                "--".to_string(),
                text.clone(),
            ]);
            let (code, _, error) = runner.capture(native_tmux_arguments(command)?, None)?;
            return Ok(json!({"ok": code == 0, "error": error.trim(), "text": text}));
        }
        _ => {}
    }
    if which("wtype").is_none() && !runner.dry_run {
        return Ok(json!({"ok": false, "error": "wtype is not installed", "text": text}));
    }
    if target_focused {
        runner.pause(Duration::from_millis(80));
    }
    let (code, _, error) = runner.capture(
        vec!["wtype".to_string(), "--".to_string(), text.clone()],
        None,
    )?;
    Ok(json!({"ok": code == 0, "error": error.trim(), "text": text}))
}

pub(super) fn run_application(
    runner: &mut Runner,
    desktop_id: &str,
    facts: &Value,
) -> AppResult<Value> {
    if let Err(error) = validate_desktop_id(desktop_id) {
        return Ok(json!({"ok": false, "error": error.to_string()}));
    }
    let mut command = vec!["gtk-launch".to_string(), desktop_id.to_string()];
    command.extend(application_arguments(desktop_id, facts));
    runner.detached(command, None)?;
    Ok(json!({"ok": true, "error": ""}))
}

pub(super) fn application_arguments(desktop_id: &str, facts: &Value) -> Vec<String> {
    string_array(facts, "paths")
        .into_iter()
        .filter_map(|path| {
            let path = parse_path(&path).ok()?;
            if desktop_id.to_lowercase().starts_with("obsidian") {
                Some(format!("obsidian://open?path={}", encode_path(&path)))
            } else {
                Url::from_file_path(path).ok().map(|url| url.to_string())
            }
        })
        .collect()
}

pub(super) fn paste_text(target: &Value, facts: &Value, form: &str) -> String {
    let base = target
        .get("terminal")
        .and_then(|terminal| terminal.get("cwd"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            facts
                .get("git_root")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| text_field(facts, "folder"));
    let paths = string_array(facts, "paths")
        .into_iter()
        .filter_map(|path| parse_path(&path).ok())
        .map(|path| {
            if form == "relative" && !base.is_empty() {
                parse_path(&base)
                    .ok()
                    .and_then(|base| relative_path(&base, &path))
                    .unwrap_or(path)
            } else {
                path
            }
        })
        .collect::<Vec<_>>();
    if target.get("kind").and_then(Value::as_str) == Some("terminal") {
        format!(
            "{} ",
            paths
                .iter()
                .map(|path| shell_quote_native(path.as_os_str()))
                .collect::<Vec<_>>()
                .join(" ")
        )
    } else {
        paths
            .iter()
            .map(|path| {
                if path.is_absolute() {
                    path_text(path)
                } else {
                    command_text(path.as_os_str())
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}
