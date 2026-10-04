#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"

require_guest

set_slots() { ctl setBladeSlots "$1" "base64:$(printf %s "$2" | base64 -w0)"; }
layout() { "$OVM" ipc "$PLUGIN" blades 2>/dev/null; }
slots() { layout | jq -c ".blades.$1.slots"; }
modules() { slots "$1" | jq -c '[.[].modules | map(.module)]'; }
reopen() { ctl closeBlade "$1"; sleep 1; ctl openBlade "$1"; sleep 2; }
shot() { "$OVM" shot "$1"; }

original=$(guest "cat -- $(printf '%q' "$(field bladeLayoutPath)")")
if ! jq -e '.blades.left.slots and .blades.right.slots' <<<"$original" >/dev/null; then
  fail harness "save the original layout" "layout document is unavailable"
  summary
fi
fixture_dir=""
drag_pgid=""
stop_drag() {
  [[ -n $drag_pgid ]] || return 0
  [[ $drag_pgid =~ ^[1-9][0-9]+$ ]] || return 1
  guest "kill -TERM -- -$drag_pgid 2>/dev/null || true"
  if ! wait_for "guest '! kill -0 -- -$drag_pgid 2>/dev/null'" 5; then
    guest "kill -KILL -- -$drag_pgid 2>/dev/null || true"
    wait_for "guest '! kill -0 -- -$drag_pgid 2>/dev/null'" 5 || return 1
  fi
  drag_pgid=""
}
restore() {
  if ! stop_drag; then
    fail harness "stop the drag producer before restoration" "process group $drag_pgid could not be stopped; restoration skipped and fixture retained"
    return 1
  fi
  set_slots left '[]'
  set_slots right '[]'
  local edge expected observed failed=0
  for edge in left right; do
    set_slots "$edge" "$(jq -c ".blades.$edge.slots" <<<"$original")"
    ctl setBladeWidth "$edge" "$(jq -r ".blades.$edge.width" <<<"$original")"
    [[ $(jq -r ".blades.$edge.mode" <<<"$original") != window ]] || ctl undockBlade "$edge"
    if [[ $(jq -r ".blades.$edge.open" <<<"$original") == true ]]; then ctl openBlade "$edge"; else ctl closeBlade "$edge"; fi
    expected=$(jq -Sc ".blades.$edge.slots | map({id,fraction,active,collapsed,modules:[.modules[] | {module}]})" <<<"$original")
    observed=$(slots "$edge" | jq -Sc .)
    if [[ $observed != "$expected" ]]; then fail harness "restore $edge sections" "got $observed wanted $expected"; failed=1; fi
  done
  if [[ -n $fixture_dir ]]; then guest "rm -f -- $fixture_dir/before.txt $fixture_dir/after.txt; rmdir -- $fixture_dir" >/dev/null; fi
  return "$failed"
}
trap 'restore || exit 1' EXIT
trap 'exit 130' INT TERM

dock_blades
ctl hideDropWheel
set_slots right '[]'
set_slots left '[{"id":"e20-files","fraction":0.5,"modules":[{"module":"files"}]},{"id":"e20-properties","fraction":0.5,"modules":[{"module":"properties"}]}]'
set_slots right '[{"id":"e20-notes","fraction":0.5,"modules":[{"module":"notes"}]}]'
ctl setBladeWidth left 380
ctl setBladeWidth right 360
ctl openBlade left
ctl openBlade right
ctl focusBlade left
sleep 3
if [[ $(modules left) != '[["files"],["properties"]]' ]] || ! slots left | jq -e '.[0].id == "e20-files" and .[1].fraction == 0.5' >/dev/null; then
  fail harness "establish the section fixture" "slot setup did not reach the application"
  summary
fi

read -r output screen_width screen_height scale origin_x origin_y < <("$OVM" hypr monitors | jq -r '.[0] | [.name,.width,.height,.scale,.x,.y] | @tsv')
if [[ $screen_width != 1920 || $screen_height != 1080 || $scale != 1 || $origin_x != 0 || $origin_y != 0 ]]; then
  for n in $(seq -w 1 12); do pending "E-20-$n" "section arrangement" "pointer fixture requires 1920x1080 at scale 1"; done
  shot E-20-01-to-E-20-12-unsupported-geometry
  summary
