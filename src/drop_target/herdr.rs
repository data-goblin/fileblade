use super::*;

pub(super) fn run_herdr(
    runner: &mut Runner,
    placement: &str,
    target: &Value,
    cwd: &str,
    command: &[String],
) -> AppResult<Value> {
    let terminal = target.get("terminal").unwrap_or(&Value::Null);
    let folder = parse_path(cwd)?;
    let initial_folder = folder.ancestors().find_map(Path::to_str).unwrap_or("/");
    let (pane_id, error) = herdr_new_pane(runner, terminal, placement, initial_folder)?;
    if pane_id.is_empty() {
        return Ok(
            json!({"ok": false, "error": if error.is_empty() { "herdr could not create the pane" } else { &error }}),
        );
    }
    let mut script = quoted_paths(command);
    if folder.to_str().is_none() {
        let cd = format!("cd -- {}", shell_quote_native(folder.as_os_str()));
        script = if script.is_empty() {
            cd
        } else {
            format!("{cd} && {script}")
        };
    }
    let error = if script.is_empty() {
        String::new()
    } else {
        herdr_run(runner, &pane_id, &script, &text_field(terminal, "socket"))?
    };
    Ok(json!({"ok": error.is_empty(), "error": error, "pane_id": pane_id}))
}

pub(super) fn herdr_new_pane(
    runner: &mut Runner,
    terminal: &Value,
    placement: &str,
    cwd: &str,
) -> AppResult<(String, String)> {
    let pane_id = text_field(terminal, "pane_id");
    let workspace_id = text_field(terminal, "workspace_id");
    let socket = text_field(terminal, "socket");
    let arguments = match placement {
        "tab" => vec![
            "tab",
            "create",
            "--workspace",
            &workspace_id,
            "--cwd",
            cwd,
            "--focus",
        ],
        "workspace" => vec!["workspace", "create", "--cwd", cwd, "--focus"],
        _ => {
            let direction = match placement {
                "right" | "down" => placement.to_string(),
                _ => herdr_split_direction(runner, &pane_id, &socket)?,
            };
            return herdr_call(
                runner,
                &[
                    "pane",
                    "split",
                    "--pane",
                    &pane_id,
                    "--direction",
                    &direction,
                    "--cwd",
                    cwd,
                    "--focus",
                ],
                &socket,
            )
            .map(|(ok, output, error)| {
                (
                    if ok {
                        herdr_result_text(&output, &["result", "pane", "pane_id"]).unwrap_or_else(
                            || {
                                if runner.dry_run {
                                    "w0:p0".to_string()
                                } else {
                                    String::new()
                                }
                            },
                        )
                    } else {
                        String::new()
                    },
                    error,
                )
            });
        }
    };
    let (ok, output, error) = herdr_call(runner, &arguments, &socket)?;
    Ok((
        if ok {
            herdr_result_text(&output, &["result", "root_pane", "pane_id"]).unwrap_or_else(|| {
                if runner.dry_run {
                    "w0:p0".to_string()
                } else {
                    String::new()
                }
            })
        } else {
            String::new()
        },
        error,
    ))
}

pub(super) fn herdr_split_direction(
    runner: &mut Runner,
    pane_id: &str,
    socket: &str,
) -> AppResult<String> {
    let (ok, output, _) = herdr_call(runner, &["pane", "layout", "--pane", pane_id], socket)?;
    if !ok {
        return Ok("right".to_string());
    }
    let value: Value = serde_json::from_str(&output).unwrap_or(Value::Null);
    let panes = value
        .pointer("/result/layout/panes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let rectangle = panes
        .iter()
        .find(|pane| text_field(pane, "pane_id") == pane_id)
        .and_then(|pane| pane.get("rect"))
        .cloned()
        .unwrap_or(Value::Null);
    let width = rectangle
        .get("width")
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        * 0.5;
    let height = rectangle
        .get("height")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    Ok(if width > 0.0 && width < height {
        "down"
    } else {
        "right"
    }
    .to_string())
}

pub(super) fn herdr_run(
    runner: &mut Runner,
    pane_id: &str,
    command: &str,
    socket: &str,
) -> AppResult<String> {
    let mut last_error = String::new();
    for _ in 0..HERDR_RUN_ATTEMPTS {
        let (ok, _, error) = herdr_call(runner, &["pane", "run", pane_id, command], socket)?;
        if ok || runner.dry_run {
            return Ok(String::new());
        }
        last_error = error;
        runner.pause(Duration::from_millis(150));
    }
    Ok(if last_error.is_empty() {
        "herdr did not accept the command".to_string()
    } else {
        last_error
    })
}

pub(super) fn herdr_call(
    runner: &mut Runner,
    arguments: &[&str],
    socket: &str,
) -> AppResult<(bool, String, String)> {
    let mut environment = std::env::vars_os()
        .filter(|(key, _)| !key.as_bytes().starts_with(b"HERDR_"))
        .collect::<BTreeMap<_, _>>();
    if !socket.is_empty() {
        environment.insert(
            "HERDR_SOCKET_PATH".into(),
            parse_path(socket)?.into_os_string(),
        );
    }
    let mut command = vec!["herdr".to_string()];
    command.extend(arguments.iter().map(|value| value.to_string()));
    let (code, output, error) = runner.capture(command, Some(environment))?;
    Ok((
        code == 0,
        output.clone(),
        if error.trim().is_empty() {
            output.trim().to_string()
        } else {
            error.trim().to_string()
        },
    ))
}

pub(super) fn herdr_result_text(output: &str, keys: &[&str]) -> Option<String> {
    let mut value = serde_json::from_str::<Value>(output).ok()?;
    for key in keys {
        value = value.get(*key)?.clone();
    }
    value.as_str().map(str::to_string)
}
