#!/usr/bin/env bash
set -euo pipefail
: "${OVM:?set OVM to the harness executable}"
[[ -n ${OVM_HOME:-} && -n ${OVM_SSH_PORT:-} ]]
python3 - "$(dirname "$0")" <<'PY'
import sys
sys.path.insert(0, sys.argv[1])
import media_lib
from media_lib import *


def provider_dates(dates):
    probe('dates', base64.b64encode(json.dumps(dates).encode()).decode())


def timeline():
    return state()['timeline']


def click_bin(index):
    axis = timeline()
    ovm('mouse', 'click', round(axis['x'] + 48), round(axis['y'] + axis['axisTop'] + axis['rowHeight'] * (index + .5)))


def click_detail(down):
    axis = timeline()
    ovm('mouse', 'click', round(axis['x'] + axis['width'] - (13 if down else 39)), round(axis['y'] + 13))


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
probe('size', 2)
probe('choose', 0, 'replace')
current = wait(lambda s: s['size'] == 2)
selected = current['selected']
axis = current['timeline']
check('calendar overview accounts for all current-folder media', axis['count'] == 425 and sum(b['count'] for b in axis['bins']) == 425, axis)
shot('52-overview')
start_y = round(axis['y'] + axis['outlineTop'] + axis['outlineHeight'] / 2)
end_y = round(axis['y'] + axis['axisTop'] + axis['rowHeight'] * 2.5)
x = round(axis['x'] + 48)
script = (repo / 'tests/vm/image-gallery-drag.toml').read_text().replace('gallery-pointer', 'brindle-timeline-scrub').replace('START_X', str(x)).replace('START_Y', str(start_y)).replace('END_X', str(x)).replace('END_Y', str(end_y))
encoded = base64.b64encode(script.encode()).decode()
ovm('ssh', 'python3 -c ' + shlex.quote("import base64;open('/tmp/brindle-timeline-scrub.toml','wb').write(base64.b64decode('" + encoded + "'))"))
ovm('ssh', 'democtl record /tmp/brindle-timeline-scrub.toml --out /tmp --force >/tmp/brindle-timeline-scrub.log 2>&1')
after_drag = state()
check('real pointer drag of the sole outline seeks immediately and keeps selection', after_drag['y'] > current['y'] + 1000 and after_drag['selected'] == selected, {'before': current['y'], 'after': after_drag['y'], 'selected': after_drag['selected']})
shot('52-scrub')
first = next(i for i, b in enumerate(axis['bins']) if b['count'])
if axis['level'] == 'years':
    click_bin(first)
    click_detail(True)
axis = timeline()
check('header down opens months for the navigated year', axis['level'] == 'months', axis)
month = next(i for i, b in enumerate(axis['bins']) if b['count'] and b['key'].endswith('-02'))
click_bin(month)
check('timeline click preserves selection and query', state()['selected'] == selected and state()['query'] == '', state()['selected'])
month_count = timeline()['bins'][month]['count']
click_detail(True)
axis = timeline()
check('header down opens numbered clipped weeks with parent total', axis['level'] == 'weeks' and sum(b['count'] for b in axis['bins']) == month_count, axis)
shot('52-weeks')
week = next(i for i, b in enumerate(axis['bins']) if b['count'])
click_bin(week)
click_detail(True)
axis = timeline()
check('header down opens days with finer hourly detail', axis['level'] == 'days' and axis['down'], axis)
shot('52-days')
day_count = axis['count']
day_keys = [day['key'] for day in axis['bins']]
click_detail(True)
axis = timeline()
check('hourly detail preserves every day row and is the finest level', axis['level'] == 'hours' and axis['count'] == day_count and [day['key'] for day in axis['bins']] == day_keys and not axis['down'], axis)
probe('timelineFocus')
ovm('key', 'left')
check('timeline-focused Left returns to days', timeline()['level'] == 'days', timeline()['level'])
ovm('key', 'left')
check('timeline-focused Left returns to weeks', timeline()['level'] == 'weeks', timeline()['level'])
ovm('key', 'left')
check('timeline-focused Left returns to months', timeline()['level'] == 'months', timeline()['level'])
probe('timelineFocus')
ovm('key', 'end')
check('timeline End seeks last nonempty period without changing selected file', state()['selected'] == selected and timeline()['period'].startswith('Dec'), {'selected': state()['selected'], 'period': timeline()['period']})
root = state()['root']
ovm('key', 'esc')
status = json.loads(ipc('data-goblin.fileblade', 'status'))
check('Escape retains the ordinary blade dismissal behavior', not status['open'] and status['rootPath'] == root, {'open': status['open'], 'root': status['rootPath']})
control('openBlade', 'left')
control('focusBlade', 'left')
time.sleep(.5)
wait(lambda s: not s['busy'] and s['count'] == 425)
probe('choose', state()['paths'].index(selected[0]), 'replace')
for width in [280, 720, 380]:
    control('setBladeWidth', 'left', width)
    probe('size', 4)
    current = state()
    check(f'width {width} retains selection and constant compact axis', current['selected'] == selected and current['timeline']['width'] == axis['width'] and current['cell'] <= width - axis['width'], {'cell': current['cell'], 'axis': current['timeline']['width'], 'selected': current['selected']})
    shot('52-width-' + str(width))
probe('size', 2)
probe('emptyPeriods', 'true')
provider_dates(['2024-02-29'] * 80 + ['2024-12-20'] * 80 + ['2024-12'] * 5 + [''] * 5)
probe('scroll', 0)
axis = timeline()
check('synthetic provider dates keep sparse zero months and Undated counts', axis['level'] == 'months' and axis['bins'][0]['count'] == 0 and axis['bins'][-1]['count'] == 5 and axis['count'] == 170, axis)
shot('52-sparse-undated')
before = state()['y']
click_bin(5)
check('empty month click cannot seek fabricated content', state()['y'] == before, state()['y'])
probe('scroll', 4700)
axis = timeline()
check('viewport crossing two periods lights both with one rectangular span', axis['active'][1] and axis['active'][11] and axis['outlineHeight'] > 0, axis)
shot('52-partial-bins')
click_bin(11)
click_detail(True)
axis = timeline()
check('month-only provider precision has a counted honest child', axis['level'] == 'weeks' and axis['bins'][-1]['key'] == 'unknown' and axis['bins'][-1]['count'] == 5 and axis['count'] == 85, axis)
shot('52-unknown-precision')
provider_dates(['0001-01-01', '2024-02-29', '9999-12-31', ''])
probe('scroll', 0)
axis = timeline()
check('extreme history coarsens to readable year ranges without another scrollbar', axis['level'] == 'ranges' and axis['rowHeight'] >= 18 and axis['count'] == 4, axis)
shot('52-long-history')
provider_dates([])
axis = timeline()
check('empty provider has no viewport outline or finer target', axis['count'] == 0 and axis['outlineHeight'] == 0 and not axis['down'], axis)
shot('52-empty')
probe('emptyPeriods', 'false')
probe('toggle')
probe('toggle')
wait(lambda s: not s['busy'] and s['count'] == 425)
print(f'{media_lib.checks} timeline checks passed', flush=True)
PY
