#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"

require_guest
layout_path=$(field bladeLayoutPath)
original=$("$OVM" ipc "$PLUGIN" blades | jq -ce '.blades')
[[ $(jq -r '(.left.slots | type) == "array" and (.right.slots | type) == "array"' <<< "$original") == true ]] || { fail harness layout 'cannot preserve the original layout'; summary; }
original_focus=$(field focusedBlade)
cleanup() {
  local edge
  for edge in right left; do
    ctl setBladeSlots "$edge" "base64:$(jq -c --arg edge "$edge" '.[$edge].slots' <<< "$original" | base64 -w0)"
    if [[ $(jq -r --arg edge "$edge" '.[$edge].open' <<< "$original") == true ]]; then ctl openBlade "$edge"; else ctl closeBlade "$edge"; fi
  done
  if [[ -n $original_focus ]]; then ctl focusBlade "$original_focus"; else ctl releaseBladeFocus; fi
}
trap cleanup EXIT

blade_left() {
  if [[ $1 == right ]]; then printf '%s\n' $((1920 - $(field propertiesBladeWidth))); else printf '0\n'; fi
}
blade_width() {
  if [[ $1 == right ]]; then field propertiesBladeWidth; else field sidebarWidth; fi
}
open_picker() {
  local point limit
  point=$(add_module_point "$1") || { fail E-22-01 "find the $1 module +" "no + in the $1 header"; return 1; }
  read -r plus_x plus_y <<< "$point"
  limit=$(($(blade_left "$1") + $(blade_width "$1") - 230))
  menu_x=$((plus_x - 15 < limit ? plus_x - 15 : limit))
  "$OVM" mouse click "$plus_x" "$plus_y"
  sleep 1
}
picker_text() {
  local shot crop
  shot=$("$OVM" shot module-picker-ocr | tail -1)
  crop=$(mktemp --suffix=.png)
  magick "$shot" -crop "160x24+$((menu_x + 7))+$((plus_y + 21))" +repage -colorspace gray -negate -resize 400% "$crop"
  tesseract "$crop" - --psm 7 2>/dev/null | tr -s '[:space:]' ' '
  rm -f -- "$crop" "$shot"
}

ctl setBladeSlots right "base64:$(printf '%s' '[{"id":"picker-notes","modules":[{"module":"notes","state":{"text":"picker note preserved"}}]}]' | base64 -w0)"
ctl openBlade right
ctl focusBlade right
"$OVM" ipc notifications dismissAll >/dev/null
sleep 1
open_picker right
"$OVM" mouse move $((menu_x + 96)) $((plus_y + 191))
sleep 1
expect_contains E-22-01 "a single Notes section opens its module picker" "$(picker_text)" "Add module"
"$OVM" key esc
sleep 1
expect_missing E-22-01 "Escape dismisses the picker after hovering its rows" "$(picker_text)" "Add module"
expect_true E-22-01 "Escape leaves the right blade open" '[[ $("$OVM" ipc "$PLUGIN" blades | jq -r .blades.right.open) == true ]]'

open_picker right
"$OVM" shot module-picker-right >/dev/null
"$OVM" mouse click $((menu_x + 205)) $((plus_y + 35))
sleep 1
expect_missing E-22-01 "the close X dismisses the module picker" "$(picker_text)" "Add module"

open_picker right
"$OVM" type skills
"$OVM" key ret
sleep 2
expect_true E-22-01 "keyboard selection adds Skills to the right Notes section" '[[ $("$OVM" ipc "$PLUGIN" blades | jq -r ".blades.right.slots[0].modules[1].module") == skills ]]'
expect_out E-22-01 "adding a module preserves Notes" "jq -r '.blades.right.slots[0].modules[0].state.text | if type == \"string\" then . else .items[0].text end' $(printf '%q' "$layout_path")" "picker note preserved"
"$OVM" shot module-picker-added-right >/dev/null

ctl openBlade left
ctl focusBlade left
sleep 1
open_picker left
"$OVM" mouse move $((menu_x + 96)) $((plus_y + 191))
sleep 1
expect_contains E-22-01 "the left blade's current header opens the picker" "$(picker_text)" "Add module"
"$OVM" key esc
sleep 1
expect_missing E-22-01 "Escape also dismisses the left picker" "$(picker_text)" "Add module"

ctl setBladeSlots left "base64:$(printf '%s' '[{"id":"picker-tabs","modules":[{"module":"files"},{"module":"branches"}]},{"module":"properties"}]' | base64 -w0)"
sleep 2
open_picker left
"$OVM" shot module-picker-left-tabs >/dev/null
expect_true E-22-01 "the left tab row keeps its + after the last tab" '((plus_x > 120))'
expect_contains E-22-01 "the + after the left tabs opens the picker" "$(picker_text)" "Add module"
"$OVM" type memory
"$OVM" key ret
sleep 2
expect_true E-22-01 "picking from the left tab row adds the tab to that section" '[[ $("$OVM" ipc "$PLUGIN" blades | jq -c "[.blades.left.slots[0].modules[].module]") == "[\"files\",\"branches\",\"memory\"]" ]]'

summary
