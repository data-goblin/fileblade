#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"

require_guest

shot() { "$OVM" shot "$1"; }
wheel() { status | jq -r ".dropWheel.$1"; }
wheel_labels() { status | jq -r '[.dropWheel.actions[]?.label]|join(",")'; }
wheel_index() { status | jq -r --arg n "$1" '[.dropWheel.actions[]?.label]|index($n)'; }
clients() { "$OVM" hypr clients 2>/dev/null | jq length; }
kill_windows() { "$OVM" ssh 'pkill -x foot; pkill -x nvim' >/dev/null 2>&1; wait_for "[[ \$(clients) == 0 ]]" 15; }

space_helper=""
space_pid=""
mux_session="fileblade-e26-$$"
cleanup_space_helper() {
  "$OVM" release ctrl >/dev/null 2>&1
  "$OVM" release spc >/dev/null 2>&1
  "$OVM" mouse up >/dev/null 2>&1
  [[ ! $space_pid =~ ^[0-9]+$ ]] || guest "kill -- -$space_pid" >/dev/null 2>&1
  [[ -z $space_helper ]] || guest "rm -f -- $(printf '%q' "$space_helper")" >/dev/null 2>&1
  guest "tmux kill-session -t $mux_session 2>/dev/null || true" >/dev/null 2>&1
}
trap cleanup_space_helper EXIT
trap 'exit 130' INT TERM
space_source="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd -P)/demos/hold-space-near.sh"
if [[ ! -f $space_source ]]; then
  fail harness "stage the modifier helper" "source helper is absent: $space_source"
  summary
fi
space_helper=$(guest 'mktemp /tmp/fileblade-drop-wheel-space.XXXXXX')
if [[ ! $space_helper =~ ^/tmp/fileblade-drop-wheel-space\.[[:alnum:]]+$ ]]; then
  fail harness "stage the modifier helper" "guest temporary path is invalid"
  summary
fi
space_data=$(base64 -w0 "$space_source")
if ! guest "printf '%s' $space_data | base64 -d > $(printf '%q' "$space_helper") && chmod 700 $(printf '%q' "$space_helper")"; then
  fail harness "stage the modifier helper" "could not copy the helper into the guest"
  summary
fi
wedge_point() { awk -v x="$1" -v y="$2" -v n="$3" -v i="$4" 'BEGIN { a = (-90 + i * 360 / n) * 3.14159265 / 180; printf "%d %d\n", x + 58 * cos(a), y + 58 * sin(a) }'; }

choose_child() {
  local id=$1 label=$2 expectation=$3 snapshot parent children child count wx wy px py
  snapshot=$(status | jq -c .dropWheel)
  parent=$(jq --arg id "$id" '[.actions[].id] | index($id)' <<<"$snapshot")
  [[ $parent =~ ^[0-9]+$ ]] || { fail "$expectation" "find the parent action" "$snapshot"; return 1; }
  children=$(jq -c ".actions[$parent].placements" <<<"$snapshot")
  child=$(jq --arg label "$label" '[.[].label] | index($label)' <<<"$children")
  [[ $child =~ ^[0-9]+$ ]] || { fail "$expectation" "find $label in the second ring" "$children"; return 1; }
  count=$(jq '.actions | length' <<<"$snapshot")
  wx=$(jq .x <<<"$snapshot"); wy=$(jq .y <<<"$snapshot")
  read -r px py <<<"$(wedge_point "$wx" "$wy" "$count" "$parent")"
  "$OVM" mouse move "$px" "$py"
  wait_for "[[ \$(wheel highlighted) == $parent ]]" 5
  read -r px py < <(awk -v x="$wx" -v y="$wy" -v n="$count" -v p="$parent" -v c="$(jq length <<<"$children")" -v i="$child" 'BEGIN {
    pi = 3.14159265
    r = 58 + 5 * (n > 4 ? n : 4)
    if (r < 86) r = 86
    step = 2 * pi / c
    if (step > pi / 4) step = pi / 4
    a = (-90 + p * 360 / n) * pi / 180 + (i - (c - 1) / 2) * step
    printf "%d %d\n", x + (r + 30) * cos(a), y + (r + 30) * sin(a)
  }')
  "$OVM" mouse move "$px" "$py"
  wait_for "[[ \$(wheel outerHighlighted) == $child ]]" 5
  expect_true "$expectation" "the pointer selects $label in the second ring" "[[ \$(wheel outerHighlighted) == $child ]]"
  shot "$expectation-second-ring"
  "$OVM" mouse click "$px" "$py"
  wait_for "[[ \$(wheel open) == false ]]" 10
  expect_true "$expectation" "choosing $label closes the wheel" "[[ \$(wheel open) == false ]]"
}

