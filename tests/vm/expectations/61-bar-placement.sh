#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
require_guest

shape=${FILEBLADE_SHAPE:-plugin}
shots=${SHOT_PREFIX:-61}
if [[ $shape == native ]]; then
  launcher=/home/omarchy/.local/bin/fileblade
else
  launcher=/home/omarchy/.config/omarchy/plugins/$PLUGIN/fileblade
fi
original_config=$(guest 'cat ~/.config/omarchy/shell.json')
jq -e . <<< "$original_config" >/dev/null || { fail harness shell-config 'cannot read shell.json'; summary; }
was_hidden=$(guest 'test -f ~/.local/state/omarchy/toggles/bar-off && echo yes || echo no')
original_placement=$(field barPlacement)
original_blades=$("$OVM" ipc "$PLUGIN" blades | jq -c '.blades | {left: .left.open, right: .right.open}')

write_position() {
  local data
  data=$(jq -c --arg position "$1" '.bar.position = $position' <<< "$original_config" | base64 -w0)
  guest "set -e; f=\$(mktemp ~/.config/omarchy/shell.json.XXXXXX); printf %s '$data' | base64 -d > \"\$f\"; chmod --reference ~/.config/omarchy/shell.json \"\$f\"; mv -f -- \"\$f\" ~/.config/omarchy/shell.json; omarchy-shell shell reloadConfig >/dev/null"
}
bar_hidden() { guest "omarchy-toggle-bar $([[ $1 == yes ]] && echo on || echo off)" >/dev/null; }
cleanup() {
  local data
  data=$(base64 -w0 <<< "$original_config")
  guest "set -e; f=\$(mktemp ~/.config/omarchy/shell.json.XXXXXX); printf %s '$data' | base64 -d > \"\$f\"; chmod --reference ~/.config/omarchy/shell.json \"\$f\"; mv -f -- \"\$f\" ~/.config/omarchy/shell.json; omarchy-shell shell reloadConfig >/dev/null"
  bar_hidden "$was_hidden"
  ctl setBarPlacement "${original_placement:-below}"
  for edge in left right; do
    if [[ $(jq -r --arg e "$edge" '.[$e]' <<< "$original_blades") == true ]]; then ctl openBlade "$edge"; else ctl closeBlade "$edge"; fi
  done
}
trap cleanup EXIT

layers() {
  "$OVM" hypr layers | jq -c '[.[] | .levels | to_entries[] | .key as $level | .value[]
    | select(.namespace | test("^omarchy-(bar|fileblade-(left|right)(-reserve)?)$")) | {key: .namespace, value: {x, y, w, h, level: ($level | tonumber)}}] | from_entries'
}
geometry() {
  local hidden=$2
  "$OVM" hypr monitors | jq -c --argjson l "$(layers)" --arg placement "$1" --arg hidden "$hidden" '.[0] as $m |
    ($l["omarchy-bar"]) as $bar | ($l["omarchy-fileblade-left"]) as $left | ($l["omarchy-fileblade-right"]) as $right |
    { screen: [$m.width, $m.height], reserved: $m.reserved, bar: $bar, left: $left, right: $right,
      leftReserve: $l["omarchy-fileblade-left-reserve"], rightReserve: $l["omarchy-fileblade-right-reserve"],
      placement: $placement, hidden: ($hidden == "yes") }'
}
fits() {
  jq -e '
    .screen as [$sw, $sh] | .bar as $b | .left as $l | .right as $r |
    ($b.x == 0 and $b.w == $sw) as $top_or_bottom_full |
    (if .hidden then {top: 0, bottom: 0, left: 0, right: 0}
     elif $b.w == $sw and $b.y == 0 then {top: $b.h, bottom: 0, left: 0, right: 0}
     elif $b.w == $sw then {top: 0, bottom: $b.h, left: 0, right: 0}
     elif $b.x == 0 then {top: 0, bottom: 0, left: $b.w, right: 0}
     else {top: 0, bottom: 0, left: 0, right: $b.w} end) as $bar |
    ($l.x == $bar.left and $l.y == $bar.top and $l.h == $sh - $bar.top - $bar.bottom) and
    ($r.x + $r.w == $sw - $bar.right and $r.y == $bar.top and $r.h == $sh - $bar.top - $bar.bottom) and
    (.leftReserve.level == 3 and .rightReserve.level == 3) and
    (.reserved[1] == $bar.top and .reserved[3] == $bar.bottom)' >/dev/null
}
beside() {
  jq -e '
    .screen as [$sw, $sh] | .bar as $b | .left as $l | .right as $r |
    $l.x == 0 and $l.y == 0 and $l.h == $sh and $r.y == 0 and $r.h == $sh and $r.x + $r.w == $sw and
    .leftReserve.level == 1 and .rightReserve.level == 1 and
    (.hidden or ($b.x == .reserved[0] and $b.x + $b.w == $sw - .reserved[2]))' >/dev/null
}
check() {
  local id=$1 label=$2 predicate=$3 placement=$4 hidden=$5 value
  if wait_for "geometry $placement $hidden | $predicate" 12; then
    pass "$id" "$label"
  else
    value=$(geometry "$placement" "$hidden")
    fail "$id" "$label" "$value"
  fi
}
shot() { "$OVM" shot "$1" >/dev/null 2>&1; }

