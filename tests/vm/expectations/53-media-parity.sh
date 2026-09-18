#!/usr/bin/env bash
set -euo pipefail
: "${OVM:?set OVM to the harness executable}"
[[ -n ${OVM_HOME:-} && -n ${OVM_SSH_PORT:-} ]]
python3 - "$(dirname "$0")" <<'PY'
import sys
sys.path.insert(0, sys.argv[1])
import media_lib
from media_lib import *


def sort(key=None, descending=False):
    value = [] if key is None else [{'key': key, 'desc': descending}]
    probe('sort', base64.b64encode(json.dumps(value).encode()).decode())


control('setRoot', '/tmp/brindle-media/library')
control('openBlade', 'left')
control('focusBlade', 'left')
control('setBladeWidth', 'left', 380)
if not state()['mode']:
    probe('toggle')
probe('query', '')
if state()['recursive']:
    probe('recursive')
wait(lambda s: not s['busy'] and s['count'] == 425)
sort()
probe('size', 2)
probe('choose', 200, 'replace')
current = state()
selected = current['selected']
anchor = current['paths'][current['firstVisible']]
for key in ['name', 'size', 'modified']:
    for descending in [False, True]:
        before = state()
        anchor = before['paths'][before['firstVisible']]
        sort(key, descending)
        time.sleep(.2)
        current = state()
        field = [row[key] for row in current['values']]
        check(f'{key} descending={descending} reorders all 425 files through PaneView', field == sorted(field, reverse=descending) and len(field) == 425, {'first': field[0], 'last': field[-1], 'sorts': current['sorts']})
        check(f'{key} descending={descending} preserves exact selection and scroll anchor', current['selected'] == selected and anchor in current['paths'][current['firstVisible']:current['firstVisible'] + current['columns']], {'selected': current['selected'], 'firstVisible': current['paths'][current['firstVisible']], 'anchor': current['anchor'], 'wanted': anchor, 'busy': current['busy']})
shot('53-sorted')
sort()
time.sleep(.2)
probe('choose', 0, 'replace')
wait(lambda s: s['focused'] and s['current'] == 0)
if state()['visual']:
    ovm('key', 'v')
ovm('key', 'v')
wait(lambda s: s['visual'])
ovm('key', 'right')
current = state()
check('V then Right extends horizontal selection', current['visual'] and len(current['selected']) == 2, current['selected'])
ovm('key', 'down')
current = state()
check('VISUAL Down extends by the actual grid column count', current['visual'] and len(current['selected']) == 2 + current['columns'], current['selected'])
shot('53-visual-arrows')
ovm('key', 'v')
ovm('key', 'right')
current = state()
check('ordinary Right returns to a single selection', not current['visual'] and len(current['selected']) == 1, current['selected'])
ovm('key', 'shift-right')
current = state()
check('Shift Right keeps range selection without VISUAL', not current['visual'] and len(current['selected']) == 2, current['selected'])
control('setRoot', '/tmp/brindle-media/library/nested')
wait(lambda s: not s['busy'] and s['count'] == 1)
probe('timelineFocus')
ovm('key', 'alt-left')
current = wait(lambda s: s['root'] == '/tmp/brindle-media/library' and not s['busy'] and s['count'] == 425)
check('timeline focus preserves browser Alt Left navigation', current['root'] == '/tmp/brindle-media/library', current['root'])
print(f'{media_lib.checks} media review checks passed', flush=True)
PY
