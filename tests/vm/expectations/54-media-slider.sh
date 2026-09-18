#!/usr/bin/env bash
set -euo pipefail
: "${OVM:?set OVM to the harness executable}"
[[ -n ${OVM_HOME:-} && -n ${OVM_SSH_PORT:-} ]]
python3 - "$(dirname "$0")" <<'PY'
import sys
sys.path.insert(0, sys.argv[1])
import media_lib
from media_lib import *


control('setRoot', '/tmp/brindle-media/library')
control('closeBlade', 'right')
control('openBlade', 'left')
control('focusBlade', 'left')
control('setBladeWidth', 'left', 380)
if not state()['mode']:
    probe('toggle')
probe('query', '')
if state()['recursive']:
    probe('recursive')
wait(lambda s: not s['busy'] and s['count'] == 425)
probe('size', 0)
probe('choose', 200, 'replace')
selected = state()['selected']
slider = state()['slider']
x = round(slider['x'] + 16 - 3 + 14)
y = round(slider['y'] + slider['height'] / 2)
end = round(slider['x'] + slider['width'] - 48 - 14)
script = (repo / 'tests/vm/image-gallery-drag.toml').read_text().replace('gallery-pointer', 'brindle-media-slider').replace('START_X', str(x)).replace('START_Y', str(y)).replace('END_X', str(end)).replace('END_Y', str(y))
script = script.replace('hold_ms = 200', 'hold_ms = 1000')
encoded = base64.b64encode(script.encode()).decode()
ovm('ssh', 'python3 -c ' + shlex.quote("import base64;open('/tmp/brindle-media-slider.toml','wb').write(base64.b64decode('" + encoded + "'))"))
ovm('ssh', 'setsid democtl record /tmp/brindle-media-slider.toml --out /tmp --force >/tmp/brindle-media-slider-record.log 2>&1 </dev/null &')
seen = set()
lost_focus = False
lost_selection = False
pressed = False
deadline = time.monotonic() + 15
while time.monotonic() < deadline:
    current = state()
    if current['slider']['pressed']:
        pressed = True
        seen.add(current['size'])
        lost_focus |= not current['slider']['focused']
        lost_selection |= current['selected'] != selected
    elif pressed:
        break
    time.sleep(.05)
check('one continuous thumb drag reaches all five live positions', seen == {0, 1, 2, 3, 4}, sorted(seen))
check('drag and reflow retain slider focus and selected path', not lost_focus and not lost_selection and state()['slider']['focused'], {'lost_focus': lost_focus, 'lost_selection': lost_selection, 'selected': state()['selected']})
check('release leaves XL with pointer ownership released', state()['size'] == 4 and not state()['slider']['pressed'], state()['slider'])
shot('54-slider-xl')
for key, wanted in [('home', 0), ('left', 0), ('right', 1), ('right', 2), ('right', 3), ('end', 4), ('right', 4), ('minus', 3)]:
    ovm('key', key)
    current = wait(lambda s: s['size'] == wanted)
    check('focused slider ' + key + ' reaches ' + str(wanted), current['slider']['focused'] and current['selected'] == selected, {'size': current['size'], 'focused': current['slider']['focused']})
control('setBladeWidth', 'left', 280)
current = wait(lambda s: s['slider']['x'] < slider['x'])
slider = current['slider']
ovm('mouse', 'click', round(slider['x'] + slider['width'] - 36), round(slider['y'] + slider['height'] / 2))
current = wait(lambda s: s['size'] == 4)
check('narrow-blade plus reaches XL with focus', current['slider']['focused'] and current['selected'] == selected, {'width': slider['width'], 'size': current['size']})
ovm('mouse', 'click', round(slider['x'] + 12), round(slider['y'] + slider['height'] / 2))
current = wait(lambda s: s['size'] == 3)
check('narrow-blade minus reaches L without changing selection', current['slider']['focused'] and current['selected'] == selected, {'size': current['size'], 'selected': current['selected']})
shot('54-slider-narrow')
probe('toggle')
probe('toggle')
current = wait(lambda s: not s['busy'] and s['count'] == 425)
check('mode round trip retains media size', current['size'] == 3, current['size'])
control('setBladeWidth', 'left', 380)
control('setRoot', '/tmp/brindle-media/library/nested')
current = wait(lambda s: s['root'].endswith('/nested') and not s['busy'] and s['count'] == 1)
slider = current['slider']
ovm('mouse', 'click', round(slider['x'] + slider['width'] - 36), round(slider['y'] + slider['height'] / 2))
wait(lambda s: s['size'] == 4 and s['slider']['focused'])
ovm('key', 'alt-left')
current = wait(lambda s: s['root'] == '/tmp/brindle-media/library' and not s['busy'])
check('Alt Left from slider focus navigates back without resizing', current['size'] == 4, {'root': current['root'], 'size': current['size']})
slider = current['slider']
ovm('mouse', 'click', round(slider['x'] + 12), round(slider['y'] + slider['height'] / 2))
wait(lambda s: s['size'] == 3 and s['slider']['focused'])
ovm('key', 'esc')
status = json.loads(ipc('data-goblin.fileblade', 'status'))
check('Escape from slider focus retains blade dismissal', not status['open'] and status['rootPath'] == '/tmp/brindle-media/library', {'open': status['open'], 'root': status['rootPath']})
control('openBlade', 'left')
print(f'{media_lib.checks} media slider checks passed', flush=True)
PY
