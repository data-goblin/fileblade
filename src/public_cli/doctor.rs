use super::*;

pub(super) fn doctor() -> AppResult<PublicResult> {
    let binary = own_binary().unwrap_or_default();
    let manifest = binary
        .ancestors()
        .skip(1)
        .take(4)
        .map(|directory| directory.join("manifest.json"))
        .find(|candidate| candidate.is_file());
    let manifest_version = manifest.as_ref().and_then(|path| {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|value| value["version"].as_str().map(str::to_string))
    });
    let backend_version = env!("CARGO_PKG_VERSION").to_string();
    let version_skew = manifest_version
        .as_ref()
        .is_some_and(|version| version != &backend_version);
    let shell = match ipc("status", &[]) {
        Ok(text) => {
            let status = response_value(&text);
            json!({"ok": true, "root": status["rootPath"], "open": status["open"]})
        }
        Err(error) => json!({"ok": false, "error": error.to_string()}),
    };
    let inflight = std::fs::read_dir(crate::recovery::inflight_dir())
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
                .count()
        })
        .unwrap_or(0);
    let serve = doctor_handshake(&binary);
    let recovery_failed = serve["recovered"]["ok"] == false;
    let ok = !version_skew && shell["ok"] == true && serve["ok"] == true && !recovery_failed;
    let mut advice = Vec::new();
    let native = crate::lease::selected_root()?.is_some();
    if version_skew {
        advice.push(
            if native {
                "update or reinstall FileBlade, then start FileBlade again"
            } else {
                "update or reinstall FileBlade, then run omarchy restart shell"
            }
            .to_string(),
        );
    }
    if shell["ok"] != true {
        advice.push(
            if native {
                "FileBlade is not answering; start FileBlade or inspect its native view log"
            } else {
                "omarchy-shell is not answering; restart the shell or enable the plugin"
            }
            .to_string(),
        );
    }
    if inflight > 0 {
        advice.push(format!(
            "{inflight} operation intent(s) exist; recovery skips active operations"
        ));
    }
    if recovery_failed {
        advice.push(format!(
            "startup recovery is blocked: {}",
            serve["recovered"]["error"]
                .as_str()
                .unwrap_or("inspect the recovery result and native authority log")
        ));
    }
    Ok(PublicResult::checked(
        json!({
            "ok": ok,
            "binary": binary,
            "backend_version": backend_version,
            "manifest": manifest,
            "manifest_version": manifest_version,
            "version_skew": version_skew,
            "serve": serve,
            "shell": shell,
            "inflight": inflight,
            "advice": advice,
        }),
        "fileblade is not healthy",
    ))
}

pub(super) fn doctor_handshake(binary: &std::path::Path) -> Value {
    use std::io::{BufRead, BufReader, Write};
    let spawned = std::process::Command::new(binary)
        .args(["serve", "--max-concurrency", "1", "--no-recover"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => return json!({"ok": false, "error": error.to_string()}),
    };
    let mut stdin = child.stdin.take();
    if let Some(stdin) = stdin.as_mut() {
        let _ = stdin.write_all(b"{\"v\":1,\"type\":\"hello\"}\n");
        let _ = stdin.flush();
    }
    let mut line = String::new();
    if let Some(stdout) = child.stdout.take() {
        let _ = BufReader::new(stdout).read_line(&mut line);
    }
    drop(stdin);
    let _ = child.wait();
    match serde_json::from_str::<Value>(line.trim()) {
        Ok(hello) if hello["ok"] == true => json!({
            "ok": true,
            "limits": hello["limits"],
            "recovered": hello["recovered"],
        }),
        Ok(hello) => json!({"ok": false, "error": hello["error"]}),
        Err(error) => json!({"ok": false, "error": format!("no hello from serve: {error}")}),
    }
}