drag_with_space() {
  local sx=$1 sy=$2 tx=$3 ty=$4 ease=$5 reach=$6 hold=$7 drag_shot=${8:-} pending_shot=${9:-} open_shot=${10:-} loaded_shot=${11:-} script
  script=$(cat <<TOML
[demo]
name = "expect-wheel"
output = "Virtual-1"
capture = "screencopy"
screen = [1920, 1080]
region = { x = 0, y = 0, w = 1920, h = 1080 }
portal = false
leading_blank_ms = 200
show_keys = false
fps = 30
settle_ms = 300
gif_fps = 10
gif_width = 900

[cursor]
size = 28
fill = "#c0caf5"
outline = "#0e0e14"

[caption]
size = 30
fill = "#c0caf5"
background = "#13141c"
font = "JetBrains Mono Nerd Font"

[points]
source = [$sx, $sy]
target = [$tx, $ty]

[[step]]
kind = "move"
to = "source"
ms = 300

[[step]]
kind = "hold"
ms = 400

[[step]]
kind = "drag"
hold_ms = 300
to = "target"
ease = "$ease"
ms = 3000

[[step]]
kind = "hold"
ms = 1500
TOML
)
  guest "printf '%s\n' $(printf '%q' "$script") > /tmp/fb-wheel.toml"
  space_pid=$(guest "setsid $(printf '%q' "$space_helper") $tx $ty $reach $hold >/tmp/fb-wheel-space.log 2>&1 </dev/null & echo \$!")
  guest 'setsid democtl record /tmp/fb-wheel.toml --out /tmp --force >/tmp/fb-wheel-record.log 2>&1 </dev/null &' >/dev/null
  mid_drag=""; loaded=""; local drag_seen=0
  local deadline=$((SECONDS + 12)) snapshot
  local blade_width; blade_width=$(field sidebarWidth)
  while ((SECONDS < deadline)); do
    snapshot=$(status | jq -c '.dropWheel' 2>/dev/null)
    if [[ $drag_seen == 0 && -n $drag_shot && $(jq -r '.dragging and (.open | not)' <<<"$snapshot" 2>/dev/null) == true ]] && (( $("$OVM" hypr cursorpos | jq -r .x) > blade_width + 40 )); then
      shot "$drag_shot"
      [[ -z $pending_shot ]] || shot "$pending_shot"
      drag_seen=1
    fi
    if [[ $(jq -r '.open' <<<"$snapshot" 2>/dev/null) == true ]]; then
      if [[ -z $mid_drag ]]; then
        mid_drag=$snapshot
        [[ -z $open_shot ]] || shot "$open_shot"
      fi
      if [[ $(jq -r '.loading | not' <<<"$snapshot" 2>/dev/null) == true ]]; then
        loaded=$snapshot
        [[ -z $loaded_shot ]] || shot "$loaded_shot"
        break
      fi
    fi
    sleep 0.2
  done
  wait_for "! guest 'pgrep -x democtl' | grep -q ." 30
  sleep 1.5
}
mid() { jq -r ".$1" <<<"${mid_drag:-null}" 2>/dev/null; }
seen() { jq -r ".$1" <<<"${loaded:-null}" 2>/dev/null; }

if [[ $FILEBLADE_SHAPE == native ]]; then
  ctl setBladeSlots left "base64:$(printf '%s' '[{"id":"e26-files","modules":[{"module":"files","state":{"mediaMode":false}}]},{"id":"e26-properties","modules":[{"module":"properties"}],"fraction":0.34}]' | base64 -w0)"
  sleep 2
fi
fixture >/dev/null
open_left
goto_root "$ROOT_DIR"
kill_windows
focus_tree
ctl hideDropWheel >/dev/null 2>&1

