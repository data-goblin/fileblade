import json
import os
from pathlib import Path
import subprocess
import sys
import time


if Path.home() != Path('/home/omarchy'):
    raise SystemExit('Run only in harness C')
root = Path(__file__).resolve().parents[2]
state = Path('/tmp/rivet-core-lifecycle')
source = root / 'Service.qml'
mode = sys.argv[1]
if mode == 'prepare':
    state.mkdir(mode=0o700)
    (state / 'Service.qml').write_bytes(source.read_bytes())
    project = state / 'project'
    (project / '.git').mkdir(parents=True)
    (project / '.claude/skills/lifecycle').mkdir(parents=True)
    (project / '.claude/skills/lifecycle/SKILL.md').write_text('---\nname: lifecycle\ndescription: Lifecycle fixture\n---\nLocal text\n')
    (project / 'AGENTS.md').write_text('Local lifecycle fixture\n')
    sentinel = str(state / 'declaration-executed')
    (project / '.claude/settings.json').write_text(json.dumps({'hooks': {'PreToolUse': [{'hooks': [{'type': 'command', 'command': 'touch ' + sentinel}]}]}}))
    (project / '.mcp.json').write_text(json.dumps({'mcpServers': {'lifecycle': {'command': 'touch', 'args': [sentinel], 'env': {'API_TOKEN': 'LIFECYCLE_PRIVATE_SENTINEL'}}}}))
    text = source.read_text().replace('cliPath: service.cliPath', 'cliPath: service.pluginDir + "/target/release/fileblade"', 1)
    fragment = '\n  Loader { source: Qt.resolvedUrl("tests/core_modules/LifecycleProbe.qml"); onLoaded: { item.service = service; item.backend = backendClient } }\n'
    source.write_text(text[:text.rfind('}')] + fragment + '}\n')
    print('Prepared; restart C and verify the catalog/open blade before check')
    raise SystemExit(0)
if mode == 'restore':
    source.write_bytes((state / 'Service.qml').read_bytes())
    print('Restored Service source; restart C and verify the catalog/open blade')
    raise SystemExit(0)
if mode != 'check':
    raise SystemExit('Expected prepare, check or restore')


def ipc(method, *arguments):
    return subprocess.run(['omarchy-shell', 'fileblade.core-lifecycle', method, *arguments],
                          capture_output=True, text=True, timeout=8, check=True).stdout.strip()


def wait(predicate):
    deadline = time.monotonic() + 20
    value = None
    while time.monotonic() < deadline:
        value = json.loads(ipc('status'))
        if predicate(value):
            return value
        time.sleep(0.1)
    raise AssertionError(value)


def idle(value):
    return all(module['views'] == 0 and not module['ready'] and not module['busy']
               and all(not lane['scan'] and not lane['watch'] for lane in module['lanes'])
               for module in value['modules'].values())


def resources(pid):
    process = Path('/proc') / str(pid)
    children = set()
    for task in (process / 'task').iterdir():
        try:
            children.update((task / 'children').read_text().split())
        except FileNotFoundError:
            pass
    return {'threads': len(list((process / 'task').iterdir())), 'children': sorted(children),
            'executable': os.readlink(process / 'exe')}


servers = []
for process in Path('/proc').iterdir():
    if process.name.isdigit():
        try:
            if os.readlink(process / 'exe') == str(root / 'target/release/fileblade') and b'serve' in (process / 'cmdline').read_bytes().split(b'\0'):
                servers.append(int(process.name))
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            pass
assert len(servers) == 1, servers
pid = servers[0]
results = []
ipc('begin', str(state / 'project'))
try:
    initial = wait(idle)
    time.sleep(1)
    baseline = resources(pid)
    assert baseline['executable'] == str(root / 'target/release/fileblade'), baseline
    for name in ('skills', 'memory', 'hooks', 'mcp'):
        for cycle in range(2):
            ipc('show', name)
            opened = wait(lambda value: value['modules'][name]['views'] == 1
                          and value['modules'][name]['ready'] and not value['modules'][name]['busy']
                          and not value['modules'][name]['error']
                          and bool(value['modules'][name]['rows'])
                          and all(lane['watch'] and not lane['scan'] for lane in value['modules'][name]['lanes']))
            assert not (state / 'declaration-executed').exists()
            assert 'LIFECYCLE_PRIVATE_SENTINEL' not in json.dumps(opened)
            subscriptions = [lane['watch'] for lane in opened['modules'][name]['lanes']]
            ipc('close')
            closed = wait(lambda value: idle(value) and not any(
                key.split('\u0000')[0] in subscriptions for key in value['pending']))
            deadline = time.monotonic() + 5
            after = resources(pid)
            while (after['threads'] > baseline['threads'] or after['children']) and time.monotonic() < deadline:
                time.sleep(0.1)
                after = resources(pid)
            assert after['threads'] <= baseline['threads'] and not after['children'], (baseline, after)
            results.append({'module': name, 'cycle': cycle, 'opened': opened, 'closed': closed,
                            'baseline': baseline, 'after': after})
            print(f'{name} cycle {cycle + 1}: views, watches, pending subscriptions and workers PASS', flush=True)
    assert not (state / 'declaration-executed').exists()
    (state / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
    print('CORE_LIFECYCLE_PASS: 8 real-Service open/close cycles; no declaration execution; MCP secrets redacted')
finally:
    ipc('restore')
