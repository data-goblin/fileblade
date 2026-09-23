import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time


repo = Path(__file__).resolve().parent.parent
with tempfile.TemporaryDirectory(prefix="fileblade-theme-") as temporary:
    root = Path(temporary)
    current = root / ".local/state/omarchy/current"
    current.mkdir(parents=True)
    runtime = root / "runtime"
    runtime.mkdir(mode=0o700)
    config = root / "probe"
    config.mkdir()
    (config / "shell.qml").write_text(
        'import QtQuick\nimport Quickshell\n'
        + 'import "' + (repo / "app/Commons").as_uri() + '" as Native\n'
        + 'ShellRoot {\n'
        + '  property string snapshot: JSON.stringify([String(Native.Color.background), String(Native.Color.bar.background), String(Native.Color.accent)])\n'
        + '  onSnapshotChanged: console.log("theme-state:" + snapshot)\n'
        + '}\n'
    )

    def apply_theme(background, surface, accent):
        stage = current / "next-theme"
        stage.mkdir()
        (stage / "colors.toml").write_text(
            f'background = "{background}"\nforeground = "#eeeeee"\naccent = "{accent}"\n'
        )
        (stage / "shell.toml").write_text(f'[bar]\nbackground = "{surface}"\n')
        theme = current / "theme"
        if theme.exists():
            shutil.rmtree(theme)
            time.sleep(0.1)
        stage.rename(theme)
        (current / "theme.name").write_text(background + "\n")

    palettes = [
        ("#102030", "#112233", "#aabbcc"),
        ("#302010", "#332211", "#ccbbaa"),
        ("#102030", "#112233", "#aabbcc"),
    ]
    apply_theme(*palettes[0])
    environment = dict(os.environ, HOME=str(root), XDG_STATE_HOME=str(root / ".local/state"),
                       XDG_CONFIG_HOME=str(root / ".config"), XDG_CACHE_HOME=str(root / ".cache"),
                       XDG_RUNTIME_DIR=str(runtime), QT_QPA_PLATFORM="offscreen", QT_QUICK_BACKEND="software")
    environment.pop("QT_QPA_PLATFORMTHEME", None)
    environment.pop("WAYLAND_DISPLAY", None)
    log_path = root / "probe.log"
    with log_path.open("w") as log:
        process = subprocess.Popen(["qs", "-n", "-p", str(config)], env=environment,
                                   stdout=log, stderr=subprocess.STDOUT)
        try:
            offset = 0
            for index, palette in enumerate(palettes):
                if index:
                    offset = log_path.stat().st_size
                    apply_theme(*palette)
                expected = "theme-state:" + json.dumps(palette, separators=(",", ":"))
                deadline = time.monotonic() + 10
                while expected not in log_path.read_text()[offset:]:
                    if process.poll() is not None or time.monotonic() >= deadline:
                        raise AssertionError(f"Theme {index} did not load: {log_path.read_text()}")
                    time.sleep(0.02)
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
print("Native theme replacement and return: passed")