"$OVM" mouse click "$ROW_X" "$(row_y "$(row_index alpha.txt)")"
"$OVM" hold ctrl
"$OVM" mouse click "$ROW_X" "$(row_y "$(row_index long.txt)")"
"$OVM" release ctrl
expect_true E-26-02 "two rows are selected before dragging the second" "[[ \$(field selectedCount) == 2 && \$(field selectedPath) == $ROOT_DIR/long.txt ]]"
[[ $(field selectedCount) == 2 && $(field selectedPath) == "$ROOT_DIR/long.txt" ]] || summary
"$OVM" mouse down
"$OVM" mouse move 180 "$(row_y "$(row_index long.txt)")"
"$OVM" mouse move 900 500
wait_for "[[ \$(wheel dragging) == true && \$(wheel count) == 2 ]]" 5
expect_true E-26-02 "the real drag carries both selected rows" "[[ \$(wheel dragging) == true && \$(wheel count) == 2 && \$(wheel open) == false ]]"
[[ $(wheel dragging) == true && $(wheel count) == 2 ]] || summary
ghost_name=$(ocr_crop E-26-02-multi-item-ghost 95x30+935+512 400% 7)
ghost_count=$(ocr_crop E-26-02-multi-item-count 18x18+1036+518 800% 10 | tr -cd '0-9')
expect_contains E-26-02 "the ghost names the last grabbed row" "$ghost_name" long.txt
expect_true E-26-02 "the rendered ghost badge counts both items" "[[ $ghost_count == 2 ]]"
"$OVM" key esc
wait_for "[[ \$(wheel dragging) == false && \$(wheel open) == false ]]" 5
expect_true E-26-08 "Escape cancels the held drag without dismissing the blade" "[[ \$(wheel dragging) == false && \$(wheel count) == 0 && \$(field open) == true ]]"
shot E-26-08-escape-before-wheel-mouse-held
"$OVM" mouse move "$ROW_X" "$(row_y "$(row_index dest)")"
"$OVM" mouse up
sleep 1
expect_true E-26-08 "mouse release after Escape keeps the drag canceled" "[[ \$(wheel dragging) == false && \$(wheel open) == false && \$(clients) == 0 ]]"
expect_out E-26-08 "release over a directory after Escape moves no carried file" "test -f $ROOT_DIR/alpha.txt && test -f $ROOT_DIR/long.txt && test ! -e $ROOT_DIR/dest/alpha.txt && test ! -e $ROOT_DIR/dest/long.txt && echo unchanged" unchanged
focus_tree

"$OVM" mouse click "$ROW_X" "$(row_y "$(row_index alpha.txt)")"
"$OVM" mouse down
"$OVM" mouse move 180 "$(row_y "$(row_index alpha.txt)")"
"$OVM" mouse move 900 500
"$OVM" hold spc
wait_for "[[ \$(wheel open) == true && \$(wheel loading) == false ]]" 10
expect_true E-26-08 "the wheel is open during the held drag before cancellation" "[[ \$(wheel open) == true && \$(wheel dragging) == true ]]"
[[ $(wheel open) == true && $(wheel dragging) == true ]] || summary
shot E-26-08-wheel-before-escape
"$OVM" key esc
"$OVM" release spc
wait_for "[[ \$(wheel dragging) == false && \$(wheel open) == false ]]" 5
shot E-26-08-escape-with-wheel-mouse-held
"$OVM" mouse up
sleep 1
expect_true E-26-08 "Escape cancels the open wheel drag and release opens nothing" "[[ \$(wheel dragging) == false && \$(wheel open) == false && \$(field open) == true && \$(clients) == 0 ]]"
focus_tree

drag_with_space "$ROW_X" "$(row_y "$(row_index alpha.txt)")" 900 500 ease_in_out 12 3000 E-26-01-drag-ghost "" E-26-03-wheel-open E-26-04-desktop-actions
expect_true E-26-01 "leaving the blade with a row starts a drag" "[[ \$(mid dragging) == true ]]"
expect_true E-26-01 "that carries one item" "[[ \$(mid count) == 1 ]]"
expect_true E-26-03 "holding the modifier opens the wheel during the drag" "[[ \$(mid open) == true && \$(mid fromDrag) == true ]]"
expect_true E-26-03 "at the pointer" "[[ \$(mid x) -gt 860 && \$(mid x) -lt 940 && \$(mid y) -gt 460 && \$(mid y) -lt 540 ]]"
labels=$(jq -r '[.actions[]?.label]|join(",")' <<<"${loaded:-null}" 2>/dev/null)
expect_true E-26-04 "over the bare desktop the target is the desktop" "[[ \$(seen target) == Desktop ]]"
expect_contains E-26-04 "which offers the default open" "$labels" "Open in new window"
expect_contains E-26-04 "and a new terminal" "$labels" "Open in new terminal"
expect_missing E-26-04 "but nothing that needs a window under the pointer" "$labels" "herdr"
expect_true E-26-09 "releasing on the hub keeps the wheel open" "[[ \$(wheel open) == true && \$(wheel dragging) == false ]]"
expect_true E-26-09 "as a wheel that no longer follows a drag" "[[ \$(wheel fromDrag) == false ]]"
shot E-26-09-hub-released

count=$(status | jq '.dropWheel.actions|length')
wx=$(wheel x); wy=$(wheel y)
open_with_index=$(wheel_index "Open with")
if [[ $count -gt 0 && $open_with_index != null ]]; then
  read -r px py <<<"$(wedge_point "$wx" "$wy" "$count" "$open_with_index")"
  "$OVM" mouse move "$px" "$py"; sleep 1.5
