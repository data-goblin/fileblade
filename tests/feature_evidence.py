#!/usr/bin/env python3
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys

root = Path(__file__).resolve().parents[1] / "features"
catalog = json.loads((root / "catalog.json").read_text())
errors = []
seen = set()
for feature in catalog["features"]:
    name = feature["id"]
    if name in seen or ".." in Path(name).parts or Path(name).is_absolute():
        errors.append(f"{name}: invalid or duplicate identifier")
        continue
    seen.add(name)
    try:
        page = (root / (name + ".md")).read_text()
        assert page.startswith("This file was written by an agent."), "missing authorship"
        assert feature["status"] == "verified", "evidence is not verified"
        evidence = feature["evidence"]
        for suffix, field in [(".png", "screenshot_sha256"), (".mp4", "video_sha256")]:
            artifact = root / (name + suffix)
            assert artifact.name in page, f"missing {suffix} link"
            assert hashlib.sha256(artifact.read_bytes()).hexdigest() == evidence[field], f"changed {suffix}"
        png = (root / (name + ".png")).read_bytes()
        assert png[:8] == b"\x89PNG\r\n\x1a\n", "invalid PNG"
        assert struct.unpack(">II", png[16:24]) == (2560, 1440), "incorrect screenshot dimensions"
        media = json.loads(subprocess.check_output([
            "ffprobe", "-v", "error", "-show_streams", "-show_format", "-of", "json",
            str(root / (name + ".mp4")),
        ], text=True))
        video = [s for s in media["streams"] if s["codec_type"] == "video"]
        assert len(video) == 1, "expected one video stream"
        assert (video[0]["width"], video[0]["height"]) == (2560, 1440), "incorrect video dimensions"
        assert video[0]["codec_name"] == "h264", "expected H.264"
        duration = float(media["format"]["duration"])
        assert 4 <= duration <= 8, f"duration {duration} is outside 4–8 seconds"
        assert abs(duration - evidence["duration_seconds"]) < 0.05, "duration receipt mismatch"
        assert evidence["full_decode"] is True, "full decode not recorded"
        assert evidence.get("review"), "visual review missing"
    except (AssertionError, KeyError, OSError, ValueError, subprocess.CalledProcessError) as error:
        errors.append(f"{name}: {error}")
for error in errors:
    print(error)
print(f"Feature evidence: {len(seen) - len(errors)}/{len(seen)} passed")
sys.exit(bool(errors))