ctl openBlade left
ctl openBlade right
ctl setBarPlacement below
bar_hidden no
write_position top
wait_for "[[ \$(blade_layer left) == 1 && \$(blade_layer right) == 1 ]]" 12

check E-61-01 'both blades start below a top bar and the bar keeps the full width' fits below no
[[ $(field barPlacement) == below ]] && pass E-61-01 'the default placement is below' || fail E-61-01 'the default placement is below' "$(field barPlacement)"
shot "$shots-below-top"

if [[ $shape == native ]]; then
  guest 'omarchy-restart-shell >/dev/null 2>&1' >/dev/null
  sleep 6
  check E-61-02 'the blades stay below a bar that was recreated after them' fits below no
else
  pending E-61-02 'bar recreated after the blades' 'needs the native shape, where the shell restarts under a running FileBlade'
fi

for position in bottom left right; do
  write_position "$position"
  check E-61-03 "blades yield to a $position bar on the shared edge" fits below no
  shot "$shots-below-$position"
done
write_position top

bar_hidden yes
check E-61-04 'hiding the bar gives the blades the full height' fits below yes
shot "$shots-below-hidden"
bar_hidden no
check E-61-04 'showing the bar again puts the blades back below it' fits below no

ctl setBarPlacement beside
check E-61-05 'beside keeps the full-height blades and shortens the bar between them' beside beside no
layout_path=$(field bladeLayoutPath)
saved() { guest "jq -r .barPlacement '$layout_path'"; }
wait_for '[[ $(saved) == beside ]]' 8
saved=$(saved)
[[ $saved == beside ]] && pass E-61-05 'beside is saved in blades.json' || fail E-61-05 'beside is saved in blades.json' "$saved"
shot "$shots-beside-top"
bar_hidden yes
check E-61-05 'beside with the bar hidden keeps the full height' beside beside yes
bar_hidden no
if restart_shell; then
  ctl openBlade left
  ctl openBlade right
  [[ $(field barPlacement) == beside ]] && pass E-61-05 'beside survives a restart' || fail E-61-05 'beside survives a restart' "$(field barPlacement)"
  check E-61-05 'after the restart the blades are still full height beside the bar' beside beside no
else
  fail E-61-05 'beside survives a restart' 'FileBlade did not come back'
fi

answer=$(guest "'$launcher' blade bar-placement below")
check E-61-06 'fileblade blade bar-placement below moves the blades back below the bar' fits below no
[[ $answer == *below* ]] && pass E-61-06 'the command reports the new placement' || fail E-61-06 'the command reports the new placement' "$answer"

ctl toggleBladeSettings left
sleep 2
shot "$shots-settings"
if screen_text | grep -q 'Navbar'; then pass E-61-07 'Settings lists the Navbar placement'; else fail E-61-07 'Settings lists the Navbar placement' 'Navbar not on screen'; fi
ctl toggleBladeSettings left

summary