fi
terminal_index=$(wheel_index "Open in new terminal")
if [[ $count -gt 0 && $terminal_index != null ]]; then
  read -r px py <<<"$(wedge_point "$wx" "$wy" "$count" "$terminal_index")"
  "$OVM" mouse move "$px" "$py"; sleep 1.5
  expect_true E-26-05 "the pointer highlights the wedge under it" "[[ \$(wheel highlighted) == $terminal_index ]]"
  shot E-26-05-wedge-highlight
  "$OVM" mouse move "$wx" "$wy"; sleep 1.2
  expect_true E-26-05 "and the hub highlights nothing" "[[ \$(wheel highlighted) == -1 ]]"
  shot E-26-05-hub-highlight-clear
else
  fail E-26-05 "the pointer highlights the wedge under it" "no wheel stayed open to move around in"
fi
"$OVM" key esc; sleep 1.5
expect_true E-26-08 "escape closes the wheel" "[[ \$(wheel open) == false ]]"
expect_true E-26-08 "without opening anything" "[[ \$(clients) == 0 ]]"
expect_out E-26-08 "and the file is untouched" "test -f $ROOT_DIR/alpha.txt && echo yes || echo no" yes
shot E-26-08-cancelled

drag_with_space "$ROW_X" "$(row_y "$(row_index long.txt)")" 560 220 linear 60 3000 "" "" "" ""
wait_for "[[ \$(clients) -ge 1 ]]" 20
expect_true E-26-07 "releasing on Open in new window opens the file" "[[ \$(clients) -ge 1 ]]"
expect_true E-26-07 "and the wheel closes" "[[ \$(wheel open) == false ]]"
shot E-26-07-file-opened
kill_windows

ctl select "$ROOT_DIR/alpha.txt"
ctl showDropWheel 900 500
wait_for "[[ \$(wheel open) == true && \$(wheel loading) == false ]]" 10
expect_contains E-26-10 "Open with lists an installed application for this text file" "$(status | jq -r '.dropWheel.actions[] | select(.id == "open-with") | .placements[].label')" Neovim
if choose_child open-with Neovim E-26-10; then
  wait_for "guest 'pgrep -a -x nvim' | grep -Fq '$ROOT_DIR/alpha.txt'" 15
  expect_contains E-26-10 "the selected application opens the carried file" "$(guest 'pgrep -a -x nvim')" "$ROOT_DIR/alpha.txt"
fi
shot E-26-10-application-opened
kill_windows

guest "setsid foot -e tmux new-session -s $mux_session >/tmp/fileblade-e26-terminal.log 2>&1 </dev/null &"
wait_for "[[ \$(clients) == 1 ]]" 10
wait_for "[[ \$(guest 'tmux list-panes -t $mux_session' | wc -l) == 1 ]]" 10
terminal=$("$OVM" hypr clients | jq -c '.[] | select(.class == "foot")')
tx=$(jq '.at[0] + .size[0] / 2 | floor' <<<"$terminal")
ty=$(jq '.at[1] + .size[1] / 2 | floor' <<<"$terminal")
ctl select "$ROOT_DIR/alpha.txt"
ctl showDropWheel "$tx" "$ty"
wait_for "[[ \$(wheel open) == true && \$(wheel loading) == false ]]" 10
expect_contains E-26-06 "the terminal action offers explicit split destinations" "$(status | jq -c '.dropWheel.actions[] | select(.id == "mux-open") | [.placements[].label]')" '"Vertical split","Horizontal split"'
if choose_child mux-open "Vertical split" E-26-06; then
  wait_for "[[ \$(guest 'tmux list-panes -t $mux_session' | wc -l) == 2 ]]" 15
  expect_out E-26-06 "the chosen terminal gains exactly one pane" "tmux list-panes -t $mux_session | wc -l" 2
  expect_contains E-26-06 "the new pane opens the carried file" "$(guest 'pgrep -a -x nvim')" "$ROOT_DIR/alpha.txt"
  expect_out E-26-06 "the vertical split puts its panes side by side" "tmux list-panes -t $mux_session -F '#{pane_top}' | sort -u | wc -l" 1
  expect_true E-26-06 "the destination stays in the original terminal window" "[[ \$(clients) == 1 ]]"
fi
shot E-26-06-destination-opened
kill_windows
pending E-26-06 "split icons show the intended pane geometry" "appearance requires Fable's inspection of E-26-06-second-ring"
pending E-26-10 "the Open with wedge shows an open-folder glyph in every context" "appearance requires Fable's inspection of E-26-10-second-ring"

summary
