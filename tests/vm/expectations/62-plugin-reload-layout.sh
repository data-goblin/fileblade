#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
require_guest

if [[ $FILEBLADE_SHAPE == native ]]; then
  pending E-62-01 "a plugin rescan keeps the tab I chose" "the shell's plugin rescan does not reload the native app"
  pending E-62-02 "a change made just before a rescan survives it" "the shell's plugin rescan does not reload the native app"
  summary
fi

document() { "$OVM" ipc "$PLUGIN" blades 2>/dev/null; }
original=$(document | jq -ce '.blades')
[[ $(jq -r '(.left.slots | type) == "array" and (.right.slots | type) == "array"' <<< "$original") == true ]] || { fail harness layout 'cannot preserve the original layout'; summary; }
slots() { ctl setBladeSlots "$1" "base64:$(printf %s "$2" | base64 -w0)"; }
cleanup() {
  local edge
  for edge in right left; do
    slots "$edge" "$(jq -c --arg edge "$edge" '.[$edge].slots' <<< "$original")"
    if [[ $(jq -r --arg edge "$edge" '.[$edge].open' <<< "$original") == true ]]; then ctl openBlade "$edge"; else ctl closeBlade "$edge"; fi
  done
}
trap cleanup EXIT

section='[{"id":"reload62","modules":[{"module":"files"},{"module":"memory"},{"module":"branches"}]},{"id":"properties","modules":[{"module":"properties"}]}]'
first() { document | jq -c '.blades.left.slots[0] | {active, modules: [.modules[].module]}'; }
stored() { guest "jq -c '.blades.left.slots[0] | {active, modules: [.modules[].module]}' $(printf '%q' "$(field bladeLayoutPath)")"; }
reloaded() {
  sleep 1
  wait_for "document | jq -e '.blades.left.slots[0].modules | length == 3' >/dev/null" 30 || return 1
  sleep 1
}
same() { [[ $3 == "$4" ]] && pass "$1" "$2" || fail "$1" "$2" "got [$3] wanted [$4]"; }

slots right '[]'
slots left "$section"
ctl openBlade left
sleep 2

for tab in 2 1; do
  ctl setBladeTab left 0 "$tab"
  sleep 2
  guest "omarchy-shell shell rescanPlugins" >/dev/null
  reloaded || fail harness reload "FileBlade did not come back after the rescan"
  same E-62-01 "an open blade comes back on tab $tab after a rescan" "$(first | jq -r .active)" "$tab"
  same E-62-01 "and blades.json still says tab $tab" "$(stored | jq -r .active)" "$tab"
done

guest "omarchy-shell -q $PLUGIN.control setBladeTab left 0 2; omarchy-shell shell rescanPlugins" >/dev/null
reloaded || fail harness reload "FileBlade did not come back after the rescan"
same E-62-02 "a tab chosen just before the rescan is kept" "$(first | jq -r .active)" 2

moved='[{"id":"reload62","modules":[{"module":"branches"},{"module":"files"},{"module":"memory"}],"active":2},{"id":"properties","modules":[{"module":"properties"}]}]'
guest "omarchy-shell -q $PLUGIN.control setBladeSlots left base64:$(printf %s "$moved" | base64 -w0); omarchy-shell shell rescanPlugins" >/dev/null
reloaded || fail harness reload "FileBlade did not come back after the rescan"
same E-62-02 "tabs rearranged just before the rescan keep their order" "$(first | jq -c .modules)" '["branches","files","memory"]'
same E-62-02 "and their current tab" "$(first | jq -r .active)" 2
same E-62-02 "and blades.json holds the same layout" "$(stored)" '{"active":2,"modules":["branches","files","memory"]}'
"$OVM" shot 62-plugin-reload-layout >/dev/null

summary