fi

drag() {
  local name=$1 sx=$2 sy=$3 tx=$4 ty=$5 during=${6:-} script duration=1800
  [[ -z $during ]] || duration=6000
  script=$(cat <<TOML
[demo]
name = "sections-20-$name"
output = "$output"
capture = "screencopy"
screen = [1920, 1080]
region = { x = 0, y = 0, w = 1920, h = 1080 }
portal = false
leading_blank_ms = 100
show_keys = false
fps = 20
settle_ms = 200
gif_fps = 5
gif_width = 480
[points]
source = [$sx, $sy]
target = [$tx, $ty]
[[step]]
kind = "move"
to = "source"
ms = 200
[[step]]
kind = "drag"
to = "target"
hold_ms = 300
ms = $duration
[[step]]
kind = "hold"
ms = 300
TOML
)
  guest "printf '%s' $(printf '%q' "$script") > /tmp/fb-sections-20.toml"
  if [[ -n $during ]]; then
    drag_pgid=$(guest "rm -f /tmp/fb-sections-20.status; setsid sh -c 'democtl record /tmp/fb-sections-20.toml --out /tmp/fb-sections-20 --force >/tmp/fb-sections-20.log 2>&1; echo \$? >/tmp/fb-sections-20.status' </dev/null >/dev/null 2>&1 & echo \$!")
    if [[ ! $drag_pgid =~ ^[1-9][0-9]+$ ]]; then
      drag_pgid=${drag_pgid:-unknown}
      fail harness "retain the drag producer" "invalid process group: $drag_pgid"
      summary
    fi
    sleep 6
    shot "$during"
    if ! wait_for "guest '! kill -0 -- -$drag_pgid 2>/dev/null'" 30 || [[ $(guest 'cat /tmp/fb-sections-20.status' 2>/dev/null) != 0 ]]; then
      fail harness "physical drag $name" "democtl failed; see /tmp/fb-sections-20.log"
      summary
    fi
    drag_pgid=""
  elif ! guest 'democtl record /tmp/fb-sections-20.toml --out /tmp/fb-sections-20 --force >/tmp/fb-sections-20.log 2>&1'; then
    fail harness "physical drag $name" "democtl failed; see /tmp/fb-sections-20.log"
    summary
  fi
  sleep 1
  printf 'OBS %s left=%s right=%s\n' "$name" "$(slots left)" "$(slots right)"
  shot "$name"
}

shot sections-20-initial
"$OVM" mouse click 10 557
sleep 1
expect_true E-20-01 "caret collapses Properties" "slots left | jq -e '.[1].collapsed == true'"
shot E-20-01-properties-collapsed
reopen left
expect_true E-20-02 "collapsed state survives closing and reopening" "slots left | jq -e '.[1].collapsed == true'"
shot E-20-02-collapsed-after-reopen
"$OVM" mouse click 10 1032
sleep 1
expect_true E-20-01 "caret expands Properties again" "slots left | jq -e '.[1].collapsed == false'"
shot E-20-01-properties-expanded

fixture_dir=$(guest 'mktemp -d /tmp/fileblade-sections-20.XXXXXX')
if [[ ! $fixture_dir =~ ^/tmp/fileblade-sections-20\.[[:alnum:]]+$ ]]; then
  fixture_dir=""
  fail harness "create the undo fixture" "mktemp failed"
  summary
fi
guest "printf 'section undo fixture\n' > $fixture_dir/before.txt"
ctl setRoot "$fixture_dir"
sleep 2
ctl select "$fixture_dir/before.txt"
ctl focusTree
"$OVM" key f2
sleep 1
"$OVM" key ctrl-a
"$OVM" type after.txt
"$OVM" key ret
if ! wait_for "guest 'test -f $fixture_dir/after.txt'" 12; then
  fail E-20-11 "seed the file undo history" "rename did not complete; refusing to undo an unrelated operation"
  summary
fi
expect_out E-20-11 "rename a real file before arranging sections" "test -f $fixture_dir/after.txt && echo yes" yes
shot E-20-11-rename-before-layout-move

