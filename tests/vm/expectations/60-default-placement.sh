#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
require_guest

shape=${FILEBLADE_SHAPE:-plugin}
layout=$(field bladeLayoutPath)
[[ $layout == /home/omarchy/*/blades.json ]] || { fail harness layout "$layout"; summary; }
config=${layout%/*}
if [[ $shape == native ]]; then
  extensions=${config%/omarchy/fileblade}/fileblade/extensions
else
  extensions=/home/omarchy/.config/omarchy/plugins
fi
original=$("$OVM" ipc "$PLUGIN" blades | jq -ce '.blades')
[[ $(jq -r '(.left.slots | type) == "array" and (.right.slots | type) == "array"' <<< "$original") == true ]] || { fail harness layout 'cannot preserve the original layout'; summary; }
ids=(fixture.placement60-a-$$ fixture.placement60-b-$$ fixture.placement60-plain-$$ fixture.placement60-hand-$$ fixture.placement60-late-$$)
for id in "${ids[@]}"; do
  [[ $(guest "test -e '$extensions/$id' && echo exists") == exists ]] && { fail harness fixture "refusing to replace $id"; summary; }
done
guest "mkdir -p '$extensions'"
slots() { ctl setBladeSlots "$1" "base64:$(printf %s "$2" | base64 -w0)"; }
cleanup() {
  local edge id
  for id in "${ids[@]}"; do
    [[ $shape == native ]] || guest "omarchy plugin disable '$id'" >/dev/null
    guest "rm -r -- '$extensions/$id' 2>/dev/null"
  done
  for edge in left right; do
    slots "$edge" "$(jq -c --arg edge "$edge" '.[$edge].slots' <<< "$original")"
    if [[ $(jq -r --arg edge "$edge" '.[$edge].open' <<< "$original") == true ]]; then ctl openBlade "$edge"; else ctl closeBlade "$edge"; fi
  done
}
trap cleanup EXIT

write_fixture() {
  local placement=$2
  local code
  code=$(base64 -w0 <<'PY'
import json,pathlib,sys
root,identity,name,placement=sys.argv[1:]
p=pathlib.Path(root)/identity
p.mkdir()
module={'id':'probe','name':name,'entry':'Module.qml','provider':None,'hostContract':2,'category':'Data'}
if placement!='none': module['defaultPlacement']=placement
manifest={'schemaVersion':1,'id':identity,'name':name,'version':'0.1.0','kinds':['service'],'entryPoints':{'service':'Service.qml'},'extensions':{'data-goblin.fileblade/blade':[module]}}
(p/'manifest.json').write_text(json.dumps(manifest))
(p/'Service.qml').write_text('import QtQuick\nItem {}\n')
(p/'Module.qml').write_text('import QtQuick\nItem { property var context: null; readonly property string title: '+json.dumps(name)+'; Text { anchors.centerIn: parent; text: '+json.dumps(name.upper()+' CONTENT')+'; color: "white" } }\n')
PY
)
  guest "printf %s '$code' | base64 -d | python3 - '$extensions' '$1' '$3' '$placement'"
}
placements=(beside-files beside-files none beside-files beside-files)
names=('Placement A' 'Placement B' 'Placement Plain' 'Placement Hand' 'Placement Late')
if [[ $shape != native ]]; then
  for i in "${!ids[@]}"; do write_fixture "${ids[$i]}" "${placements[$i]}" "${names[$i]}"; done
  guest "omarchy-shell shell rescanPlugins >/dev/null 2>&1" >/dev/null
  sleep 8
  wait_for "document | jq -e '.blades.left.slots' >/dev/null" 20
fi
install() {
  local i
  for i in "${!ids[@]}"; do [[ ${ids[$i]} == "$1" ]] && break; done
  if [[ $shape == native ]]; then
    write_fixture "$1" "${placements[$i]}" "${names[$i]}"
    ctl rescanBladeModules
  else
    guest "for try in \$(seq 20); do omarchy plugin enable '$1' 2>/dev/null && break; sleep 0.5; done" >/dev/null
  fi
}
same() { [[ $3 == "$4" ]] && pass "$1" "$2" || fail "$1" "$2" "got [$3] wanted [$4]"; }
module_of() { printf '%s/probe' "$1"; }
document() { "$OVM" ipc "$PLUGIN" blades; }
files_slot() { document | jq -c '[.blades[] | .slots[] | select(any(.modules[]; .module == "files"))][0] | {active, modules: [.modules[].module]}'; }
count_of() { document | jq --arg m "$1" '[.blades[] | .slots[] | .modules[] | select(.module == $m)] | length'; }
recorded() { document | jq -e --arg m "$1" '.defaultPlacements | index($m) != null' >/dev/null; }
listed() { "$OVM" ipc "$PLUGIN.control" bladeModules | jq -e --arg id "$1" 'any(.modules[]; .id == $id)' >/dev/null; }

a=$(module_of "${ids[0]}") b=$(module_of "${ids[1]}") plain=$(module_of "${ids[2]}") hand=$(module_of "${ids[3]}") late=$(module_of "${ids[4]}")
slots left '[{"id":"files","modules":[{"module":"files"},{"module":"notes"}],"active":1},{"id":"properties","modules":[{"module":"properties"}]}]'
slots right '[]'
ctl openBlade left
ctl closeBlade right
sleep 2
right_open_before=$(document | jq -r '.blades.right.open')

install "${ids[0]}"
wait_for "listed '$a'" 12 || fail E-60-01 'first extension is discovered' 'not listed within 12 seconds'
if wait_for "[[ \$(files_slot | jq -r '.modules | join(\",\")') == 'files,notes,$a' ]]" 12; then pass E-60-01 'a beside-files module lands after the tabs in the Files section'; else fail E-60-01 'a beside-files module lands after the tabs in the Files section' "$(files_slot)"; fi
ctl setBladeTab left 0 1
install "${ids[1]}"
if wait_for "[[ \$(files_slot | jq -r '.modules | join(\",\")') == 'files,notes,$a,$b' ]]" 12; then pass E-60-01 'a second extension follows the first in the same section'; else fail E-60-01 'a second extension follows the first in the same section' "$(files_slot)"; fi
install "${ids[2]}"
wait_for "listed '$plain'" 12
sleep 2
same E-60-01 'a module without the field is listed but not placed' "$(count_of "$plain")" 0
same E-60-02 'the tab I was looking at stays current' "$(files_slot | jq -r .active)" 1
same E-60-02 'no blade opens on its own' "$(document | jq -r '.blades.right.open')" "$right_open_before"
"$OVM" shot 60-default-placement-tabs >/dev/null

current=$(document | jq -c '.blades.left.slots')
slots left "$(jq -c --arg a "$a" --arg b "$b" 'map(.modules |= map(select(.module != $a)))' <<< "$current")"
slots right "[{\"id\":\"moved\",\"modules\":[{\"module\":\"$b\"}]}]"
slots left "$(document | jq -c --arg b "$b" '.blades.left.slots | map(.modules |= map(select(.module != $b)))')"
ctl rescanBladeModules
if [[ $shape != native ]]; then
  guest "omarchy plugin disable '${ids[0]}'; sleep 1; omarchy plugin enable '${ids[0]}'" >/dev/null
fi
sleep 4
same E-60-03 'a closed tab is not added back after rescan and re-enable' "$(count_of "$a")" 0
same E-60-03 'a moved tab stays where I moved it' "$(document | jq -r --arg b "$b" '[.blades.right.slots[].modules[] | select(.module == $b)] | length')" 1
recorded "$a" && pass E-60-03 'the record keeps the closed module' || fail E-60-03 'the record keeps the closed module' "$(document | jq -c .defaultPlacements)"
ctl closeBlade left
sleep 2
ctl openBlade left
sleep 3
same E-60-03 'reopening the blade leaves it closed' "$(count_of "$a")" 0

slots left "$(document | jq -c --arg h "$hand" '.blades.left.slots + [{"id":"hand","modules":[{"module":$h}]}]')"
install "${ids[3]}"
wait_for "recorded '$hand'" 12
same E-60-04 'a module I placed by hand is not added twice' "$(count_of "$hand")" 1

slots left "$(document | jq -c '.blades.left.slots | map(.modules |= map(select(.module != "files"))) | map(select(.modules | length > 0))')"
install "${ids[4]}"
if wait_for "[[ \$(document | jq -r --arg l '$late' --arg h '$hand' '[.blades[].slots[] | select(any(.modules[]; .module == \$h)) | any(.modules[]; .module == \$l)] | any') == true ]]" 12; then
  pass E-60-05 'without a Files tab the module joins an earlier placed sibling'
else
  fail E-60-05 'without a Files tab the module joins an earlier placed sibling' "$(document | jq -c '[.blades[].slots[] | [.modules[].module]]')"
fi

ctl resetBladeLayout
if wait_for "[[ \$(files_slot | jq -r '.modules | map(select(startswith(\"fixture.placement60-\"))) | length') -ge 4 ]]" 15; then
  pass E-60-06 'resetting the layout puts every beside-files module next to FileBlade again'
else
  fail E-60-06 'resetting the layout puts every beside-files module next to FileBlade again' "$(files_slot)"
fi
same E-60-06 'the reset leaves modules without the field unplaced' "$(count_of "$plain")" 0
summary
