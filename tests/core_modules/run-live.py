import json
from pathlib import Path
import subprocess
import sys
import time

if Path.home() != Path('/home/omarchy'):
    raise SystemExit('Run only in the assigned Omarchy test guest')

module = sys.argv[1]
if module not in ('skills', 'memory', 'hooks', 'mcp'):
    raise SystemExit('Expected a core module name')
state = Path('/tmp/rivet-core-live')
project = state / 'project'
target = 'fileblade.core-live.' + module


def ipc(target, method, *arguments):
    result = subprocess.run(['omarchy-shell', target, method, *map(str, arguments)],
                            capture_output=True, text=True, timeout=8, check=True)
    return result.stdout.strip()


def status():
    return json.loads(ipc(target, 'status'))


def wait(predicate):
    deadline = time.monotonic() + 25
    last = None
    while time.monotonic() < deadline:
        try:
            last = status()
            if predicate(last):
                return last
        except (subprocess.SubprocessError, ValueError):
            pass
        time.sleep(0.15)
    raise AssertionError(f'{module} timeout: {last}')


def ready(value):
    return value['ready'] and not value['busy'] and not value['applying'] and not value['loadError']


def record(label, value):
    (state / (module + '-' + label + '.json')).write_text(json.dumps(value, indent=2))
    print(module + ': ' + label + ' PASS', flush=True)


ipc('data-goblin.fileblade.control', 'setSlotModule', 'right', '0', module)
ipc('data-goblin.fileblade.control', 'focusBlade', 'right')
loaded = wait(lambda value: ready(value) and bool(value['rows']))
assert loaded['provider'] == 'fileblade.core.' + module, loaded
assert loaded['observers'] == 1 and not loaded['watchError'], loaded
assert all(lane['watch'] for lane in loaded['lanes']), loaded
row = next(row for row in loaded['rows'] if
           (module == 'skills' and row['name'] == 'rivet-fixture') or
           (module == 'memory' and row['path'] == str(project / 'AGENTS.md')) or
           (module == 'hooks' and row['agent'] == 'claude-code') or
           (module == 'mcp' and row['name'] == 'rivet-fixture' and row['agentId'] == 'claude-code'))
record('loaded', loaded)
assert loaded['bin'] and not loaded['bin']['rows'], loaded
agent = 'claude-code' if module == 'memory' else 'codex'
applied_path = Path.home() / '.codex' / ('hooks.json' if module == 'hooks' else 'config.toml')
applied_original = applied_path.read_bytes() if module in ('hooks', 'mcp') and applied_path.exists() else None
if module in ('hooks', 'mcp'):
    (state / (module + '-apply-target.json')).write_text(json.dumps({'path': str(applied_path), 'existed': applied_original is not None}))
    if applied_original is not None:
        (state / (module + '-apply-target.original')).write_bytes(applied_original)
if module in ('skills', 'memory'):
    ipc(target, 'consent', 'false')
    denied = wait(lambda value: not value['consent'])
    ipc(target, 'apply', row['id'], agent, 'true')
    denied = wait(lambda value: 'Enable Manage agent files' in value['applyError'])
    assert denied['completions'] == loaded['completions'] and not denied['applying'], denied
    record('consent-refused', denied)
    ipc(target, 'consent', 'true')
    wait(lambda value: value['consent'])

for enabled in ('true', 'false'):
    before = status()['completions']
    ipc(target, 'apply', row['id'], agent, enabled)
    changed = wait(lambda value: ready(value) and value['completions'] > before)
    assert changed['response']['ok'] and not changed['applyError'], changed
    assert any(result.get('changed') for result in changed['response']['results']), changed
    if module in ('skills', 'memory'):
        link = project / ('.agents/skills/rivet-fixture' if module == 'skills' else 'CLAUDE.md')
        assert link.is_symlink() if enabled == 'true' else not link.exists()
    record('apply' if enabled == 'true' else 'unapply', changed)

if module in ('hooks', 'mcp'):
    if applied_original is None:
        applied_path.unlink(missing_ok=True)
    else:
        applied_path.write_bytes(applied_original)

source = project / {'skills': '.claude/skills/rivet-fixture/SKILL.md', 'memory': 'AGENTS.md',
                    'hooks': '.claude/settings.json', 'mcp': '.mcp.json'}[module]
original = source.read_bytes()
ipc(target, 'bin', row['id'], 'bin')
removed = wait(lambda value: ready(value) and value['bin'] and not value['bin']['busy'] and bool(value['bin']['rows']))
assert not removed['bin']['error'], removed
if module in ('skills', 'memory'):
    assert not source.exists()
else:
    assert source.read_bytes() != original
    assert removed['bin']['route']['provider'] == 'fileblade.core.' + module
    assert removed['bin']['route']['directory'] == ''
record('removed', removed)
entry = next(entry for entry in removed['bin']['rows'] if entry.get('sourceId') == row['id'] or
             (entry.get('source') or {}).get('id') == row['id'])
recovery_path = None
if module in ('hooks', 'mcp'):
    manifest = json.loads((Path(entry['realpath']) / 'manifest.json').read_text())
    assert manifest['restoreHelper'] == removed['bin']['route'], manifest
    identifier = manifest['helperRecordId']
    assert len(identifier) == 32 and all(character in '0123456789abcdef' for character in identifier)
    recovery_path = Path.home() / '.local/state/fileblade' / (module + '-recovery') / (identifier + '.json')
    recovery = json.loads(recovery_path.read_text())
    assert recovery['formatVersion'] == 1 and recovery['payload'] == manifest['payload'], recovery
    record('paired-recovery', {'manifest': manifest, 'record': recovery})
ipc(target, 'bin', entry['id'], 'restore')
restored = wait(lambda value: ready(value) and value['bin'] and not value['bin']['busy'] and not value['bin']['rows'])
assert not restored['bin']['error'], restored
if recovery_path is not None:
    assert not recovery_path.exists(), recovery_path
if module in ('hooks', 'mcp'):
    assert json.loads(source.read_bytes()) == json.loads(original)
else:
    assert source.read_bytes() == original
record('restored', restored)
ipc('data-goblin.fileblade.control', 'closeBlade', 'right')
closed = json.loads(ipc('data-goblin.fileblade', 'blades'))
assert closed['blades']['right']['open'] is False, closed
try:
    inactive = status()
except subprocess.CalledProcessError:
    inactive = None
if inactive is not None:
    assert not inactive['ready'] and inactive['observers'] == 0, inactive
record('closed', closed)
ipc('data-goblin.fileblade.control', 'focusBlade', 'right')
reopened = wait(lambda value: ready(value) and value['observers'] == 1 and bool(value['rows']))
assert not reopened['watchError'], reopened
record('reopened', reopened)
print('CORE_LIVE_PASS ' + module, flush=True)
