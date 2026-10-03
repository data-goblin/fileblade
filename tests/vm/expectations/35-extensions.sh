#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
require_guest

native=${FILEBLADE_SHAPE:-plugin}
layout=$(field bladeLayoutPath)
[[ $layout == /home/omarchy/*/blades.json ]] || { fail harness layout "$layout"; summary; }
config=${layout%/*}
settings=$config/settings.json
if [[ $native == native ]]; then
  extensions=${config%/omarchy/fileblade}/fileblade/extensions
else
  extensions=/home/omarchy/.config/omarchy/plugins
fi
case_dir=$(guest 'mktemp -d /tmp/fileblade-extensions.XXXXXX') || { fail harness fixture 'mktemp failed'; summary; }
original=$(guest "head -c 262145 '$layout'" | jq -ce '.blades')
[[ $(jq -r '(.left.slots | type) == "array" and (.right.slots | type) == "array"' <<< "$original") == true ]] || { fail harness layout 'cannot preserve original module state on both edges'; summary; }
ids=(fixture.extensions35-$$ fixture.extensions35-late-$$ fixture.extensions35-old-$$)
for id in "${ids[@]}"; do
  if [[ $(guest "test -e '$extensions/$id' && echo exists") == exists ]]; then
    fail harness fixture "refusing to replace $id"
    summary
  fi
done
guest "mkdir -p '$extensions'; if test -f '$settings'; then cp -- '$settings' '$case_dir/settings'; fi" || { fail harness backup 'cannot preserve settings'; summary; }
slots() { ctl setBladeSlots "$1" "base64:$(printf %s "$2" | base64 -w0)"; }
cleanup() {
  local edge id
  for id in "${ids[@]}"; do
    [[ $native == native ]] || guest "omarchy plugin disable '$id'" >/dev/null
    guest "rm -rf -- '$extensions/$id'"
  done
  if [[ $native == native ]]; then
    guest "if test -f '$case_dir/settings'; then cp -- '$case_dir/settings' '$settings'; else rm -f -- '$settings'; fi"
  fi
  ctl closeBlade left
  sleep 2
  ctl openBlade left
  for edge in left right; do
    slots "$edge" "$(jq -c --arg edge "$edge" '.[$edge].slots' <<< "$original")"
    if [[ $(jq -r --arg edge "$edge" '.[$edge].open' <<< "$original") == true ]]; then ctl openBlade "$edge"; else ctl closeBlade "$edge"; fi
  done
  guest "rm -rf -- '$case_dir'"
}
trap cleanup EXIT

create() {
  local code
  code=$(base64 -w0 <<'PY'
import json,pathlib,sys,time
root,case,identity,legacy=sys.argv[1:]
p=pathlib.Path(root)/identity
p.mkdir()
if legacy=='delayed': time.sleep(3)
module={'id':'probe','name':'Extension 35','entry':'Module.qml','hostContract':2}
if legacy!='legacy': module['provider']='Provider.qml'
manifest={'schemaVersion':1,'id':identity,'name':'Extension 35','version':'0.1.0','kinds':['service'],'entryPoints':{'service':'Provider.qml'},'extensions':{'data-goblin.fileblade/blade':[module]}}
(p/'manifest.json').write_text(json.dumps(manifest))
if legacy=='delayed': time.sleep(3)
(p/'Module.qml').write_text('import QtQuick\nItem { property var context: null; readonly property string title: "Extension 35"; Text { anchors.centerIn: parent; text: "EXTENSION 35 CONTENT"; color: "white" } }\n')
marker=json.dumps(case+'/'+identity)
(p/'Provider.qml').write_text('import QtQuick\nimport Quickshell\nItem { property string providerId: ""; property string providerRoot: ""; property var files: null; property url inventoryComponentUrl: ""; Component.onCompleted: Quickshell.execDetached(["touch", '+marker+' + ".started"]); function shutdown() { Quickshell.execDetached(["touch", '+marker+' + ".stopped"]) } }\n')
PY
)
  guest "printf %s '$code' | base64 -d > '$case_dir/create.py'; python3 '$case_dir/create.py' '$extensions' '$case_dir' '$1' '${2:-owned}'"
  [[ $native == native ]] || guest "omarchy-shell shell rescanPlugins >/dev/null; for try in \$(seq 20); do omarchy plugin enable '$1' 2>/dev/null && break; sleep 0.5; done" >/dev/null
}
activation() {
  if [[ $native != native ]]; then
    guest "omarchy plugin $1 '${ids[0]}'" >/dev/null
    return
  fi
  local code
  code=$(base64 -w0 <<'PY'
import json,pathlib,sys
p=pathlib.Path(sys.argv[1]); d=json.loads(p.read_text())
if sys.argv[2]=='invalid': d['extensions']='invalid-fixture'
else: d['extensions'][sys.argv[3]]['enabled']=sys.argv[2]=='enable'
q=p.with_name('.extensions35-next');q.touch(mode=0o600,exist_ok=False);q.write_text(json.dumps(d));q.replace(p)
PY
)
  guest "printf %s '$code' | base64 -d > '$case_dir/activation.py'; python3 '$case_dir/activation.py' '$settings' '$1' '${ids[0]}'"
}
listed() { "$OVM" ipc "$PLUGIN.control" bladeModules | jq -e --arg id "$1/probe" 'any(.modules[]; .id == $id)' >/dev/null; }
await_list() {
  if wait_for "listed '$2'" 8; then pass "$1" "$3"; else fail "$1" "$3" 'module not listed within 8 seconds'; fi
}
place() { slots left "[{\"id\":\"extensions35\",\"modules\":[{\"module\":\"$1/probe\"}]}]"; ctl openBlade left; ctl focusBlade left; }
content() { ocr_crop extensions35-content "$(field sidebarWidth)x1022+0+26" 300% 6 '0%,100%'; }

create "${ids[0]}"
ctl closeBlade left
sleep 3
ctl openBlade left
await_list E-35-01 "${ids[0]}" 'installed extension is in the module catalogue'
place "${ids[0]}"
sleep 2
expect_contains E-35-01 'extension content renders' "$(content)" 'EXTENSION 35 CONTENT'
expect_out E-35-01 'owned provider starts' "test -f '$case_dir/${ids[0]}.started' && echo started" started
pending E-35-01 'Welcome, settings and module picker presentation' 'catalogue and blade checked; all three chooser surfaces still need pointer qualification'

activation disable
if wait_for "! listed '${ids[0]}'" 8; then pass E-35-02 'disable removes the active module'; else fail E-35-02 'disable removes the active module' 'still listed after 8 seconds'; fi
expect_out E-35-02 'disable shuts down the provider' "test -f '$case_dir/${ids[0]}.stopped' && echo stopped" stopped
activation enable
await_list E-35-02 "${ids[0]}" 'enable restores the module without restart'

create "${ids[1]}" delayed
await_list E-35-03 "${ids[1]}" 'extension written in stages appears while blades stay open'
if [[ $native == native ]]; then
  sleep 3
  guest "cp -- '$extensions/${ids[1]}/manifest.json' '$case_dir/valid-manifest'; printf '{}' > '$extensions/${ids[1]}/manifest.json'"
  if wait_for "! listed '${ids[1]}'" 8; then pass E-35-03 'corrupt manifest removes the extension'; else fail E-35-03 'corrupt manifest removes the extension' 'still listed after 8 seconds'; fi
  sleep 3
  guest "cp -- '$case_dir/valid-manifest' '$extensions/${ids[1]}/manifest.json'"
  await_list E-35-03 "${ids[1]}" 'repaired manifest restores the extension without reopening'
fi
ctl openBlade right
ctl closeBlade left
sleep 3
ctl openBlade left
await_list E-35-03 "${ids[1]}" 'reopening one blade discovers the new extension'

if [[ $native == native ]]; then
  guest "cp -- '$settings' '$case_dir/valid-settings'"
  activation invalid
  if wait_for "! listed '${ids[0]}'" 8; then pass E-35-04 'unknown activation removes execution authority'; else fail E-35-04 'unknown activation removes execution authority' 'provider remains listed'; fi
  expect_contains E-35-04 'unavailable extension keeps its saved tab' "$("$OVM" ipc "$PLUGIN" blades)" "${ids[0]}/probe"
  expect_contains E-35-04 'saved tab reports unavailability' "$(content)" disabled
  guest "cp -- '$case_dir/valid-settings' '$settings'"
else
  pending E-35-04 'unreadable activation inventory' 'requires isolated CLI failure injection; no shared shell executable is replaced'
fi

create "${ids[2]}" legacy
ctl closeBlade left
sleep 3
ctl openBlade left
await_list E-35-05 "${ids[2]}" 'legacy extension remains discoverable'
place "${ids[2]}"
sleep 2
if [[ $native == native ]]; then
  expect_contains E-35-05 'legacy provider shows an update notice without disclosure' "$(content)" 'needs an update'
else
  pending E-35-05 'both shell disclosure contracts' 'run on named self-scoped and shared-registry shell versions; native checks the no-registry path'
fi
summary
