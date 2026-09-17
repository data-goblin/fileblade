import os
from pathlib import Path
import subprocess
import sys
import tempfile


root = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="fileblade-core-contracts-") as temporary:
    environment = dict(os.environ, PYTHONDONTWRITEBYTECODE="1", PYTHONPATH=str(root / "python"),
                       FILEBLADE_BINARY=os.environ.get("FILEBLADE_BINARY", str(root / "fileblade-bin")),
                       XDG_STATE_HOME=temporary)
    cases = {
        "skills": ["unit.py"],
        "hooks": ["test_discovery.py", "test_apply.py", "test_undo.py"],
        "mcp": ["test_inventory.py", "test_apply.py", "test_undo.py"],
    }
    for module, files in cases.items():
        for filename in files + ["test_path_identity.py", "test_watch.py"] + (["../test_core_bin.py"] if module in ("hooks", "mcp") else []):
            path = Path(__file__).parent / module / filename
            print(f"{module}/{filename}", flush=True)
            subprocess.run([sys.executable, "-B", str(path)], cwd=root, env=dict(environment, FILEBLADE_TEST_MODULE=module),
                           check=True, timeout=120)
    subprocess.run([sys.executable, "-B", str(Path(__file__).parent / "test_recovery_store.py")],
                   cwd=root, env=environment, check=True, timeout=120)