before=$(slots left)
drag E-20-09-invalid-drop-unchanged 70 557 950 500
expect_true E-20-09 "dropping outside both blades preserves the layout" "[[ \$(slots left) == $(printf '%q' "$before") ]]"
drag E-20-03-properties-above-files 70 557 230 100 E-20-06-section-insertion
expect_true E-20-03 "section title drops above Files" "[[ \$(modules left) == '[[\"properties\"],[\"files\"]]' ]]"
expect_true E-20-09 "valid drop retains the indicated order" "[[ \$(modules left) == '[[\"properties\"],[\"files\"]]' ]]"
shot E-20-09-valid-drop-order
drag E-20-03-properties-below-files 70 42 230 1000
expect_true E-20-03 "section title drops below Files" "[[ \$(modules left) == '[[\"files\"],[\"properties\"]]' ]]"
expect_true E-20-11 "dragging back reverses a layout move" "[[ \$(modules left) == '[[\"files\"],[\"properties\"]]' ]]"
shot E-20-11-layout-move-reversed

drag E-20-04-properties-moved-right 70 557 1700 1000
expect_true E-20-04 "section moves to the other blade" "[[ \$(modules left) == '[[\"files\"]]' && \$(modules right) == '[[\"notes\"],[\"properties\"]]' ]]"
before=$(slots right)
reopen right
expect_true E-20-10 "moved sections survive closing and reopening" "[[ \$(slots right) == $(printf '%q' "$before") ]]"
shot E-20-10-sections-after-reopen
ctl focusBlade left
ctl focusTree
"$OVM" key ctrl-z
wait_for "guest 'test -f $fixture_dir/before.txt && test ! -e $fixture_dir/after.txt'" 12
expect_out E-20-11 "Ctrl+Z reverses the file rename after a layout move" "test -f $fixture_dir/before.txt && test ! -e $fixture_dir/after.txt && echo yes" yes
expect_true E-20-11 "Ctrl+Z leaves the moved section in place" "[[ \$(modules left) == '[[\"files\"]]' && \$(modules right) == '[[\"notes\"],[\"properties\"]]' ]]"
shot E-20-11-undo-keeps-layout
ctl focusBlade right
drag E-20-05-properties-becomes-tab 1620 557 220 500 E-20-07-body-target
expect_true E-20-05 "module dropped on the Files body becomes a tab" "[[ \$(modules left) == '[[\"files\",\"properties\"]]' ]]"
before=$(slots left)
drag E-20-09-invalid-tab-drop-unchanged 150 42 950 500
expect_true E-20-09 "tab dropped outside both blades preserves its layout" "[[ \$(modules left) == '[[\"files\",\"properties\"]]' && \$(slots left) == $(printf '%q' "$before") ]]"
drag E-20-05-tabs-reordered 150 42 30 42 E-20-08-tab-insertion
expect_true E-20-05 "dragging a tab changes its position" "[[ \$(modules left) == '[[\"properties\",\"files\"]]' ]]"
before=$(slots left)
reopen left
expect_true E-20-10 "arranged tabs survive closing and reopening" "[[ \$(modules left) == '[[\"properties\",\"files\"]]' && \$(slots left) == $(printf '%q' "$before") ]]"
shot E-20-10-tabs-after-reopen

pending E-20-06 "horizontal insertion line during section drag" "drop geometry is not exposed; recordings require visual review"
pending E-20-07 "section body outlined while receiving a tab" "drop styling is not exposed; recordings require visual review"
pending E-20-08 "slim tab insertion bar with unobscured titles" "pixel shape and title legibility require visual review"

set_slots left '[{"id":"e20-files","fraction":0.5,"modules":[{"module":"files"}]},{"id":"e20-properties","fraction":0.5,"modules":[{"module":"properties"}]}]'
sleep 2
drag E-20-12-divider-resized 250 539 250 400
expect_true E-20-12 "divider changes both adjacent size fractions" "slots left | jq -e '.[0].fraction < 0.45 and .[1].fraction > 0.55 and ((.[0].fraction + .[1].fraction - 1) | fabs) < 0.001'"
before=$(slots left)
reopen left
expect_true E-20-12 "resized sections reopen at the chosen sizes" "[[ \$(slots left) == $(printf '%q' "$before") ]]"
shot E-20-12-resized-after-reopen
restore
trap - EXIT
summary
