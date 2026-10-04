set -u
OVM=${OVM:?set OVM to the ovm harness path}
PLUGIN=data-goblin.fileblade
FILEBLADE_SHAPE=${FILEBLADE_SHAPE:-plugin}
CONTROL_COMMAND=(omarchy-shell -q "$PLUGIN.control")
if [[ $FILEBLADE_SHAPE == native ]]; then
  CONTROL_COMMAND=("$OVM" ipc "$PLUGIN.control")
fi
ROOT_DIR=/home/omarchy/fbexp
fails=0
checks=0

skip() { [[ -n ${ONLY:-} && ${ONLY} != "$1" ]]; }

guest() { "$OVM" ssh "$1" 2>/dev/null; }
guest_alive() { "$OVM" status 2>/dev/null | grep '^ssh: ok' >/dev/null; }
require_guest() {
  guest_alive && return 0
  fail harness "guest is available" "the guest is gone; remaining checks not run"
  summary
}
ctl() {
  if [[ $FILEBLADE_SHAPE == native ]]; then
    "${CONTROL_COMMAND[@]}" "$@" >/dev/null
    return
  fi
  local cmd="${CONTROL_COMMAND[*]}" a
  for a in "$@"; do cmd+=" $(printf '%q' "$a")"; done
  guest "$cmd" >/dev/null
}
ctl_out() {
  if [[ $FILEBLADE_SHAPE == native ]]; then
    "${CONTROL_COMMAND[@]}" "$@"
    return
  fi
  local cmd="${CONTROL_COMMAND[*]}" a
  for a in "$@"; do cmd+=" $(printf '%q' "$a")"; done
  guest "$cmd"
}
ctl_path() { ctl "$1" "$2"; }
backend() {
  if [[ $FILEBLADE_SHAPE == native ]]; then
    "$OVM" ipc _backend "$@"
    return
  fi
  local command="$GUEST_PLUGIN/fileblade _backend" argument
  for argument in "$@"; do command+=" $(printf '%q' "$argument")"; done
  guest "$command"
}
status() { "$OVM" ipc "$PLUGIN" status 2>/dev/null; }
field() { status | jq -r ".$1"; }
tree_names() { "$OVM" ipc "$PLUGIN" tree "${1:-200}" 2>/dev/null | jq -r '[.entries[]?|.name]|join(",")'; }

pending=0
pending() { skip "$1" && return 0; pending=$((pending + 1)); printf 'PEND %s  %s :: %s\n' "$1" "$2" "$3"; }

pass() { checks=$((checks + 1)); printf 'ok   %s  %s\n' "$1" "$2"; }
fail() { checks=$((checks + 1)); fails=$((fails + 1)); printf 'FAIL %s  %s :: %s\n' "$1" "$2" "$3"; }

expect() {
  skip "$1" && return 0
  local got; got=$(field "$3")
  [[ $got == "$4" ]] && pass "$1" "$2" || fail "$1" "$2" "$3=$got wanted $4"
}

expect_out() {
  skip "$1" && return 0
  local got; got=$(guest "$3" | tr -d '\r')
  [[ $got == "$4" ]] && pass "$1" "$2" || fail "$1" "$2" "got [$got] wanted [$4]"
}

expect_contains() {
  skip "$1" && return 0
  [[ $3 == *"$4"* ]] && pass "$1" "$2" || fail "$1" "$2" "[$3] does not contain [$4]"
}

expect_missing() {
  skip "$1" && return 0
  [[ $3 != *"$4"* ]] && pass "$1" "$2" || fail "$1" "$2" "[$3] unexpectedly contains [$4]"
}

expect_true() {
  skip "$1" && return 0
  if eval "$3" >/dev/null 2>&1; then pass "$1" "$2"; else fail "$1" "$2" "condition failed: $3"; fi
}

blade_layer() { "$OVM" hypr layers 2>/dev/null | jq -r --arg n "omarchy-fileblade-$1" '[..|objects|select(.namespace? == $n)]|length'; }

left_open() { [[ $(blade_layer left) == 1 ]]; }
ensure_left_open() { left_open || { ctl toggleBladeFocus left; wait_for "[[ \$(blade_layer left) == 1 ]]" 12; }; }
ensure_left_closed() { left_open && { ctl toggleBladeFocus left; wait_for "[[ \$(blade_layer left) == 0 ]]" 12; }; return 0; }
ensure_unfocused() { [[ $(field focusedBlade) == "" ]] || { ctl releaseBladeFocus >/dev/null 2>&1; wait_for "[[ -z \$(field focusedBlade) ]]" 8; }; return 0; }

open_left() {
  [[ $(field open) == true && $(field focusedBlade) == left ]] && return 0
  ctl toggleBladeFocus left; sleep 2
  [[ $(field open) == true ]] || { ctl open; sleep 2; }
  [[ $(field focusedBlade) == left ]] || { ctl focusBlade left; sleep 1; }
}

