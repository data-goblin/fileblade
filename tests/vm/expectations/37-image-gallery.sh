#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"

require_guest
GALLERY_STATE=${GALLERY_STATE:-$(guest 'mktemp -d "$XDG_RUNTIME_DIR/fileblade-gallery.XXXXXX"')}
GALLERY_KEEP=${GALLERY_KEEP:-0}
PROBE=fileblade.gallery-test
sequence=0
gallery_log_dir=${GALLERY_LOG_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/fileblade-gallery-log.XXXXXX")}

gallery() { "$OVM" ipc "$PROBE.${1:-blade}" status 2>/dev/null; }
gallery_restart() {
  "$(dirname "$0")/../stop-shell" || return 1
  guest 'if test -d ~/.cache/quickshell/qmlcache; then mv ~/.cache/quickshell/qmlcache "$(mktemp -d ~/.cache/quickshell/gallery-cache.XXXXXX)/qmlcache"; fi'
  "$OVM" restart-shell >/dev/null
  wait_for "gallery | jq -e '.ready and .count > 0'" 30
  ctl focusBlade left
  sleep 1
}
cleanup_gallery() {
  [[ $GALLERY_KEEP == 1 ]] && return
  "$(dirname "$0")/../stop-shell" >/dev/null 2>&1
  guest "python3 $GUEST_PLUGIN/tests/vm/image-gallery-fixture.py restore $(printf '%q' "$GALLERY_STATE")"
  guest 'if test -d ~/.cache/quickshell/qmlcache; then mv ~/.cache/quickshell/qmlcache "$(mktemp -d ~/.cache/quickshell/gallery-cache.XXXXXX)/qmlcache"; fi'
  "$OVM" restart-shell >/dev/null 2>&1
  [[ -n ${GALLERY_LOG_DIR:-} ]] || rm -rf -- "$gallery_log_dir"
}
trap cleanup_gallery EXIT
if [[ ${GALLERY_PREPARED:-0} != 1 ]]; then
  "$(dirname "$0")/../stop-shell" || exit 1
  guest "python3 $GUEST_PLUGIN/tests/vm/image-gallery-fixture.py prepare $(printf '%q' "$GALLERY_STATE")" || exit 1
fi
gallery_restart || { fail harness 'gallery fixture loads' 'no ready gallery'; summary; }

observe() {
  local id=$1 action=$2 expected=$3 expression=$4 target=${5:-blade} got shot
  skip "$id" && return
  sequence=$((sequence + 1))
  got=$(gallery "$target")
  shot=$("$OVM" shot "flint-${id}-${sequence}" | tail -1)
  printf 'SCENARIO %s action=%s expected=%s observed=%s shot=%s\n' "$id" "$action" "$expected" "$got" "$shot"
  if jq -e "$expression" <<<"$got" >/dev/null 2>&1; then pass "$id" "$expected"; else fail "$id" "$expected" "$got"; fi
}
click_tile() {
  local p
  p=$(gallery | jq -r '.tiles[0].point | "\(.x) \(.y)"')
  read -r tile_x tile_y <<<"$p"
  "$OVM" mouse click "$tile_x" "$tile_y"
  sleep 1
}
drag_gallery() {
  local start_x=$1 start_y=$2 end_x=$3 end_y=$4 name=$5
  guest "sed 's/START_X/$start_x/g; s/START_Y/$start_y/g; s/END_X/$end_x/g; s/END_Y/$end_y/g' $GUEST_PLUGIN/tests/vm/image-gallery-drag.toml > $(printf '%q' "$GALLERY_STATE")/drag.toml"
  guest "democtl record $(printf '%q' "$GALLERY_STATE")/drag.toml --out $(printf '%q' "$GALLERY_STATE")/$name" > "$gallery_log_dir/fileblade-gallery-$name.log" 2>&1 &
  drag_pid=$!
}
key_gallery() { "$OVM" key "$1"; sleep 0.8; }

observe E-37-01 'open gallery' 'pane icon loads as a tinted image' '.icons | any(.ready and (.url | endswith("goblin.svg")))'
observe E-37-01 'open bar widget' 'bar icon is a loaded image' '.ready and (.icon | endswith("goblin.svg"))' bar
ctl toggleBladeSettings left
sleep 1
observe E-37-01 'open blade settings' 'settings row shows the pictorial module icon' '.surfaceIcons | any((.url | endswith("goblin.svg")) and .point.y > 100)'
"$OVM" shot flint-E-37-01-settings >/dev/null
ctl toggleBladeSettings left
sleep 1
if plus=$(add_module_point left); then
  "$OVM" mouse click ${plus}
  sleep 1
else
  fail E-37-01 'find the left module +' 'no + in the left header'
fi
observe E-37-01 'open module picker' 'picker includes image icons and glyph fallbacks' '(.surfaceIcons | any(.url | endswith("goblin.svg"))) and (.surfaceIcons | any(.url == "" and .glyph != ""))'
"$OVM" shot flint-E-37-01-picker >/dev/null
key_gallery esc
observe E-37-02 'load dated fixture' 'months descend and undated comes last; visible thumbnails load' '[.sections[].key] == ["2026-09","2025-06","2024-01",""] and (.tiles | any(.source != "" and .error == ""))'
click_tile
selected=$(gallery | jq -r .current.path)
expect E-37-02 'tile click selects its exact path including # and %' selectedPath "$selected"
observe E-37-02 'select picture' 'Properties loads an image preview' '.preview != ""' 
key_gallery m
expect E-37-02 'm opens file actions' actionMenuOpen true
expect E-37-02 'file actions name the selected picture' actionMenuPath "$selected"
"$OVM" shot flint-E-37-02-menu >/dev/null
key_gallery esc
"$OVM" mouse rclick "$tile_x" "$tile_y"
sleep 1
expect E-37-02 'right click opens the same menu' actionMenuPath "$selected"
expect E-37-02 'right click menu is open' actionMenuOpen true
key_gallery esc
click_tile
key_gallery ret
wait_for "[[ \$(field lastLaunchedPath) == $(printf '%q' "$selected") ]]" 15
expect E-37-02 'Enter opens the selected image' lastLaunchedPath "$selected"
"$OVM" shot flint-E-37-02-viewer >/dev/null
ctl focusBlade left
read -r second_x second_y second_path < <(gallery | jq -r '.tiles[1] | "\(.point.x) \(.point.y) \(.path)"')
double_click "$second_x" "$second_y"
wait_for "[[ \$(field lastLaunchedPath) == $(printf '%q' "$second_path") ]]" 15
expect E-37-02 'double click opens another image' lastLaunchedPath "$second_path"
guest "pkill -f -- $(printf '%q' "[/]${GALLERY_STATE#/}/library/")" >/dev/null
ctl focusBlade left
click_tile
for key in right down left up l j h k; do
  before=$(gallery | jq -r .current.path)
  key_gallery "$key"
  after=$(gallery | jq -r .current.path)
  [[ $before != "$after" ]] && pass E-37-02 "$key moves selection" || fail E-37-02 "$key moves selection" "$after"
done
key_gallery home
for key in minus minus minus minus minus; do key_gallery "$key"; done
observe E-37-03 'press minus to the minimum' 'smallest step and toolbar agree' '.size == 0 and .stepper.step == 0 and .stepper.steps == 5'
for step in 1 2 3 4; do
  key_gallery equal
  observe E-37-03 'press equal' "preview step $step and toolbar agree" ".size == $step and .stepper.step == $step"
done
key_gallery equal
observe E-37-03 'press equal at maximum' 'size clamps at fifth step' '.size == 4'
gallery_restart
observe E-37-03 'restart the shell' 'blade preview size survives' '.size == 4 and .stepper.step == 4'
click_tile
key_gallery slash
"$OVM" type 'character:bink'
sleep 1
observe E-37-04 'type character:bink' 'live search reports 12 of 24' '.count == 12 and .status == "12 of 24"'
key_gallery esc
observe E-37-04 'first Escape' 'query clears and blade remains visible' '.query == "" and .count == 24 and .active'
key_gallery slash
"$OVM" type '-tag:danger'
sleep 1
observe E-37-04 'type -tag:danger' 'negative filter reports 16 of 24' '.count == 16 and .status == "16 of 24"'
key_gallery esc
key_gallery esc
expect E-37-04 'next Escape closes blade' open false
ctl openBlade left
ctl focusBlade left
sleep 1
click_tile
key_gallery home
rail_x=$(gallery | jq -r .rail.x)
rail_y=$(gallery | jq -r '.rail.y + (.rail.height * 0.5) | floor')
"$OVM" mouse move "$rail_x" "$rail_y"
sleep 1
observe E-37-05 'hover timeline midpoint' 'month pill appears, three year labels and three dated dots exist' '.rail.engaged and .rail.pill != "" and (.rail.years | length) == 3 and ([.rail.months[] | select(.undated | not)] | length) == 3'
"$OVM" mouse click "$rail_x" "$rail_y"
sleep 1
observe E-37-05 'click timeline midpoint' 'grid scrolls and thumb advances' '.scroll > 0 and .rail.fraction > 0'

rail_end=$(gallery | jq -r '.rail.y + .rail.height - 10 | floor')
drag_gallery "$rail_x" "$rail_y" "$rail_x" "$rail_end" timeline
sleep 3
observe E-37-05 'drag timeline' 'month pill remains engaged while scrolling' '.rail.engaged and .rail.pill != "" and .scroll > 0'
wait "$drag_pid" || fail harness "virtual pointer recorder" "see fileblade-gallery recorder log"
observe E-37-05 'release timeline at bottom' 'thumb reaches the bottom of its track' '(.rail.thumbY + .rail.thumbLength - .rail.trackHeight - 8 | fabs) < 12'
ctl focusBlade left
key_gallery home
click_tile
drag_gallery "$tile_x" "$tile_y" 900 550 wheel
sleep 3
"$OVM" hold spc
sleep 0.6
expect E-37-02 'virtual pointer image drag reaches the wheel controller' dropWheel.dragging true
"$OVM" shot flint-E-37-02-drag >/dev/null
"$OVM" key esc
"$OVM" release spc
wait "$drag_pid" || fail harness "virtual pointer recorder" "see fileblade-gallery recorder log"
expect E-37-02 'cancel drag clears the wheel' dropWheel.dragging false
read -r bar_x bar_y < <(gallery bar | jq -r '.point | "\(.x) \(.y)"')
"$OVM" mouse click "$bar_x" "$bar_y"
sleep 1
if ! gallery popout | jq -e '.ready' >/dev/null 2>&1; then
  guest 'omarchy-shell shell toggle kurt.goblin-images' >/dev/null
  sleep 1
fi
blade_before=$(gallery | jq -c '{size,query,count}')
observe E-37-06 'open bar dropdown' 'whole module loads with header, grid, search, stepper and timeline' '.ready and .count == 24 and .stepper.steps == 5 and (.icons | any(.ready)) and (.rail.years | length) == 3' popout
key_gallery esc
if gallery popout | jq -e '.ready' >/dev/null 2>&1; then fail E-37-06 'Escape closes popout' 'still registered'; else pass E-37-06 'Escape closes popout'; fi
[[ $(gallery | jq -c '{size,query,count}') == "$blade_before" ]] && pass E-37-06 'blade copy remains unchanged' || fail E-37-06 'blade copy remains unchanged' "$(gallery)"
guest 'omarchy-shell shell toggle kurt.goblin-images' >/dev/null
sleep 1
"$OVM" mouse click 1000 800
sleep 1
if gallery popout | jq -e '.ready' >/dev/null 2>&1; then fail E-37-06 'outside click closes popout' 'still registered'; else pass E-37-06 'outside click closes popout'; fi
guest 'omarchy-shell shell toggle kurt.goblin-images' >/dev/null
sleep 1
read -r popup_x popup_y popup_path < <(gallery popout | jq -r '.tiles[0] | "\(.point.x) \(.point.y) \(.path)"')
"$OVM" mouse rclick "$popup_x" "$popup_y"
sleep 1
expect E-37-06 'popout right click hands focus to file actions' actionMenuOpen true
expect E-37-06 'popout actions name the clicked image' actionMenuPath "$popup_path"
expect_contains E-37-06 'popout file actions are actually drawn' "$(screen_text)" 'Open' 
"$OVM" shot flint-E-37-06-menu >/dev/null
key_gallery esc
summary
