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
id=fixture.properties40-$$
module=$id/probe
[[ $(guest "test -e '$extensions/$id' && echo exists") == exists ]] && { fail harness fixture "refusing to replace $id"; summary; }
original=$(guest "head -c 262145 '$layout'" | jq -ce '.blades')
backup=$(guest 'mktemp -d /tmp/fileblade-properties40.XXXXXX') || { fail harness backup 'mktemp failed'; summary; }
guest "if test -f '$settings'; then cp -- '$settings' '$backup/settings'; fi" || { fail harness backup 'cannot preserve settings'; summary; }
[[ $(jq -r '(.left.slots | type) == "array" and (.right.slots | type) == "array"' <<< "$original") == true ]] || { fail harness layout 'cannot preserve the original layout'; summary; }
slots() { ctl setBladeSlots "$1" "base64:$(printf %s "$2" | base64 -w0)"; }
cleanup() {
  local edge
  [[ $native == native ]] || guest "omarchy plugin disable '$id'" >/dev/null
  guest "rm -rf -- '$extensions/$id'"
  if [[ $native == native ]]; then
    guest "if test -f '$backup/settings'; then cp -- '$backup/settings' '$settings'; fi"
  fi
  guest "rm -rf -- '$backup'"
  ctl closeBlade left
  sleep 2
  ctl openBlade left
  for edge in left right; do
    slots "$edge" "$(jq -c --arg edge "$edge" '.[$edge].slots' <<< "$original")"
    if [[ $(jq -r --arg edge "$edge" '.[$edge].open' <<< "$original") == true ]]; then ctl openBlade "$edge"; else ctl closeBlade "$edge"; fi
  done
}
trap cleanup EXIT

fixture

code=$(base64 -w0 <<'PY'
import json,pathlib,sys
root,identity=sys.argv[1:]
p=pathlib.Path(root)/identity
p.mkdir()
module={'id':'probe','name':'Properties 40','entry':'Module.qml','hostContract':2,'provider':'Provider.qml','category':'Test'}
manifest={'schemaVersion':1,'id':identity,'name':'Properties 40','version':'0.1.0','kinds':['service'],'entryPoints':{'service':'Provider.qml'},'extensions':{'data-goblin.fileblade/blade':[module]}}
(p/'manifest.json').write_text(json.dumps(manifest))
(p/'Provider.qml').write_text('import QtQuick\nItem { property string providerId: ""; property string providerRoot: ""; property var files: null; property url inventoryComponentUrl: ""; function attach(context) { return true } function detach(context) { return true } function shutdown() {} }\n')
(p/'Module.qml').write_text('''import QtQuick
FocusScope {
  id: module
  required property var context
  readonly property string title: "Properties 40"
  readonly property var properties: context.service("properties")
  property int refreshed: 0
  function publish() {
    if (!properties) return false
    return properties.inspect(context, {
      title: refreshed > 0 ? "Refreshed 40 x" + refreshed : "Fixture item 40",
      subtitle: "Published by the properties fixture",
      glyph: "\\uf0e7",
      color: "accent",
      fields: [
        { label: "Owner", value: "fixture", kind: "text" },
        { label: "Notes", value: "first line\\nsecond line", kind: "multiline" },
        { label: "Tags", value: ["alpha", "beta"], kind: "tags" },
        { label: "Portal", value: "https://example.org/item/40", kind: "link" },
        { label: "Path", value: "copyme", kind: "code" }
      ],
      actions: [{ id: "refresh", text: "Refresh" }]
    })
  }
  function takeFocus(part) { module.forceActiveFocus() }
  Component.onCompleted: publish()
  Component.onDestruction: if (properties) properties.release(context)
  Connections {
    target: module.properties
    ignoreUnknownSignals: true
    function onActionTriggered(ownerModuleId, actionId) {
      if (ownerModuleId === module.context.moduleId && actionId === "refresh") { module.refreshed++; module.publish() }
    }
  }
  Rectangle { anchors.fill: parent; color: "#202020" }
  Text { anchors.centerIn: parent; text: "PUBLISH FIXTURE"; color: "white"; font.pixelSize: 16 }
  MouseArea { anchors.fill: parent; onClicked: { module.context.requestFocus(""); module.publish() } }
}
''')
PY
)
guest "mkdir -p '$extensions'; printf %s '$code' | base64 -d > /tmp/properties40.py; python3 /tmp/properties40.py '$extensions' '$id'; rm -f /tmp/properties40.py"
[[ $native == native ]] || guest "omarchy-shell shell rescanPlugins >/dev/null; for try in \$(seq 20); do omarchy plugin enable '$id' 2>/dev/null && break; sleep 0.5; done" >/dev/null
ctl closeBlade left
sleep 3
ctl openBlade left
listed() { "$OVM" ipc "$PLUGIN.control" bladeModules | jq -e --arg id "$module" 'any(.modules[]; .id == $id)' >/dev/null; }
wait_for listed 10 || { fail harness fixture 'fixture module is not listed'; summary; }
goto_root "$ROOT_DIR"
wait_for "[[ \$(field rootPath) == '$ROOT_DIR' ]]" 10
ctl select "$ROOT_DIR/deep"
wait_for "[[ \$(field selectedPath) == '$ROOT_DIR/deep' ]]" 10
expect E-50-04 'a file is selected before the module publishes' selectedPath "$ROOT_DIR/deep"

