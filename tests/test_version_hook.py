import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
HOOK = ROOT / "tools/hooks/pre-commit"
SOURCES = ["manifest.json", "Cargo.toml", "Cargo.lock", "CHANGELOG.md", "fileblade-bin"]


class VersionHookTests(unittest.TestCase):
    def setUp(self):
        (ROOT / "target").mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix="version-hook-test-", dir=ROOT / "target")
        self.addCleanup(self.temporary.cleanup)
        self.repo = Path(self.temporary.name)
        self.env = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
        self.env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull,
                        GIT_TERMINAL_PROMPT="0", PYTHONDONTWRITEBYTECODE="1")
        self.git("init", "-q", "-b", "0.1.3")
        self.git("config", "user.name", "Version hook fixture")
        self.git("config", "user.email", "fixture@example.invalid")
        self.git("config", "core.hooksPath", "tools/hooks")
        (self.repo / "tools/hooks").mkdir(parents=True)
        shutil.copy2(HOOK, self.repo / "tools/hooks/pre-commit")
        self.write_versions()
        self.git("add", "--", *SOURCES)

    def git(self, *args, check=True):
        result = subprocess.run(["git", *args], cwd=self.repo, env=self.env,
                                capture_output=True, text=True, timeout=15)
        if check:
            self.assertEqual(result.returncode, 0, result.stderr)
        return result

    def write_versions(self, value="0.1.3"):
        self.write("manifest.json", json.dumps({"version": value}))
        self.write("Cargo.toml", f'[workspace.package]\nversion = "9.9.9"\n[package]\nname = "fileblade"\nversion = "{value}"\n[dependencies.other]\nversion = "8.8.8"\n')
        self.write("Cargo.lock", f'version = 4\n[[package]]\nname = "before"\nversion = "9.9.9"\n[[package]]\nname = "fileblade"\nversion = "{value}"\n[[package]]\nname = "after"\nversion = "8.8.8"\n')
        self.write("CHANGELOG.md", f'# Changelog\n\n## {value} (unreleased)\n\n- UI changes\n  - Example.\n')
        self.write_binary(f'printf "fileblade {value}\\n"')

    def write(self, name, content):
        (self.repo / name).write_text(content)

    def write_binary(self, body):
        self.write("fileblade-bin", f"#!/usr/bin/env bash\n{body}\n")
        (self.repo / "fileblade-bin").chmod(0o755)

    def hook(self, success=True, contains=""):
        result = subprocess.run([sys.executable, str(HOOK)], cwd=self.repo, env=self.env,
                                capture_output=True, text=True, timeout=12)
        self.assertEqual(result.returncode == 0, success, result.stderr)
        self.assertIn(contains, result.stderr)
        self.assertNotIn("Traceback", result.stderr)
        self.assertEqual(list((self.repo / ".git").glob("fileblade-version-*")), [])
        return result

    def test_valid_first_commit_and_unstaged_changes_are_preserved(self):
        self.write_versions("9.9.9")
        self.write("unrelated.txt", "in-flight work")
        before = {path: (self.repo / path).read_bytes() for path in SOURCES}
        self.git("commit", "-qm", "Valid staged versions")
        self.assertEqual(json.loads(self.git("show", "HEAD:manifest.json").stdout)["version"], "0.1.3")
        self.assertEqual({path: (self.repo / path).read_bytes() for path in SOURCES}, before)
        self.assertEqual((self.repo / "unrelated.txt").read_text(), "in-flight work")

    def test_wrong_staged_manifest_is_rejected_by_git_commit(self):
        self.write("manifest.json", '{"version":"0.9.9"}')
        self.git("add", "manifest.json")
        self.write("manifest.json", '{"version":"0.1.3"}')
        result = self.git("commit", "-qm", "Reject mismatched index", check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("staged Cargo.toml", result.stderr)
        self.assertEqual(json.loads(self.git("show", ":manifest.json").stdout)["version"], "0.9.9")
        self.assertEqual(json.loads((self.repo / "manifest.json").read_text())["version"], "0.1.3")

    def test_alternate_index_is_authoritative(self):
        alternate = self.repo / ".git/alternate-index"
        shutil.copy2(self.repo / ".git/index", alternate)
        self.env["GIT_INDEX_FILE"] = str(alternate)
        self.write("manifest.json", '{"version":"0.9.9"}')
        self.git("add", "manifest.json")
        self.write("manifest.json", '{"version":"0.1.3"}')
        self.hook(False, "staged Cargo.toml")
        del self.env["GIT_INDEX_FILE"]
        self.hook()

    def test_commit_only_checks_the_temporary_commit_index(self):
        self.git("commit", "-qm", "Baseline")
        self.write_versions("0.1.4")
        self.git("add", "--", *SOURCES)
        self.write("unrelated.txt", "partial commit")
        self.git("add", "unrelated.txt")
        self.git("commit", "--only", "-qm", "Only unrelated file", "--", "unrelated.txt")
        self.assertEqual(json.loads(self.git("show", "HEAD:manifest.json").stdout)["version"], "0.1.3")
        self.assertEqual(json.loads(self.git("show", ":manifest.json").stdout)["version"], "0.1.4")

    def test_shared_hook_works_inside_a_linked_worktree(self):
        self.git("add", "tools/hooks/pre-commit")
        self.git("commit", "-qm", "Baseline with hook")
        primary = self.repo
        linked = primary / "linked"
        self.git("worktree", "add", "--quiet", "--detach", str(linked), "HEAD")
        self.repo = linked
        self.assertEqual(self.git("config", "--get", "core.hooksPath").stdout.strip(), "tools/hooks")
        self.git("commit", "--allow-empty", "-qm", "Linked worktree commit")
        self.write("manifest.json", '{"version":"0.9.9"}')
        self.git("add", "manifest.json")
        self.write("manifest.json", '{"version":"0.1.3"}')
        result = self.git("commit", "-qm", "Reject linked mismatch", check=False)
        self.assertNotEqual(result.returncode, 0)
        git_directory = Path(self.git("rev-parse", "--absolute-git-dir").stdout.strip())
        self.assertEqual(list(git_directory.glob("fileblade-version-*")), [])
        self.repo = primary
        self.assertEqual(json.loads(self.git("show", ":manifest.json").stdout)["version"], "0.1.3")

    def test_toml_formatting_and_package_selection(self):
        cases = [
            ('[package]\nname="fileblade"\n version="0.9.9"\n [dependencies.other]\nversion = "0.1.3"\n', False, "staged Cargo.toml"),
            ("[package]\nname='fileblade'\nversion='0.1.3'\n", True, ""),
            ('[package]\nname="fileblade"\nversion.workspace=true\n[workspace.package]\nversion="0.1.3"\n', False, "workspace inheritance"),
            ('[package]\nname="other"\nversion="0.1.3"\n', False, "named fileblade"),
            ('[package]\nname="fileblade"\n', False, "valid version"),
            ('[package]\nname="fileblade"\nversion="0.1.3"\nversion="0.9.9"\n', False, "valid TOML"),
        ]
        for content, success, message in cases:
            with self.subTest(content=content):
                self.write("Cargo.toml", content)
                self.git("add", "Cargo.toml")
                self.hook(success, message)

    def test_lockfile_record_boundaries_and_identity(self):
        cases = [
            ('[[package]]\nversion="0.9.9"\nname="fileblade"\n[[package]]\nname="after"\nversion="0.1.3"\n', False, "staged Cargo.lock"),
            ('[[package]]\nversion="0.1.3"\nname="fileblade"\n', True, ""),
            ('[[package]]\nname="fileblade"\n[[package]]\nname="after"\nversion="0.1.3"\n', False, "valid version"),
            ('[[package]]\nname="other"\nversion="0.1.3"\n', False, "exactly one local"),
            ('[[package]]\nname="fileblade"\nversion="0.1.3"\n[[package]]\nname="fileblade"\nversion="0.1.3"\n', False, "exactly one local"),
            ('[[package]]\nname="fileblade"\nversion="0.1.3"\nsource="registry+https://example.invalid"\n', False, "exactly one local"),
        ]
        for content, success, message in cases:
            with self.subTest(content=content):
                self.write("Cargo.lock", content)
                self.git("add", "Cargo.lock")
                self.hook(success, message)

    def test_changelog_requires_a_versioned_release(self):
        cases = [
            ("# Changelog\nNo release heading.\n", False),
            ("## \n## 0.9.9\n", False),
            ("## Unreleased\n## 0.1.3\n", False),
            ("## 0.1.3.4\n## 0.1.3\n", False),
            ("## 0.9.9\n## 0.1.3\n", False),
            ("## Overview\n## 0.1.3 (unreleased)\n", True),
            ("## 0.1.3\n\n### UI changes\n", True),
        ]
        for content, success in cases:
            with self.subTest(content=content):
                self.write("CHANGELOG.md", content)
                self.git("add", "CHANGELOG.md")
                self.hook(success)

    def test_missing_or_symlinked_sources_are_rejected(self):
        for name in SOURCES:
            with self.subTest(name=name):
                original = (self.repo / name).read_bytes()
                self.git("rm", "--cached", "--", name)
                self.hook(False, "resolved entry")
                (self.repo / name).unlink()
                (self.repo / name).symlink_to("nonexistent")
                self.git("add", "--", name)
                self.hook(False, "regular file")
                (self.repo / name).unlink()
                (self.repo / name).write_bytes(original)
                if name == "fileblade-bin":
                    (self.repo / name).chmod(0o755)
                self.git("add", "--", name)

    def test_nonexecutable_staged_binary_is_rejected(self):
        self.git("update-index", "--chmod=-x", "fileblade-bin")
        self.hook(False, "executable mode 100755")
        self.assertTrue(os.access(self.repo / "fileblade-bin", os.X_OK))

    def test_binary_failures_and_versions(self):
        for body in ["exit 1", "exit 0", "echo broken", "echo fileblade 0.9.9", "echo other 0.1.3", "echo fileblade 0.1.3; echo extra"]:
            with self.subTest(body=body):
                self.write_binary(body)
                self.git("add", "fileblade-bin")
                self.write_binary('echo fileblade 0.1.3')
                self.hook(False)

    def test_binary_output_is_bounded(self):
        self.write_binary('printf "%5000s" x')
        self.git("add", "fileblade-bin")
        self.hook(False, "exceeds 4096 bytes")

    def test_binary_deadline_stops_the_process_group(self):
        pid_path = self.repo / "sleep.pid"
        self.write_binary('sleep 30 &\necho $! > sleep.pid\nwait')
        self.git("add", "fileblade-bin")
        self.hook(False, "5-second version deadline")
        pid = int(pid_path.read_text())
        status = Path(f"/proc/{pid}/stat")
        if status.exists():
            self.assertEqual(status.read_text().split()[2], "Z", "version helper child is still running")

    def test_versions_and_release_branches(self):
        self.git("symbolic-ref", "HEAD", "refs/heads/feature")
        for value, success in [("0.1.3-rc.1+build.7", True), ("01.1.3", False), ("0.1.3-01", False), ("0.1.3+", False)]:
            with self.subTest(value=value):
                self.write_versions(value)
                self.git("add", "--", *SOURCES)
                self.hook(success)
        self.write_versions()
        self.git("add", "--", *SOURCES)
        self.git("symbolic-ref", "HEAD", "refs/heads/0.1.4")
        self.hook(False, "branch 0.1.4")

    def test_oversized_staged_metadata_is_rejected(self):
        self.write("manifest.json", '{"version":"0.1.3","padding":"' + "x" * 65536 + '"}')
        self.git("add", "manifest.json")
        self.hook(False, "exceeds 65536 bytes")


if __name__ == "__main__":
    unittest.main()
