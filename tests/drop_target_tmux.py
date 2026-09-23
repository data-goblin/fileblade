import json
import os
from pathlib import Path
import pty
import socket
import subprocess
import sys
import tempfile
import threading
import time

binary = str(Path(sys.argv[1]).resolve())
with tempfile.TemporaryDirectory(prefix="fileblade-tmux-") as temporary:
    root = Path(temporary)
    path = root / "a quoted ' file.txt"
    path.write_text("A real multiplexer regression\n")
    env = dict(os.environ, HOME=temporary, XDG_CONFIG_HOME=temporary,
               XDG_STATE_HOME=temporary, TMUX_TMPDIR=temporary, TERM="xterm-256color")
    env.pop("TMUX", None)
    env.pop("FILEBLADE_NATIVE_ROOT", None)
    listener = socket.socket(socket.AF_UNIX)
    listener.bind(str(root / "hypr.sock"))
    listener.listen()
    env["FILEBLADE_HYPR_SOCKET"] = str(root / "hypr.sock")
    client = {}

    def compositor():
        while True:
            stream, _ = listener.accept()
            with stream:
                request = stream.recv(4096).decode()
                if request == "j/clients":
                    result = [client]
                elif request == "j/monitors":
                    result = [{"id": 0, "x": 0, "y": 0, "width": 1000, "height": 800,
                               "scale": 1, "activeWorkspace": {"id": 1}}]
                else:
                    stream.sendall(b"ok")
                    continue
                stream.sendall(json.dumps(result).encode())

    threading.Thread(target=compositor, daemon=True).start()
    for selector in [["-L", "named"], ["-Ljoined"], ["-S", str(root / "space socket")], ["-Srelative.sock"]]:
        prefix = ["tmux", *selector]
        subprocess.run([*prefix, "-f", "/dev/null", "new-session", "-d", "-s", "proof", "sleep 30"],
                       cwd=root, env=env, check=True)
        master, slave = pty.openpty()
        process = subprocess.Popen([*prefix, "attach", "-t", "proof"], cwd=root, env=env,
                                   stdin=slave, stdout=slave, stderr=slave)
        os.close(slave)
        try:
            deadline = time.monotonic() + 5
            while True:
                rows = subprocess.check_output([*prefix, "list-clients", "-F", "#{client_pid}"], cwd=root, env=env, text=True)
                if str(process.pid) in rows.splitlines():
                    break
                assert process.poll() is None and time.monotonic() < deadline
                time.sleep(.02)
            client = {"address": "0xabc", "pid": process.pid, "class": "foot", "mapped": True,
                      "hidden": False, "at": [100, 100], "size": [500, 400], "workspace": {"id": 1}}
            context = json.loads(subprocess.check_output([binary, "_backend", "drop-context", "--x", "200",
                                                        "--y", "200", "--path", str(path)], env=env))
            target = context["target"]
            expected = subprocess.check_output([*prefix, "display-message", "-p", "#{socket_path}"], cwd=root, env=env, text=True).strip()
            assert target["terminal"].get("socket") == str((root / expected).resolve()), context
            result = json.loads(subprocess.check_output([binary, "_backend", "drop-run", "--action", "mux-open",
                                                        "--placement", "right", "--path", str(path),
                                                        "--target", json.dumps(target)], env=env))
            assert result["ok"], result
            rows = subprocess.check_output([*prefix, "list-panes", "-F", "#{pane_pid}"], cwd=root, env=env, text=True).splitlines()
            assert len(rows) == 2, rows
            deadline = time.monotonic() + 5
            while not any(str(path).encode() in Path(f"/proc/{pid}/cmdline").read_bytes() for pid in rows):
                assert time.monotonic() < deadline, rows
                time.sleep(.02)
        finally:
            subprocess.run([*prefix, "kill-server"], cwd=root, env=env, check=False, stderr=subprocess.DEVNULL)
            process.wait(timeout=5)
            os.close(master)
print("Real tmux named, joined and explicit sockets: passed")
