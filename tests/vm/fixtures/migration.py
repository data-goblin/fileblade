import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess


def write(path, value):
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    path.write_bytes(value if isinstance(value, bytes) else json.dumps(value, indent=2).encode() + b'\n')
    path.chmod(0o600)


def generate(destination, scenario):
    destination.mkdir(mode=0o700)
    root = Path(__file__).resolve().parents[3]
    home = destination / 'legacy-home'
    config_home = home / '.config'
    state_home = home / '.local/state'
    data_home = home / '.local/share'
    config = config_home / 'omarchy/fileblade'
    state = state_home / 'omarchy/fileblade'
    recovery = state_home / 'fileblade'
    project = home / 'project'
    project.mkdir(mode=0o700, parents=True)
    (project / '.git').mkdir(mode=0o700)
    native = destination / 'native'
    native_roots = {'config': native / 'config/omarchy/fileblade', 'state': native / 'state/omarchy/fileblade', 'recovery': native / 'state/fileblade'}
    for path in (config, state, recovery, *native_roots.values()):
        path.mkdir(mode=0o700, parents=True, exist_ok=True)
    environment = dict(os.environ, HOME=str(home), XDG_CONFIG_HOME=str(config_home),
                       XDG_STATE_HOME=str(state_home), XDG_CACHE_HOME=str(home / '.cache'), XDG_DATA_HOME=str(data_home),
                       CODEX_HOME=str(home / '.codex'), CLAUDE_CONFIG_DIR=str(home / '.claude'),
                       FILEBLADE_APP_ROOT=str(root), PYTHONDONTWRITEBYTECODE='1')
    environment.pop('FILEBLADE_NATIVE_STATE_ROOT', None)
    environment['FILEBLADE_BINARY'] = os.environ.get('FILEBLADE_BINARY', str(root / 'fileblade-bin'))
    modules = ('skills', 'memory', 'hooks', 'mcp')
    providers = ['data-goblin.fileblade-' + module for module in modules]
    slots = []
    sources = {}
    for module, provider in zip(modules, providers):
        manifest_path = Path(__file__).with_name('migration-manifests') / (module + '.json')
        raw = manifest_path.read_bytes()
        provenance = json.loads((root / 'modules' / module / 'source.json').read_text())
        assert hashlib.sha256(raw).hexdigest() == provenance['manifestSha256']
        write(config_home / 'omarchy/plugins' / provider / 'manifest.json', raw)
        sources[module] = provenance
        identifier = provider + '/' + module
        slots.append({'id': 'legacy-' + module, 'active': 0, 'collapsed': False,
                      'modules': [{'module': identifier, 'state': {'future': {'preserve': module}}}]})
        write(state / 'modules' / identifier.replace('/', '+') / 'opaque.bin', bytes([0, 255, 13, 10]))
        write(config / 'config' / identifier.replace('/', '+') / 'overrides.json', {'future': module})
    slots.append({'id': 'notebook', 'active': 1, 'modules': [
        {'module': 'notes', 'state': {'text': {'version': 2, 'revision': 7, 'activeId': 'note-2', 'items': [
            {'id': 'note-1', 'label': 'First', 'text': 'Preserved notebook\n'},
            {'id': 'note-2', 'label': 'Second', 'text': 'Second tab\n'}]}}},
        {'module': 'data-goblin.goblins/goblins', 'state': {'stackLayers': False, 'future': 42}}
    ]})
    write(config / 'blades.json', {'version': 1, 'blades': {'left': {'open': True, 'slots': slots},
          'right': {'open': False, 'slots': []}}, 'future': {'preserve': True}})
    write(state / 'state.json', {'version': 12, 'rootPath': str(project), 'welcomeState': 'dismissed',
          'confirmTrash': True, 'favorites': [], 'searchHistory': ['user edited'], 'future': {'keep': [1, 2, 3]}})
    write(config / 'settings.json', {'version': 1, 'filebladeVersion': '0.1.2', 'agentManagement': True,
          'trashRetentionDays': 0, 'future': {'custom': True}})
    write(config / 'keybindings.json', {'version': 1, 'filebladeVersion': '0.1.2',
          'bindings': {'show': 'SUPER, E', 'custom': 'CTRL, K'}, 'future': {'owner': 'user'}})
    write(config_home / 'hypr/bindings.lua', b'local user_binding = "preserve exactly"\n')
    write(config / 'hooks/labels.json', {'version': 1, 'labels': {'1234abcd': 'My custom label'}})
    write(config_home / 'omarchy/shell.json', {'plugins': [{'id': value} for value in ['data-goblin.fileblade', *providers, 'data-goblin.goblins']],
          'disabledPlugins': [] if scenario == 'active' else ['data-goblin.fileblade', *providers]})
    recovery_entries = {}
    for module in ('hooks', 'mcp'):
        path = project / ('.claude/settings.json' if module == 'hooks' else '.mcp.json')
        value = ({'hooks': {'PreToolUse': [{'hooks': [{'type': 'command', 'command': 'printf MIGRATION_FIXTURE'}]}]}}
                 if module == 'hooks' else {'mcpServers': {'migration-fixture': {'command': 'printf', 'args': ['MIGRATION_FIXTURE']}}})
        write(path, value)
        listed = subprocess.run([environment['FILEBLADE_BINARY'], '_backend', 'helper-read',
                                 '--provider', 'fileblade.core.' + module, '--plugin-dir', '',
                                 '--helper', 'inventory', '--method', 'list',
                                 '--arguments', json.dumps(['--project', str(project), '--json'])],
                                env=environment, capture_output=True, check=True, timeout=15)
        listing = json.loads(listed.stdout)
        row = next(row for row in listing['items' if module == 'hooks' else 'definitions']
                   if row['scope'] == 'project' and row.get('agentId', row.get('agent')) == 'claude-code')
        route = {'provider': 'data-goblin.fileblade-' + module, 'directory': str(config_home / 'omarchy/plugins' / ('data-goblin.fileblade-' + module)), 'helper': 'inventory'}
        command = [environment['FILEBLADE_BINARY'], '_backend', 'bin-remove', '--module', module,
                   '--item', json.dumps({'id': row['id'], 'name': 'Migration fixture'}),
                   '--helper-route', json.dumps(route), '--arguments', json.dumps(['--project', str(project), '--id', row['id'], '--json'])]
        removed = json.loads(subprocess.run(command, env=environment, capture_output=True, check=True, timeout=20).stdout)
        if not removed.get('ok'):
            raise RuntimeError(removed)
        recovery_entries[module] = removed['entry']
    write(project / 'restorable.txt', b'Restored link target\n')
    link = project / 'restorable-link'
    link.symlink_to('restorable.txt')
    item = {'id': 'migration-link', 'name': 'Stored symbolic link', 'paths': [str(link)]}
    removed = json.loads(subprocess.run([environment['FILEBLADE_BINARY'], '_backend', 'bin-put',
                         '--module', 'skills', '--item', json.dumps(item)], env=environment,
                         capture_output=True, check=True, timeout=20).stdout)
    if not removed.get('ok'):
        raise RuntimeError(removed)
    recovery_entries['skills'] = removed['entry']
    attribute_file = project / 'attribute-file'
    attribute_tree = project / 'attribute-tree'
    write(attribute_file, b'Attribute file bytes\n')
    write(attribute_tree / 'child', b'Attribute child bytes\n')
    os.setxattr(attribute_file, 'user.fileblade-fixture', bytes([0, 255, 13, 10]))
    os.setxattr(attribute_tree, 'user.fileblade-fixture', b'directory attribute')
    os.setxattr(attribute_tree / 'child', 'user.fileblade-fixture', b'child attribute')
    item = {'id': 'migration-attributes', 'name': 'Metadata fixture', 'paths': [str(attribute_file), str(attribute_tree)]}
    removed = json.loads(subprocess.run([environment['FILEBLADE_BINARY'], '_backend', 'bin-put',
                         '--module', 'memory', '--item', json.dumps(item)], env=environment,
                         capture_output=True, check=True, timeout=20).stdout)
    if not removed.get('ok'):
        raise RuntimeError(removed)
    recovery_entries['memory'] = removed['entry']
    if attribute_file.exists() or attribute_tree.exists():
        raise RuntimeError('bin-put left an original attribute fixture in place')
    if scenario == 'malformed':
        write(state / 'state.json', b'{ malformed original\n')
    elif scenario == 'newer':
        write(config / 'settings.json', {'version': 2, 'futureConsent': 'do not reinterpret'})
    elif scenario == 'missing-recovery':
        next((recovery / 'hooks-recovery').glob('*.json')).unlink()
    inventory = {}
    for path in sorted(home.rglob('*')):
        if path.is_file() and not path.is_symlink():
            inventory[str(path.relative_to(home))] = hashlib.sha256(path.read_bytes()).hexdigest()
    document = {'version': 1, 'scenario': scenario,
                'legacy': {'config': str(config), 'state': str(state), 'recovery': str(recovery)},
                'native': {role: str(path) for role, path in native_roots.items()},
                'artifactBin': str(data_home / 'fileblade/bin'), 'sources': sources,
                'recoveryEntries': recovery_entries, 'attributeFile': str(attribute_file), 'attributeTree': str(attribute_tree), 'originalSha256': inventory}
    write(destination / 'fixture.json', document)
    return document


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('destination', type=Path)
    parser.add_argument('--scenario', choices=('stopped', 'active', 'malformed', 'newer', 'missing-recovery'), default='stopped')
    options = parser.parse_args()
    if Path.home() != Path('/home/omarchy'):
        raise SystemExit('Run this fixture generator only in the assigned Omarchy test guest')
    print(json.dumps(generate(options.destination.absolute(), options.scenario)))