slots left "[{\"module\":\"files\"},{\"module\":\"$module\"},{\"module\":\"properties\"}]"
ctl openBlade left
owner() { field propertiesOwner; }
title() { field propertiesTitle; }
if wait_for "[[ \$(owner) == '$module' ]]" 12; then pass E-50-01 'the module item replaces the file in Properties'; else fail E-50-01 'the module item replaces the file in Properties' "owner $(owner)"; fi
expect E-50-01 'status names the shown item' propertiesTitle 'Fixture item 40'
sleep 1
shot=$("$OVM" shot properties40-subject | tail -1)
text=$(ocr_crop properties40-pane "$(field sidebarWidth)x360+0+720" 300% 6)
for word in 'Fixture item' 'Refresh' 'first line' 'example.org'; do
  expect_contains E-50-01 "pane shows $word" "$text" "$word"
done
muted=$(ocr_crop properties40-muted "$(field sidebarWidth)x360+0+720" 300% 6 '3%,22%')
for word in 'Published' 'TAGS' 'PORTAL'; do
  expect_contains E-50-01 "pane shows muted $word" "$muted" "$word"
done
printf 'screenshot %s\n' "$shot"

click_word Refresh
if wait_for "[[ \$(title) == 'Refreshed 40 x1' ]]" 8; then pass E-50-02 'a click on an action runs it in the module'; else fail E-50-02 'a click on an action runs it in the module' "title $(title)"; fi
pending E-50-02 'a click on a link opens the browser' 'the guest has no browser session to observe; tests/properties_primitives.rs proves the backend hand-off to gio open'

click_word Refreshed
"$OVM" key j; "$OVM" key j; "$OVM" key j; "$OVM" key j
guest "printf stale | timeout 3 wl-copy >/dev/null 2>&1 </dev/null; true"
"$OVM" key ret
sleep 1
expect_out E-50-03 'Enter on code copies it' 'timeout 3 wl-paste -n' 'copyme'
"$OVM" key delete
"$OVM" key f2
sleep 1
expect_out E-50-03 'file shortcuts leave the selected file alone' "test -d '$ROOT_DIR/deep' && echo kept" kept
expect E-50-03 'file shortcuts keep the item shown' propertiesOwner "$module"
guest "printf stale | timeout 3 wl-copy >/dev/null 2>&1 </dev/null; true"
click_word copyme
expect_out E-50-02 'a click on code copies it' 'timeout 3 wl-paste -n' 'copyme'
"$OVM" key g
"$OVM" key ret
if wait_for "[[ \$(title) == 'Refreshed 40 x2' ]]" 8; then pass E-50-03 'Enter on an action runs it'; else fail E-50-03 'Enter on an action runs it' "title $(title)"; fi
"$OVM" key esc
sleep 1

ensure_left_open
index=$(row_index deep)
row=$( [[ $index =~ ^[0-9]+$ ]] && row_y "$index")
if [[ $row =~ ^[0-9]+$ ]]; then
  "$OVM" mouse click "$ROW_X" "$row"
  if wait_for "[[ -z \$(owner) ]]" 8; then pass E-50-04 'selecting the same file again brings the file back'; else fail E-50-04 'selecting the same file again brings the file back' "owner $(owner)"; fi
  expect E-50-04 'the file selection is unchanged' selectedPath "$ROOT_DIR/deep"
else
  fail E-50-04 'selecting the same file again brings the file back' 'deep row not found'
fi
click_word PUBLISH
if wait_for "[[ \$(owner) == '$module' ]]" 8; then pass E-50-04 'selecting in the module shows its item again'; else fail E-50-04 'selecting in the module shows its item again' "owner $(owner)"; fi
slots left '[{"module":"files"},{"module":"properties"}]'
if wait_for "[[ -z \$(owner) ]]" 8; then pass E-50-04 'removing the module pane returns to the file'; else fail E-50-04 'removing the module pane returns to the file' "owner $(owner)"; fi
summary
