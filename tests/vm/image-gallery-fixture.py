#!/usr/bin/env python3
import json
from pathlib import Path
import shutil
import sys

consumer = Path.home() / '.config/omarchy/plugins/kurt.goblin-images'
state = Path(sys.argv[2])
module = consumer / 'blades/Module.qml'
config = Path.home() / '.config/omarchy/shell.json'
layout = Path.home() / '.config/omarchy/fileblade/blades.json'

if sys.argv[1] == 'prepare':
    state.mkdir(mode=0o700, parents=True, exist_ok=True)
    for name, path in [('Module.qml', module), ('shell.json', config), ('blades.json', layout), ('GoblinBarWidget.qml', consumer / 'GoblinBarWidget.qml'), ('manifest.json', consumer / 'manifest.json')]:
        shutil.copy2(path, state / name)
    manifest_path = consumer / 'manifest.json'
    manifest = json.loads(manifest_path.read_text())
    icons = manifest['extensions']['data-goblin.fileblade/blade']
    icon = dict(icons[0], id='gallery-icon-test', name='Gallery icon test')
    icons.append(icon)
    manifest_path.write_text(json.dumps(manifest))
    fragment = '''
  Loader {
    source: module.context ? module.context.paths.canonical(module.context.paths.join(module.context.host.pluginDir, "tests/vm/ImageGalleryProbe.qml")) : ""
    onLoaded: item.subject = module
  }
'''
    source = module.read_text()
    module.write_text(source[:source.rfind('}')] + fragment + '}\n')
    widget = consumer / 'GoblinBarWidget.qml'
    source = widget.read_text()
    fragment = '\n  Loader { source: Qt.resolvedUrl("../data-goblin.fileblade/tests/vm/ImageGalleryProbe.qml"); onLoaded: item.subject = widget }\n'
    widget.write_text(source[:source.rfind('}')] + fragment + '}\n')
    root = Path.home() / '.claude/skills/goblin-images'
    sample = next(root.glob('images/**/*.png'))
    library = state / 'library'
    library.mkdir()
    entries = []
    for i in range(24):
        name = f'gallery-{i:02d} #%.png'
        shutil.copy2(sample, library / name)
        entries.append({'path': name, 'filename': name,
                        'modified': ['2026-09-10', '2025-06-01', '2024-01-01', 'invalid'][i // 6],
                        'character': 'bink' if i % 2 == 0 else 'bonk',
                        'tags': ['danger'] if i % 3 == 0 else ['office']})
    (library / 'index.json').write_text(json.dumps(entries))
    document = json.loads(layout.read_text())
    document['blades']['left'].update({'open': True, 'width': 460, 'slots': [
        {'id': 'gallery', 'fraction': 0.72, 'active': 0, 'collapsed': False,
         'modules': [{'module': 'kurt.goblin-images/goblin-images', 'state': {'library': str(library), 'size': 2}}]},
        {'id': 'properties', 'fraction': 0.28, 'active': 0, 'collapsed': False, 'modules': [{'module': 'properties'}]}]})
    document['blades']['right']['open'] = False
    layout.write_text(json.dumps(document))
    shell = json.loads(config.read_text())
    rows = shell['bar']['layout']['right']
    rows[:] = [row for row in rows if row['id'] != 'kurt.goblin-images']
    rows.insert(0, {'id': 'kurt.goblin-images', 'showNavbarIcon': True})
    config.write_text(json.dumps(shell))
else:
    for name, path in [('Module.qml', module), ('shell.json', config), ('blades.json', layout), ('GoblinBarWidget.qml', consumer / 'GoblinBarWidget.qml'), ('manifest.json', consumer / 'manifest.json')]:
        if (state / name).exists():
            shutil.copy2(state / name, path)
    shutil.rmtree(state)
