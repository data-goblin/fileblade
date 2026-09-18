import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

root = Path(__file__).resolve().parents[2]
if root != Path.home() / '.config/omarchy/plugins/data-goblin.fileblade':
    raise SystemExit('run only in the staged plugin inside the allocated harness')
fixture = Path('/tmp/rivet-settings-version')
config = Path.home() / '.config/omarchy/fileblade'
state = Path(os.environ.get('XDG_STATE_HOME', str(Path.home() / '.local/state'))) / 'omarchy/fileblade/state.json'
service = root / 'Service.qml'
controller = root / 'controllers/StateController.qml'


def ipc(method):
    result = subprocess.run(['omarchy-shell', 'fileblade.settings-version', method],
                            check=True, capture_output=True, text=True, timeout=10)
    return json.loads(result.stdout)


def settled():
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        result = ipc('snapshot')
        if result['ready'] and not result['busy'] and state.exists():
            saved = json.loads(state.read_text())
            if saved == result['document']:
                return result
        time.sleep(.1)
    raise AssertionError('state did not settle through the real backend')


phase = sys.argv[1]
if phase == 'prepare':
    fixture.mkdir(mode=0o700)
    for source, name in [(service, 'Service.qml'), (controller, 'StateController.qml'),
                         (state, 'state.json'), (config / 'blades.json', 'blades.json')]:
        if source.exists():
            shutil.copy2(source, fixture / name)
        else:
            (fixture / (name + '.absent')).touch()
    (fixture / 'Probe.qml').write_text('''import QtQuick
import Quickshell.Io
Item {
  id: probe
  property var service
  property var state
  function snapshot() {
    return { ready: state.ready, busy: !!state.stateWriteRequestId || !!state.queuedStateDocument,
      document: state.document(), git: service.gitEnabled, icons: service.propertyIcons }
  }
  IpcHandler {
    target: "fileblade.settings-version"
    function snapshot(): string { return JSON.stringify(probe.snapshot()) }
    function seed(): string {
      probe.state.stateRereadPending = true
      probe.state.receiveStateRead({ ok: true, text: '{"version":12,"future":{"keep":[null,false,42]}}' })
      probe.service.setGitEnabled(probe.service.gitEnabled)
      probe.state.save()
      return JSON.stringify(probe.snapshot())
    }
    function choices(): string {
      var s = probe.service
      s.setShowHidden(s.showHidden)
      s.setPriorityProperty(s.priorityProperty)
      s.setSearchOptions(s.searchCaseSensitive, s.searchRegex)
      s.setTreeOrder(s.treeSort, s.treeFilter)
      probe.state.save()
      return JSON.stringify(probe.snapshot())
    }
    function reset(): string {
      probe.service.resetSettings()
      probe.state.save()
      return JSON.stringify(probe.snapshot())
    }
  }
}
''')
    source = service.read_text().replace('cliPath: service.cliPath', 'cliPath: service.pluginDir + "/target/release/fileblade"', 1)
    source = source.replace('id: service', 'id: service\n  Loader { source: "file:///tmp/rivet-settings-version/Probe.qml"; onLoaded: { item.service = service; item.state = stateController } }', 1)
    service.write_text(source)
elif phase == 'seed':
    ipc('seed')
    result = settled()
    assert result['git'] is True and result['icons'] is True, result
    assert result['document']['gitEnabled'] is True, result
    assert 'propertyIcons' not in result['document'], result
    print(json.dumps(result))
elif phase == 'revise':
    source = controller.read_text()
    for key in ['gitEnabled', 'propertyIcons']:
        before = f'{key}: service.boolValue(config.{key}, true),'
        assert source.count(before) == 1
        source = source.replace(before, f'{key}: false,')
    controller.write_text(source)
elif phase == 'verify':
    result = ipc('snapshot')
    assert result['git'] is True and result['icons'] is False, result
    assert result['document']['gitEnabled'] is True, result
    assert 'propertyIcons' not in result['document'], result
    assert result['document']['future'] == {'keep': [None, False, 42]}, result
    print('after revised-default restart:', json.dumps(result))
    ipc('choices')
    result = settled()
    for key in ['showHidden', 'priorityProperty', 'priorityColumns', 'searchCaseSensitive',
                'searchRegex', 'treeSort', 'treeFilter']:
        assert key in result['document'], (key, result)
    ipc('reset')
    result = settled()
    assert result['git'] is False and result['icons'] is False, result
    assert result['document']['future'] == {'keep': [None, False, 42]}, result
    for key in ['gitEnabled', 'propertyIcons', 'showHidden', 'priorityProperty', 'priorityColumns',
                'searchCaseSensitive', 'searchRegex', 'treeSort', 'treeFilter']:
        assert key not in result['document'], (key, result)
    print('after explicit choices and reset:', json.dumps(result))
elif phase == 'restore':
    for target, name in [(service, 'Service.qml'), (controller, 'StateController.qml'),
                         (state, 'state.json'), (config / 'blades.json', 'blades.json')]:
        if (fixture / (name + '.absent')).exists():
            target.unlink(missing_ok=True)
        else:
            shutil.copy2(fixture / name, target)
else:
    raise SystemExit('expected prepare, seed, revise, verify or restore')