wait_for() {
  local deadline=$((SECONDS + ${2:-20}))
  until eval "$1" >/dev/null 2>&1; do
    ((SECONDS > deadline)) && return 1
    sleep 1
  done
}

restart_shell() {
  if [[ $FILEBLADE_SHAPE == native ]]; then
    "$OVM" restart >/dev/null 2>&1 || return 1
    wait_for "[[ -n \$(field rootPath) ]]" 25
    return $?
  fi
  local attempt
  for attempt in 1 2; do
    "$(dirname "${BASH_SOURCE[0]}")/../stop-shell" || return 1
    "$OVM" restart-shell >/dev/null 2>&1
    if wait_for "[[ -n \$(field rootPath) ]]" 25; then
      sleep 2
      guest 'pkill -xf /usr/bin/quickshell' >/dev/null 2>&1
      return 0
    fi
    printf 'restart_shell: the shell did not answer on attempt %s\n' "$attempt" >&2
  done
  return 1
}

goto_root() { ctl setRoot "$1"; sleep 2; }

row_y() {
  local n=${1:-}
  [[ $n =~ ^[0-9]+$ ]] || { echo 152; return 1; }
  echo $((152 + n * 30))
}
ROW_X=130

row_index() { "$OVM" ipc "$PLUGIN" tree 300 2>/dev/null | jq -r --arg n "$1" '[.entries[]?|.name]|index($n)'; }
visible_row_y() {
  local shot crop width result
  shot=$("$OVM" shot "row-hit-${1//[^[:alnum:]._-]/_}" 2>/dev/null | tail -1)
  [[ -f $shot ]] || return 1
  width=$(field sidebarWidth)
  [[ $width =~ ^[0-9]+$ ]] || return 1
  crop=$(mktemp --suffix=.png)
  if ! magick "$shot" -crop "${width}x1080+0+0" +repage -colorspace gray -level 5%,30% -negate -resize 300% "$crop" 2>/dev/null; then
    rm -f -- "$crop"
    return 1
  fi
  result=$(tesseract "$crop" - --psm 6 tsv 2>/dev/null | awk -F '\t' -v needle="$1" '
    $1 == 5 {
      y = ($8 + $10 / 2) / 3
      if (toupper($12) ~ /^PROPERT/) bottom = bottom ? (y < bottom ? y : bottom) : y
      if (y < 135) next
      line = $2 FS $3 FS $4 FS $5
      words[line] = words[line] SUBSEP $12
      positions[line] = y
    }
    END {
      gsub(/[[:space:]]/, "", needle)
      for (line in words) {
        if (bottom && positions[line] >= bottom - 15) continue
        count = split(words[line], tokens, SUBSEP)
        for (start = 1; start <= count; start++) {
          value = ""
          for (last = start; last <= count; last++) {
            value = value tokens[last]
            if (value == needle && (!best || positions[line] < best)) best = positions[line]
            if (length(value) >= length(needle)) break
          }
        }
      }
      if (best) print int(best)
    }')
  rm -f -- "$crop"
  [[ $result =~ ^[0-9]+$ ]] || return 1
  printf '%s\n' "$result"
}

click_row() {
  local name=$1 want point command='wtype -k Home' got step deadline=$((SECONDS + 8))
  while true; do
    want=$(row_index "$name")
    [[ $want =~ ^[0-9]+$ ]] && break
    if ((SECONDS >= deadline)); then
      fail harness "click $name" "row is absent"
      summary
    fi
    sleep 0.5
  done
  ctl focusTree
  for ((step=0; step<want; step++)); do command+=' -k Down'; done
  if ((want > 0)); then command+=' -k Up'; else command+=' -k Down'; fi
  guest "$command"
  sleep 0.7
  got=$(field selectedPath)
  if [[ ${got##*/} == "$name" ]]; then
    fail harness "click $name" "could not move the cursor off the target before clicking"
    summary
  fi
  point=$(visible_row_y "$name")
  if [[ ! $point =~ ^[0-9]+$ ]]; then
    fail harness "click $name" "could not locate the row inside the visible tree"
    summary
  fi
  "$OVM" mouse click "$ROW_X" "$point"
  sleep 1.2
  got=$(field selectedPath)
  if [[ ${got##*/} != "$name" ]]; then
    fail harness "click $name" "pointer selected $got"
    summary
  fi
  CLICK_ROW_Y=$point
}
focus_tree() { "$OVM" mouse click "$ROW_X" "$(row_y 1)"; sleep 1.5; }
double_click() {
  local point_x=$1 point_y=$2
  [[ $point_x =~ ^[0-9]+$ && $point_y =~ ^[0-9]+$ ]] || return 1
  local fixture_source fixture_data
  fixture_source="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd -P)/tests/vm/double-click.toml"
  [[ -f $fixture_source ]] || return 1
  fixture_data=$(base64 -w0 "$fixture_source") || return 1
  guest "double_click_dir=\$(mktemp -d /tmp/fileblade-double-click.XXXXXX)
    printf '%s' $fixture_data | base64 -d > \"\$double_click_dir/template.toml\"
    sed 's/POINT_X/$point_x/g; s/POINT_Y/$point_y/g' \"\$double_click_dir/template.toml\" > \"\$double_click_dir/input.toml\"
    democtl record \"\$double_click_dir/input.toml\" --out \"\$double_click_dir/record\" >/dev/null 2>&1
    double_click_status=\$?
    rm -rf -- \"\$double_click_dir\"
    exit \$double_click_status"
}
screen_text() { "$OVM" ocr 2>/dev/null | tr -s '[:space:]' ' '; }
click_word() {
  local shot point crop width offset=0
  "$OVM" mouse move 900 900
  sleep 0.5
  shot=$("$OVM" shot "ocr-click-$1" | tail -1)
  width=$(field sidebarWidth)
  if [[ ${2:-left} == right ]]; then
    width=$(field propertiesBladeWidth)
    offset=$(($(magick identify -format '%w' "$shot") - width))
  fi
  crop=$(mktemp --suffix=.png)
  magick "$shot" -crop "${width}x1080+${offset}+0" +repage -colorspace gray -level '15%,40%' -negate -resize 300% "$crop" || { rm -f -- "$crop"; return 1; }
  point=$(tesseract "$crop" - --psm 6 tsv 2>/dev/null | awk -F '\t' -v word="$1" -v offset="$offset" '$1 == 5 && tolower($12) == tolower(word) { x=offset+int(($7+$9/2)/3); y=int(($8+$10/2)/3) } END { if (x) print x, y }')
  rm -f -- "$crop"
  [[ $point =~ ^[0-9]+\ [0-9]+$ ]] || { fail harness "locate $1" "not found in $shot"; return 1; }
  "$OVM" mouse click ${point}
  sleep 1
}
add_module_point() {
  local edge=${1:-left} shot crop width offset=0 point
  "$OVM" mouse move 900 900
  sleep 0.5
  shot=$("$OVM" shot "add-module-$edge" | tail -1)
  width=$(field sidebarWidth)
  if [[ $edge == right ]]; then
    width=$(field propertiesBladeWidth)
    offset=$(($(magick identify -format '%w' "$shot") - width))
  fi
  crop=$(mktemp --suffix=.png)
  magick "$shot" -crop "${width}x80+${offset}+0" +repage -colorspace gray -level '5%,40%' -negate -resize 400% "$crop" || { rm -f -- "$crop" "$shot"; return 1; }
  point=$(tesseract "$crop" - --psm 6 tsv 2>/dev/null | awk -F '\t' -v offset="$offset" '
    $1 == 5 && $12 == "+" {
      x = offset + int(($7 + $9 / 2) / 4); y = int(($8 + $10 / 2) / 4)
      if (!found || y < best_y - 4 || (y <= best_y + 4 && x < best_x)) { best_x = x; best_y = y; found = 1 }
    }
    END { if (found) print best_x, best_y }')
  rm -f -- "$crop" "$shot"
  [[ $point =~ ^[0-9]+\ [0-9]+$ ]] || return 1
  printf '%s\n' "$point"
}
click_settings_gear() {
  local point width
  width=$(field sidebarWidth)
  point=$(add_module_point left) && [[ $width =~ ^[0-9]+$ ]] || { fail harness 'find the settings gear' 'no + in the left header'; return 1; }
  "$OVM" mouse click $((width - 25)) "${point#* }"
}
ocr_crop() {
  local shot crop result
  shot=$("$OVM" shot "$1" 2>/dev/null | tail -1)
  [[ -f $shot ]] || return 1
  crop=$(mktemp --suffix=.png)
  if ! magick "$shot" -crop "$2" +repage -colorspace gray -level "${5:-15%,40%}" -negate -resize "$3" "$crop" 2>/dev/null; then
    rm -f -- "$crop"
    return 1
  fi
  result=$(tesseract "$crop" - --psm "$4" 2>/dev/null | tr -s '[:space:]' ' ')
  rm -f -- "$crop"
  printf '%s\n' "$result"
}
summary_text() { ocr_crop ocr-summary 378x28+0+130 400% 7; }
picker_text() {
  local width
  width=$(field sidebarWidth)
  [[ $width =~ ^[0-9]+$ ]] && ((width > 20)) || return 1
  ocr_crop ocr-picker "$((width - 20))x120+10+58" 400% 6 '5%,40%'
}
trash_names() {
  backend trash-list | jq -r '[.entries[]?|.name]|join(",")' 2>/dev/null
}
search_names() { "$OVM" ipc "$PLUGIN" searchResults "${1:-200}" 2>/dev/null | jq -r '[.entries[]?|.name]|join(",")'; }
tree_text() { ocr_crop ocr-tree 378x560+0+135 300% 11 '5%,30%'; }
pane_text() { ocr_crop ocr-pane 378x480+0+600 300% 6; }

blade_mode() { status | jq -r ".bladeModes.$1"; }
dock_blades() {
  local edge
  for edge in left right; do
    [[ $(blade_mode "$edge") == docked ]] && continue
    ctl dockBlade "$edge"
    wait_for "[[ \$(blade_mode $edge) == docked ]]" 15
  done
}

GUEST_PLUGIN=/home/omarchy/.config/omarchy/plugins/$PLUGIN
left_modules() { "$OVM" ipc "$PLUGIN" blades | jq -c '[.blades.left.slots[].modules[].module]'; }
reset_modules() {
  [[ $(left_modules) == '["files","properties"]' ]] && return 0
  if [[ $FILEBLADE_SHAPE == native ]]; then
    ctl setBladeSlots left "base64:$(printf '%s' '[{"module":"files"},{"module":"properties"}]' | base64 -w0)"
  else
    guest "$GUEST_PLUGIN/fileblade blade set left files,properties" >/dev/null 2>&1
  fi
  wait_for "[[ \$(left_modules) == '[\"files\",\"properties\"]' ]]" 12
  sleep 3
}

fixture() {
  dock_blades
  reset_modules
  [[ $(field sidebarWidth) == 380 ]] || ctl setSidebarWidth 380
  [[ $(field propertiesBladeWidth) == 360 ]] || ctl setPropertiesBladeWidth 360
  ctl setPriorityColumns type
  guest "find /home/omarchy/.local/share/Trash/files -mindepth 1 -delete 2>/dev/null; find /home/omarchy/.local/share/Trash/info -mindepth 1 -delete 2>/dev/null; true" >/dev/null
  local keep='.fixture-ok alpha.txt bravo.txt long.txt data.json .dotfile dots deep link-alpha.txt broken-link small.png huge.png link-image.png logo.svg repo empty dest'
  local prune="" name
  for name in $keep; do prune+=" ! -name $name"; done
  if [[ ${FORCE_FIXTURE:-0} != 1 ]] && [[ $(guest "test -f $ROOT_DIR/.fixture-ok && test -s $ROOT_DIR/huge.png && echo yes") == yes ]]; then
    guest "cd $ROOT_DIR && find . -mindepth 1 -maxdepth 1 $prune -exec rm -rf {} +; rm -rf dest/* empty/* deep/added-while-closed.txt; mkdir -p empty dest deep/inner; printf 'alpha\n' > alpha.txt; printf 'bravo\n' > bravo.txt; printf 'deep\n' > deep/inner/deep.txt" >/dev/null
    if [[ $(guest "cd $ROOT_DIR && test -f long.txt && test -f data.json && test -e dots/.only-hidden && test -L link-alpha.txt && test -L broken-link && test -s small.png && test -L link-image.png && test -s logo.svg && test -d repo/.git && echo yes") == yes ]]; then
      return 0
    fi
  fi
  guest "rm -rf $ROOT_DIR" >/dev/null 2>&1
  guest "mkdir -p $ROOT_DIR/empty $ROOT_DIR/dest $ROOT_DIR/deep/inner $ROOT_DIR/dots
    cd $ROOT_DIR
    printf 'alpha\n' > alpha.txt
    printf 'bravo\n' > bravo.txt
    printf 'line %s\n' \$(seq 1 300) > long.txt
    printf '{\"a\":1}\n' > data.json
    touch .dotfile
    touch dots/.only-hidden
    printf 'deep\n' > deep/inner/deep.txt
    ln -sf alpha.txt link-alpha.txt
    ln -sf /nonexistent broken-link
    magick -size 320x200 gradient:blue-yellow small.png
    magick -size 3000x3000 xc: +noise Random -define png:compression-level=0 huge.png
    ln -sf small.png link-image.png
    printf '<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><rect width="64" height="64" fill="#7aa2f7"/></svg>\n' > logo.svg
    mkdir -p repo && cd repo && git init -q .
    printf 'ignored\n' > ignored.txt
    printf 'ignored.txt\n' > .gitignore
    printf 'tracked\n' > tracked.txt
    git add tracked.txt .gitignore
    git -c user.email=a@b -c user.name=a commit -qm init
    printf 'changed\n' >> tracked.txt
    touch $ROOT_DIR/.fixture-ok
    echo FIXTURE-OK"
}

summary() {
  printf '\n%s: %d checks, %d failed, %d pending\n' "${0##*/}" "$checks" "$fails" "$pending"
  exit $((fails > 0))
}
