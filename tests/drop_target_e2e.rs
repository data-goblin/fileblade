use fileblade::drop_target::{self, DropRunOptions};
use serde_json::Value;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::tempdir;

#[test]
fn hunk_review_routes_each_destination_with_repository_cwd_and_quoted_paths() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .arg(root)
            .status()
            .unwrap()
            .success()
    );
    fs::create_dir(root.join("nested")).unwrap();
    let path = root.join("nested/quote' and space.txt");
    fs::write(&path, "changed").unwrap();
    for mux in ["herdr", "tmux"] {
        let target = serde_json::json!({"kind": "terminal", "terminal": {
            "multiplexer": mux, "pane_id": "w1:p1", "workspace_id": "w1",
            "session": "$1", "tty": "/dev/pts/10", "socket": "/fixture/mux.sock"
        }});
        for placement in ["pane", "tab", "workspace", "window", ""] {
            let result = drop_target::drop_run(&DropRunOptions {
                action: "review".into(),
                placement: placement.into(),
                paths: vec![path.to_string_lossy().into_owned()],
                target_json: target.to_string(),
                desktop_id: String::new(),
                dry_run: true,
            });
            assert_eq!(result["ok"], true, "{mux}/{placement}: {result}");
            let commands = result["commands"].as_array().unwrap();
            if matches!(placement, "" | "window") {
                let command = strings(&commands[0]);
                assert!(command.contains(&"xdg-terminal-exec"));
                assert!(command.contains(&format!("--dir={}", root.display()).as_str()));
                assert_eq!(
                    &command[command.len() - 4..],
                    ["hunk", "diff", "--", path.to_str().unwrap()]
                );
            } else {
                let expected = match (mux, placement) {
                    ("herdr", "pane") => "split",
                    ("herdr", _) => "create",
                    (_, "pane") => "split-window",
                    (_, "tab") => "new-window",
                    _ => "new-session",
                };
                assert!(
                    commands.iter().any(|c| strings(c).contains(&expected)),
                    "{result}"
                );
                assert!(
                    commands
                        .iter()
                        .any(|c| strings(c).contains(&root.to_str().unwrap())),
                    "{result}"
                );
                let script = commands
                    .iter()
                    .flat_map(strings)
                    .find(|s| s.contains("hunk"))
                    .unwrap();
                let output = Command::new("bash")
                    .args([
                        "-c",
                        &format!("hunk() {{ printf '%s\\0' \"$@\"; }}; {script}"),
                    ])
                    .output()
                    .unwrap();
                assert!(output.status.success());
                let args: Vec<_> = output
                    .stdout
                    .split(|b| *b == 0)
                    .filter(|s| !s.is_empty())
                    .collect();
                assert_eq!(
                    args,
                    [b"diff".as_slice(), b"--", path.as_os_str().as_bytes()]
                );
            }
        }
    }
}

#[test]
fn hunk_review_refuses_unresolved_destinations_and_non_git_file_pairs() {
    let temporary = tempdir().unwrap();
    let paths: Vec<_> = ["before.txt", "after.txt"]
        .iter()
        .map(|name| {
            let path = temporary.path().join(name);
            fs::write(&path, name).unwrap();
            path.to_string_lossy().into_owned()
        })
        .collect();
    for target in [
        serde_json::json!({"kind": "desktop"}),
        serde_json::json!({"kind": "terminal", "terminal": {"multiplexer": "none"}}),
        serde_json::json!({"kind": "terminal", "terminal": {"multiplexer": "herdr"}}),
    ] {
        for placement in ["pane", "tab", "workspace"] {
            let result = drop_target::drop_run(&DropRunOptions {
                action: "review".into(),
                placement: placement.into(),
                paths: paths.clone(),
                target_json: target.to_string(),
                desktop_id: String::new(),
                dry_run: true,
            });
            assert_eq!(result["ok"], false, "{result}");
            assert_eq!(result["commands"], serde_json::json!([]));
        }
    }
    let result = drop_target::drop_run(&DropRunOptions {
        action: "review".into(),
        placement: String::new(),
        paths: paths.clone(),
        target_json: String::new(),
        desktop_id: String::new(),
        dry_run: true,
    });
    assert_eq!(result["ok"], false, "{result}");
    assert_eq!(result["commands"], serde_json::json!([]));
}

#[test]
fn nvim_split_keeps_hostile_filenames_out_of_remote_send_commands() {
    let temporary = tempdir().unwrap();
    let names = [
        "pipe|let g:pwned=1|.txt",
        "double\"-single'.txt",
        "back\\slash.txt",
        "line\nbreak.txt",
        "-leading-dash.txt",
    ];
    let paths: Vec<_> = names
        .iter()
        .map(|name| {
            let path = temporary.path().join(name);
            fs::write(&path, "body").unwrap();
            path.to_string_lossy().into_owned()
        })
        .collect();
    let target = serde_json::json!({
        "kind": "editor",
        "address": "",
        "editor": {"kind": "nvim", "server": "test-server"}
    });
    let result = drop_target::drop_run(&DropRunOptions {
        action: "nvim-open".to_string(),
        placement: "split".to_string(),
        paths: paths.clone(),
        target_json: target.to_string(),
        desktop_id: String::new(),
        dry_run: true,
    });

    assert_eq!(result["ok"], true, "{result}");
    let commands = result["commands"].as_array().unwrap();
    assert_eq!(commands.len(), 1);
    let command = strings(&commands[0]);
    assert_eq!(
        &command[..4],
        ["nvim", "--server", "test-server", "--remote-expr"]
    );
    let encoded_paths = paths
        .iter()
        .map(|path| {
            format!(
                "\"{}\"",
                path.bytes()
                    .map(|byte| format!("\\x{byte:02x}"))
                    .collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        command[4],
        format!(
            "luaeval('(function(paths) local function drop(path, split, tab) local buffer = vim.fn.bufadd(path); local windows = vim.fn.win_findbuf(buffer); if #windows > 0 then vim.api.nvim_set_current_win(windows[1]) else if tab then vim.cmd.tabnew() elseif split or (vim.bo.modified and not vim.o.hidden) then vim.cmd.vsplit() end; vim.api.nvim_set_current_buf(buffer) end end; for index, path in ipairs(paths) do drop(path, index > 1, false) end; return 0; end)(_A)', [{encoded_paths}])"
        )
    );
    assert!(!command[4].contains('\n'));
    assert!(!command.contains(&"--remote-send"));
}

#[test]
fn nvim_opens_byte_and_unicode_spellings_as_distinct_buffers() {
    let temporary = tempdir().unwrap();
    let paths = [
        temporary.path().join(OsStr::from_bytes(b"\xff.txt")),
        temporary.path().join("�.txt"),
    ];
    for (index, path) in paths.iter().enumerate() {
        fs::write(path, format!("spelling {index}")).unwrap();
    }
    let socket = temporary.path().join("nvim.sock");
    let Some(_server) = start_nvim(&socket) else {
        return;
    };
    let target = serde_json::json!({"kind": "editor", "address": "", "editor": {"kind": "nvim", "server": socket}});
    let result = drop_target::drop_run(&DropRunOptions {
        action: "nvim-open".to_string(),
        placement: "split".to_string(),
        paths: paths
            .iter()
            .map(|path| fileblade::common::path_text(path))
            .collect(),
        target_json: target.to_string(),
        desktop_id: String::new(),
        dry_run: false,
    });
    assert_eq!(result["ok"], true, "{result}");
    let mut names = query_strings(
        &socket,
        "json_encode(luaeval(\"(function() local names = {}; for _, buffer in ipairs(vim.api.nvim_list_bufs()) do local name = vim.api.nvim_buf_get_name(buffer); if name ~= '' then names[#names + 1] = (name:gsub('.', function(c) return string.format('%02x', string.byte(c)) end)); end; end; return names; end)()\"))",
    );
    let mut expected = paths
        .iter()
        .map(|path| {
            path.as_os_str()
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    names.sort();
    expected.sort();
    assert_eq!(names, expected);
}

#[test]
fn nvim_server_opens_each_hostile_filename_without_a_sentinel_buffer() {
    let temporary = tempdir().unwrap();
    let names = [
        "pipe|let g:pwned=1|.txt",
        "double\"-single'.txt",
        "back\\slash.txt",
        "line\nbreak.txt",
        "-leading-dash.txt",
    ];
    let paths: Vec<_> = names
        .iter()
        .map(|name| {
            let path = temporary.path().join(name);
            fs::write(&path, "body").unwrap();
            path.to_string_lossy().into_owned()
        })
        .collect();
    let socket = temporary.path().join("nvim.sock");
    let Some(mut server) = start_nvim(&socket) else {
        return;
    };

    let target = serde_json::json!({
        "kind": "editor",
        "address": "",
        "editor": {"kind": "nvim", "server": socket}
    });
    let result = drop_target::drop_run(&DropRunOptions {
        action: "nvim-open".to_string(),
        placement: "split".to_string(),
        paths: paths.clone(),
        target_json: target.to_string(),
        desktop_id: String::new(),
        dry_run: false,
    });
    assert_eq!(result["ok"], true, "{result}");

    let mut expected = paths.clone();
    expected.sort();
    let mut buffers = query_strings(
        &socket,
        "json_encode(map(filter(getbufinfo(), {_, value -> !empty(value.name)}), {_, value -> fnamemodify(value.name, ':p')}))",
    );
    buffers.sort();
    assert_eq!(buffers, expected);
    let mut windows = query_strings(
        &socket,
        "json_encode(map(getwininfo(), {_, value -> empty(bufname(value.bufnr)) ? '' : fnamemodify(bufname(value.bufnr), ':p')}))",
    );
    windows.sort();
    assert_eq!(windows, expected);
    assert_eq!(remote_expression(&socket, "get(g:, 'pwned', 0)"), "0");
    server.0.kill().unwrap();
    server.0.wait().unwrap();
}

#[test]
fn nvim_buffer_and_split_preserve_a_modified_current_buffer() {
    for placement in ["buffer", "split"] {
        let temporary = tempdir().unwrap();
        let original = temporary.path().join("modified.txt");
        let dropped = temporary.path().join("dropped.txt");
        fs::write(&original, "saved").unwrap();
        fs::write(&dropped, "dropped").unwrap();
        let socket = temporary.path().join("nvim.sock");
        let Some(mut server) = start_nvim(&socket) else {
            return;
        };
        let original = original.to_string_lossy().into_owned();
        let encoded_original = serde_json::to_string(&original)
            .unwrap()
            .replace('\'', "''");
        assert_eq!(
            remote_expression(
                &socket,
                &format!(
                    "luaeval('(function(path) vim.api.nvim_set_current_buf(vim.fn.bufadd(path)); return 0; end)(_A)', json_decode('{encoded_original}'))"
                )
            ),
            "0"
        );
        assert_eq!(remote_expression(&socket, "execute('set nohidden')"), "");
        assert_eq!(remote_expression(&socket, "&hidden"), "0");
        assert_eq!(remote_expression(&socket, "setline(1, 'unsaved')"), "0");
        assert_eq!(remote_expression(&socket, "&modified"), "1");
        let initial_buffers = query_strings(
            &socket,
            "json_encode(map(filter(getbufinfo(), {_, value -> !empty(value.name)}), {_, value -> fnamemodify(value.name, ':p')}))",
        );
        assert_eq!(initial_buffers.len(), 1);
        assert_eq!(initial_buffers[0], original);

        let target = serde_json::json!({
            "kind": "editor",
            "address": "",
            "editor": {"kind": "nvim", "server": socket}
        });
        let dropped = dropped.to_string_lossy().into_owned();
        let result = drop_target::drop_run(&DropRunOptions {
            action: "nvim-open".to_string(),
            placement: placement.to_string(),
            paths: vec![dropped.clone()],
            target_json: target.to_string(),
            desktop_id: String::new(),
            dry_run: false,
        });
        assert_eq!(result["ok"], true, "{placement}: {result}");

        let mut expected = vec![original.clone(), dropped];
        expected.sort();
        let mut buffers = query_strings(
            &socket,
            "json_encode(map(filter(getbufinfo(), {_, value -> !empty(value.name)}), {_, value -> fnamemodify(value.name, ':p')}))",
        );
        buffers.sort();
        assert_eq!(buffers, expected, "{placement}");
        let mut windows = query_strings(
            &socket,
            "json_encode(map(getwininfo(), {_, value -> empty(bufname(value.bufnr)) ? '' : fnamemodify(bufname(value.bufnr), ':p')}))",
        );
        windows.sort();
        assert_eq!(windows, expected, "{placement}");
        let changed: Value = serde_json::from_str(&remote_expression(
            &socket,
            "json_encode(map(filter(getbufinfo(), {_, value -> value.changed}), {_, value -> [fnamemodify(value.name, ':p'), getbufline(value.bufnr, 1, '$')]}))",
        ))
        .unwrap();
        assert_eq!(changed, serde_json::json!([[original, ["unsaved"]]]));
        server.0.kill().unwrap();
        server.0.wait().unwrap();
    }
}

#[test]
fn nvim_reuses_a_visible_buffer_from_another_tab_for_every_placement() {
    for placement in ["buffer", "split", "tab"] {
        let temporary = tempdir().unwrap();
        let existing = temporary.path().join("existing.txt");
        fs::write(&existing, "existing").unwrap();
        let existing = existing.to_string_lossy().into_owned();
        let socket = temporary.path().join("nvim.sock");
        let Some(mut server) = start_nvim(&socket) else {
            return;
        };
        let encoded_existing = serde_json::to_string(&existing)
            .unwrap()
            .replace('\'', "''");
        assert_eq!(
            remote_expression(
                &socket,
                &format!(
                    "luaeval('(function(path) vim.cmd.tabnew(); vim.api.nvim_set_current_buf(vim.fn.bufadd(path)); vim.cmd.tabprevious(); return 0; end)(_A)', json_decode('{encoded_existing}'))"
                )
            ),
            "0"
        );
        assert_eq!(remote_expression(&socket, "tabpagenr('$')"), "2");

        let target = serde_json::json!({
            "kind": "editor",
            "address": "",
            "editor": {"kind": "nvim", "server": socket}
        });
        let result = drop_target::drop_run(&DropRunOptions {
            action: "nvim-open".to_string(),
            placement: placement.to_string(),
            paths: vec![existing.clone()],
            target_json: target.to_string(),
            desktop_id: String::new(),
            dry_run: false,
        });
        assert_eq!(result["ok"], true, "{placement}: {result}");

        assert_eq!(remote_expression(&socket, "tabpagenr('$')"), "2");
        assert_eq!(
            remote_expression(&socket, "fnamemodify(bufname(), ':p')"),
            existing
        );
        let windows = query_strings(
            &socket,
            "json_encode(map(getwininfo(), {_, value -> empty(bufname(value.bufnr)) ? '' : fnamemodify(bufname(value.bufnr), ':p')}))",
        );
        assert_eq!(windows.len(), 2, "{placement}: {windows:?}");
        assert_eq!(
            windows.iter().filter(|path| !path.is_empty()).count(),
            1,
            "{placement}: {windows:?}"
        );
        assert!(windows.contains(&existing));
        server.0.kill().unwrap();
        server.0.wait().unwrap();
    }
}

#[test]
fn nvim_tab_opens_each_new_path_in_exactly_one_new_tab() {
    let temporary = tempdir().unwrap();
    let mut paths = Vec::new();
    for name in ["first.txt", "second.txt"] {
        let path = temporary.path().join(name);
        fs::write(&path, name).unwrap();
        paths.push(path.to_string_lossy().into_owned());
    }
    let socket = temporary.path().join("nvim.sock");
    let Some(mut server) = start_nvim(&socket) else {
        return;
    };

    let target = serde_json::json!({
        "kind": "editor",
        "address": "",
        "editor": {"kind": "nvim", "server": socket}
    });
    let result = drop_target::drop_run(&DropRunOptions {
        action: "nvim-open".to_string(),
        placement: "tab".to_string(),
        paths: paths.clone(),
        target_json: target.to_string(),
        desktop_id: String::new(),
        dry_run: false,
    });
    assert_eq!(result["ok"], true, "{result}");

    let mut expected = paths;
    expected.sort();
    assert_eq!(remote_expression(&socket, "tabpagenr('$')"), "3");
    let mut buffers = query_strings(
        &socket,
        "json_encode(map(filter(getbufinfo(), {_, value -> !empty(value.name)}), {_, value -> fnamemodify(value.name, ':p')}))",
    );
    buffers.sort();
    assert_eq!(buffers, expected);
    let windows = query_strings(
        &socket,
        "json_encode(map(getwininfo(), {_, value -> empty(bufname(value.bufnr)) ? '' : fnamemodify(bufname(value.bufnr), ':p')}))",
    );
    assert_eq!(windows.len(), 3);
    let mut named_windows: Vec<_> = windows
        .into_iter()
        .filter(|path| !path.is_empty())
        .collect();
    named_windows.sort();
    assert_eq!(named_windows, expected);
    server.0.kill().unwrap();
    server.0.wait().unwrap();
}

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_nvim(socket: &std::path::Path) -> Option<Server> {
    let strict = std::env::var("FILEBLADE_TEST_STRICT").is_ok_and(|value| value == "1");
    let directory = socket.parent()?;
    let spawned = Command::new("nvim")
        .args(["--headless", "--clean", "--listen"])
        .arg(socket)
        .env("NVIM_LOG_FILE", directory.join("nvim.log"))
        .env("XDG_STATE_HOME", directory.join("state"))
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let child = match spawned {
        Ok(child) => child,
        Err(error) => {
            assert!(!strict, "nvim is required for this test: {error}");
            return None;
        }
    };
    let mut server = Server(child);
    let started = Instant::now();
    while !socket.exists() && started.elapsed() < Duration::from_secs(10) {
        if let Some(status) = server.0.try_wait().unwrap() {
            let log = fs::read_to_string(directory.join("nvim.log")).unwrap_or_default();
            assert!(
                !strict,
                "Neovim exited before creating its socket ({status}): {log}"
            );
            return None;
        }
        thread::sleep(Duration::from_millis(10));
    }
    if socket.exists() {
        return Some(server);
    }
    let log = fs::read_to_string(directory.join("nvim.log")).unwrap_or_default();
    assert!(
        !strict,
        "Neovim did not create its socket within 10 seconds: {log}"
    );
    None
}

fn remote_expression(socket: &std::path::Path, expression: &str) -> String {
    let output = Command::new("nvim")
        .arg("--server")
        .arg(socket)
        .args(["--remote-expr", expression])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn query_strings(socket: &std::path::Path, expression: &str) -> Vec<String> {
    serde_json::from_str(&remote_expression(socket, expression)).unwrap()
}

fn strings(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item.as_str().unwrap())
        .collect()
}

#[test]
fn real_tmux_clients_keep_their_named_and_explicit_sockets() {
    let output = Command::new("python3")
        .arg("-c")
        .arg(include_str!("drop_target_tmux.py"))
        .arg(env!("CARGO_BIN_EXE_fileblade"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}
