#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"

require_guest

fixture >/dev/null
open_left

LIB="$(dirname "$0")/capacity_lib.py"
WIDTH=$(field sidebarWidth)
FIX=/home/omarchy/capacity-fixture
BIND=/home/omarchy/capacity-bind
IMG=/home/omarchy/capacity-fixture.img
THEMES=/home/omarchy/.config/omarchy/themes
UDEV_RULE=/run/udev/rules.d/99-fileblade-capacity-fixture.rules
theme_dir() { guest 't=~/.local/state/omarchy/current/theme; [ -e "$t" ] || t=~/.config/omarchy/current/theme; readlink -f "$t"'; }
ORIGINAL_THEME=$(guest 'n=~/.local/state/omarchy/current/theme.name; [ -s "$n" ] && cat "$n" || basename "$(readlink -f ~/.config/omarchy/current/theme)"')

cleanup() {
  ctl setRoot "$ROOT_DIR" >/dev/null 2>&1
  [[ -n $ORIGINAL_THEME ]] && guest "omarchy-theme-set $ORIGINAL_THEME" >/dev/null 2>&1
  "$OVM" sudo "umount -l $FIX/stack 2>/dev/null; umount -l $BIND 2>/dev/null; umount -l $FIX 2>/dev/null; [ -n '$LOOP' ] && losetup -d $LOOP 2>/dev/null; rm -f $IMG $UDEV_RULE; udevadm control --reload; rm -r $FIX $BIND $THEMES/capacity-green $THEMES/capacity-noblue 2>/dev/null; true" >/dev/null 2>&1
  guest "rm -f $ROOT_DIR/blob.bin $ROOT_DIR/fixlink" >/dev/null 2>&1
}
trap cleanup EXIT

LOOP=""
"$OVM" sudo "umount -l $BIND 2>/dev/null; umount -l $FIX 2>/dev/null; rm -r $FIX $BIND 2>/dev/null; rm -f $IMG; true" >/dev/null 2>&1
guest "dd if=/dev/zero of=$IMG bs=1M count=64 status=none && mkfs.ext4 -q -F $IMG && mkdir -p $FIX $BIND" >/dev/null
rule=$(printf 'SUBSYSTEM=="block", KERNEL=="loop*", ATTR{loop/backing_file}=="%s", ENV{UDISKS_IGNORE}="1"\n' "$IMG" | base64 -w0)
"$OVM" sudo "mkdir -p /run/udev/rules.d && printf %s $rule | base64 -d > $UDEV_RULE && udevadm control --reload" >/dev/null 2>&1
LOOP=$("$OVM" sudo "losetup --find --show $IMG" 2>/dev/null | tr -d '\r' | tail -1)
if [[ $LOOP == /dev/loop* ]]; then
  "$OVM" sudo "udevadm trigger --action=change --settle $LOOP" >/dev/null 2>&1
  [[ $("$OVM" sudo "udevadm info -q property $LOOP" 2>/dev/null) == *UDISKS_IGNORE=1* ]] || fail harness "keep udisks off the fixture" "$LOOP has no UDISKS_IGNORE"
fi
if [[ $LOOP != /dev/loop* ]] || ! "$OVM" sudo "mount $LOOP $FIX && chown omarchy:omarchy $FIX && mount --bind $FIX $BIND" >/dev/null 2>&1; then
  fail harness "capacity fixture" "the loop image did not mount"
  summary
fi
"$OVM" sudo "rm -f $UDEV_RULE && udevadm control --reload && udevadm trigger --action=change --settle $LOOP" >/dev/null 2>&1
[[ $("$OVM" sudo "udevadm info -q property $LOOP" 2>/dev/null) != *UDISKS_IGNORE=1* ]] || fail harness "show the mounted fixture to udisks again" "$LOOP still has UDISKS_IGNORE"
guest "mkdir -p $FIX/stack $FIX/inner" >/dev/null
HOME_MOUNT=$(guest "findmnt -T /home/omarchy -n -o TARGET")

space_json() { guest "$GUEST_PLUGIN/fileblade -o json space $1"; }
shot() { "$OVM" shot "$1" 2>/dev/null | tail -1; }
bar_geometry() { python3 "$LIB" bar "$(shot "$1")" "$WIDTH"; }
bar_fill() { python3 "$LIB" fill-at "$(shot "$1")" "$WIDTH" "$Y_TOP" "$Y_BOTTOM" "$X0"; }
bar_track() { python3 "$LIB" track "$(shot "$1")" "$WIDTH" "$Y_BOTTOM" "$X0"; }
green_count() { python3 "$LIB" green-count "$(shot "$1")" "$WIDTH" "$Y_BOTTOM"; }
near() { local delta=$(( $1 - $2 )); [[ ${delta#-} -le ${3:-3} ]]; }
locate_word() {
  local shot crop point
  shot=$(shot "ocr-locate-$1")
  crop=$(mktemp --suffix=.png)
  magick "$shot" -crop "${WIDTH}x1080+0+0" +repage -colorspace gray -level '15%,40%' -negate -resize 300% "$crop" || { rm -f -- "$crop"; return 1; }
  point=$(tesseract "$crop" - --psm 6 tsv 2>/dev/null | awk -F '\t' -v word="$1" '$1 == 5 && index(tolower($12), tolower(word)) == 1 { x=int(($7+$9/2)/3); y=int(($8+$10/2)/3) } END { if (x) print x, y }')
  rm -f -- "$crop"
  [[ $point =~ ^[0-9]+\ [0-9]+$ ]] || return 1
  printf '%s\n' "$point"
}
open_settings() { [[ $(field settingsOpen) == true ]] || { click_settings_gear; wait_for "[[ \$(field settingsOpen) == true ]]" 10; }; }
close_settings() { "$OVM" key esc; sleep 1; wait_for "[[ \$(field settingsOpen) == false ]]" 5 || { "$OVM" key esc; sleep 1; }; [[ $(field open) == true ]] || open_left; }
sheet_text() { ocr_crop "settings-$1" "${WIDTH}x360+0+40" 300% 6 '5%,40%'; }
toggle_row_y() {
  local level shot crop y
  "$OVM" mouse move 900 900; sleep 0.5
  shot=$(shot "settings-row-$1")
  for level in '15%,40%' '5%,40%' '0%,60%'; do
    crop=$(mktemp --suffix=.png)
    magick "$shot" -crop "${WIDTH}x600+0+0" +repage -colorspace gray -level "$level" -negate -resize 300% "$crop" || { rm -f -- "$crop"; continue; }
    y=$(tesseract "$crop" - --psm 6 tsv 2>/dev/null | awk -F '\t' '$1 == 5 && tolower($12) ~ /^(drive|usage|under|toolbar)$/ && int(($8+$10/2)/3) > 140 { y=int(($8+$10/2)/3); print y; exit }')
    rm -f -- "$crop"
    [[ $y =~ ^[0-9]+$ ]] && { printf '%s\n' "$y"; return 0; }
  done
  return 1
}
toggle_capacity_setting() {
  local attempt found=0 y
  open_settings; sleep 3
  ctl focusBlade left; sleep 1
  for attempt in 1 2; do
    "$OVM" mouse click 95 118; sleep 1; "$OVM" key ctrl-a; "$OVM" type "drive usage"; sleep 2
    [[ $(sheet_text "filter-$attempt") == *usage* ]] && { found=1; break; }
  done
  if (( found )) && y=$(toggle_row_y "$1"); then
    "$OVM" mouse click $((WIDTH - 35)) "$y"; sleep 1.5
    close_settings
    return 0
  fi
  close_settings
  return 1
}
wait_ready() { wait_for "[[ \$(field capacityState) == ready && \$(field capacityShown) == true ]]" "${1:-15}"; }
"$OVM" mouse move 900 900 >/dev/null 2>&1

goto_root /home/omarchy
wait_ready 20
read -r run_home Y_TOP Y_BOTTOM X0 <<<"$(bar_geometry cap-home)"
frac_home=$(field capacityFraction)
ctl navigate "trash:///"; sleep 3
BAR_WIDTH=$(bar_track cap-track)
goto_root /home/omarchy
wait_ready
expected_home=$(python3 -c "print(round($BAR_WIDTH * $frac_home))")
expect_true E-98-01 "a blue bar under the toolbar shows a fill for the home folder" "[[ $run_home -gt 0 && $Y_TOP -gt 0 ]]"
expect_true E-98-01 "and the fill is as long as the hairline times the fraction" "[[ $BAR_WIDTH -ge 300 ]] && near $run_home $expected_home 3"
expect E-98-01 "and the bar names the home folder's mount" capacityMount "$HOME_MOUNT"

"$OVM" mouse move $((X0 + 12)) "$Y_BOTTOM"; sleep 1.5
text=$(ocr_crop cap-tip "${WIDTH}x130+0+$((Y_TOP > 120 ? Y_TOP - 120 : 0))" 300% 6 '5%,40%')
tip=$(field capacityTip)
hover_ok=$([[ $text == *full* && $text == *free* ]] && echo true || echo false)
tip_ok=$([[ $tip == *'% full'* && ( $tip == *' GB '* || $tip == *' MB '* || $tip == *' TB '* ) ]] && echo true || echo false)
expect_true E-98-02 "hovering the bar shows how full the drive is" "[[ $hover_ok == true ]]"
expect_true E-98-02 "and the status tip carries the same words" "[[ $tip_ok == true ]]"
"$OVM" mouse move 900 900; sleep 1

goto_root "$FIX"
wait_for "[[ \$(field capacityMount) == '$FIX' ]]" 15
wait_ready
guest "dd if=/dev/urandom of=$FIX/seed.bin bs=1M count=24 status=none" >/dev/null
ctl refresh; sleep 3
wait_ready
run_fix=$(bar_fill cap-fixture)
frac_fix=$(field capacityFraction)
expected_fix=$(python3 -c "print(round($BAR_WIDTH * $frac_fix))")
expect E-98-03 "opening the fixture names its mount" capacityMount "$FIX"
expect_true E-98-03 "and the fill shrinks to the fixture's fullness" "near $run_fix $expected_fix 3"
used_fix=$(field capacityUsed)
percent_fix=$(field capacityPercent)
goto_root "$BIND"
wait_for "[[ \$(field capacityMount) == '$BIND' ]]" 15
expect E-98-03 "the bind mount names the bind target" capacityMount "$BIND"
expect E-98-03 "with the fixture's used bytes" capacityUsed "$used_fix"
expect E-98-03 "and the fixture's percentage" capacityPercent "$percent_fix"
"$OVM" sudo "mount -t tmpfs -o size=8m tmpfs $FIX/stack" >/dev/null 2>&1
goto_root "$FIX/stack"
wait_for "[[ \$(field capacityMount) == '$FIX/stack' ]]" 15
expect E-98-03 "a mount stacked inside the fixture reports itself" capacityMount "$FIX/stack"
"$OVM" sudo "umount -l $FIX/stack" >/dev/null 2>&1
wait_for "[[ \$(field capacityMount) == '$FIX' ]]" 20
expect E-98-03 "and after it is unmounted the same path reports the fixture again" capacityMount "$FIX"

for location in trash:/// recent:/// drives:///; do
  ctl navigate "$location"; sleep 3
  expect E-98-04 "$location shows no fill" capacityShown false
  expect E-98-04 "and reports the bar hidden for $location" capacityState hidden
done
hidden_fill=$(bar_fill cap-hidden)
hidden_track=$(bar_track cap-hidden)
expect_true E-98-04 "and only the plain hairline is left under the toolbar" "[[ $hidden_fill -eq 0 ]] && near $hidden_track $BAR_WIDTH 3"
goto_root "$FIX"
wait_ready

df_line=$(guest "df -B1 --output=used,avail,size,pcent $FIX | tail -1")
read -r df_used df_avail df_size df_pcent <<<"$df_line"
json=$(space_json "$FIX")
space_used=$(jq -r .used <<<"$json"); space_avail=$(jq -r .available <<<"$json"); space_size=$(jq -r .size <<<"$json"); space_pcent=$(jq -r .percent <<<"$json")
[[ $space_used =~ ^[0-9]+$ ]] || fail harness "read fileblade space json" "$json"
expect_true E-98-05 "fileblade space reports the fixture's used bytes like df" "[[ $space_used == $df_used ]]"
expect_true E-98-05 "and its free bytes" "[[ $space_avail == $df_avail ]]"
expect_true E-98-05 "and its size" "[[ $space_size == $df_size ]]"
expect_true E-98-05 "and df's percentage" "[[ $space_pcent == ${df_pcent%\%} ]]"
line=$(guest "$GUEST_PLUGIN/fileblade space $FIX")
line_ok=$([[ $line == *"${df_pcent%\%}% full"* ]] && echo true || echo false)
expect_true E-98-05 "and the text form prints one line with that percentage" "[[ $line_ok == true ]]"

line=$(guest "$GUEST_PLUGIN/fileblade space")
line_ok=$([[ $line == *'% full'* && $line == *"on $FIX"* ]] && echo true || echo false)
expect_true E-98-06 "fileblade space without a path measures the open folder" "[[ $line_ok == true ]]"
ctl navigate "trash:///"; sleep 3
refusal=$(guest "$GUEST_PLUGIN/fileblade space 2>&1; echo \"exit=\$?\"")
refusal_ok=$([[ $refusal == *'not on a local drive'* && $refusal == *'exit=1'* ]] && echo true || echo false)
expect_true E-98-06 "and refuses while Trash is open" "[[ $refusal_ok == true ]]"
goto_root "$FIX"
wait_ready

before=$(field capacityUsed)
guest "dd if=/dev/urandom of=$ROOT_DIR/blob.bin bs=1M count=16 status=none" >/dev/null
guest "$GUEST_PLUGIN/fileblade copy-to $FIX $ROOT_DIR/blob.bin --wait" >/dev/null 2>&1
expect_true E-98-07 "copying a file into the fixture raises the fill right after the copy" "$(wait_for "[[ \$(field capacityUsed) -gt $before ]]" 10 && echo true || echo false)"

toggle_capacity_setting off || fail harness "find the Drive usage toggle" "the settings sheet did not show the row"
wait_for "[[ \$(field capacityShown) == false ]]" 10
expect E-98-08 "turning the setting off hides the fill at once" capacityShown false
off_fill=$(bar_fill cap-setting-off)
off_track=$(bar_track cap-setting-off)
expect_true E-98-08 "and leaves the plain hairline" "[[ $off_fill -eq 0 ]] && near $off_track $BAR_WIDTH 3"
sleep 2
restart_shell
open_left
goto_root "$FIX"; sleep 3
expect E-98-08 "the choice survives a shell restart" capacityShown false
toggle_capacity_setting on || fail harness "find the Drive usage toggle again" "the settings sheet did not show the row"
wait_ready 10
expect E-98-08 "and turning it back on brings the fill back" capacityShown true

goto_root "$FIX/inner"
wait_ready
"$OVM" mouse click $((X0 + 12)) "$Y_BOTTOM"; sleep 1.5
expect E-98-09 "clicking the bar changes nothing" rootPath "$FIX/inner"
expect E-98-09 "and opens nothing" settingsOpen false
"$OVM" mouse move 80 $((Y_TOP - 2)); sleep 1.5
text=$(screen_text)
hover_ok=$([[ $text != *'% full'* ]] && echo true || echo false)
expect_true E-98-09 "hovering the bottom edge of the Up button shows its own tip, not the bar's" "[[ $hover_ok == true ]]"
"$OVM" mouse click 80 $((Y_TOP - 2)); sleep 3
expect E-98-09 "and the Up button still works" rootPath "$FIX"
"$OVM" mouse move 900 900; sleep 1

ctl setRoot "$FIX"; sleep 0.4
ctl setRoot /home/omarchy
wait_for "[[ \$(field rootPath) == /home/omarchy ]]" 10
wait_ready
expect E-98-10 "switching folders quickly ends on the last folder's mount" capacityMount "$HOME_MOUNT"
sleep 5
expect E-98-10 "and stays there" capacityMount "$HOME_MOUNT"

before=$(field capacityRequestCount)
guest 'pkill -f "serve --max-concurrency"' >/dev/null 2>&1
expect_true E-98-11 "after the backend restarts the bar asks again on its own" "$(wait_for "[[ \$(field capacityRequestCount) -gt $before ]]" 40 && echo true || echo false)"
wait_ready 20
expect E-98-11 "and shows the fill" capacityShown true

goto_root "$FIX"
wait_ready
before=$(field capacityUsed)
guest "dd if=/dev/urandom of=$FIX/extra.bin bs=1M count=8 status=none" >/dev/null
expect_true E-98-12 "a write from outside FileBlade shows within the minute" "$(wait_for "[[ \$(field capacityUsed) -gt $before ]]" 75 && echo true || echo false)"

"$OVM" sudo "umount -l $BIND; umount -l $FIX; mount $LOOP $FIX; mount --bind $FIX $BIND" >/dev/null 2>&1; sleep 3
percent_now=$(jq -r .percent <<<"$(space_json "$FIX")")
volume=$(backend mounts | jq -c --arg img "$IMG" '[.volumes[]|select(.image == $img)][0]')
volume_percent=$(jq -r .percent <<<"$volume")
expect_true E-98-13 "the backend reports df's percentage for the image from a fresh sample" "[[ $volume_percent == $percent_now ]]"
ctl navigate "drives:///"; sleep 4
if point=$(locate_word capacity); then
  read -r word_x word_y <<<"$point"
  read -r drives_fill drives_track <<<"$(python3 "$LIB" drives-bar "$(shot cap-drives)" "$word_y" $((WIDTH - 130)) $((WIDTH - 20)))"
  volume_fraction=$(jq -r .fraction <<<"$volume")
  drives_expected=$(python3 -c "print(round($drives_track * $volume_fraction))")
  expect_true E-98-13 "and its Drives row draws a bar as full as the backend fraction says" "[[ $drives_track -ge 20 ]] && near $drives_fill $drives_expected 2"
else
  fail E-98-13 "and its Drives row draws a bar as full as the toolbar bar says" "the image's row was not located on screen"
fi
goto_root "$FIX"

guest "rm -rf /tmp/capacity-theme-base; cp -r $(theme_dir)/. /tmp/capacity-theme-base; mkdir -p $THEMES/capacity-green $THEMES/capacity-noblue; cp -r /tmp/capacity-theme-base/. $THEMES/capacity-green/; cp -r /tmp/capacity-theme-base/. $THEMES/capacity-noblue/; sed -i 's/^blue *=.*/blue = \"#22cc44\"/' $THEMES/capacity-green/colors.toml; sed -i '/^blue *=/d' $THEMES/capacity-noblue/colors.toml; rm -rf /tmp/capacity-theme-base" >/dev/null 2>&1
guest "omarchy-theme-set capacity-green" >/dev/null 2>&1; sleep 4
restart_shell; open_left
goto_root "$FIX"; wait_ready 20
"$OVM" mouse move 900 900; sleep 1
count=$(green_count cap-theme-green)
expect_true E-98-14 "the fill takes the theme's blue when a theme defines one" "[[ $count -gt 0 ]]"
guest "omarchy-theme-set capacity-noblue" >/dev/null 2>&1; sleep 4
restart_shell; open_left
goto_root "$FIX"; wait_ready 20
"$OVM" mouse move 900 900; sleep 1
count_green=$(green_count cap-theme-noblue)
count_blue=$(bar_fill cap-theme-noblue)
expect_true E-98-14 "and falls back to blue when the theme has none" "[[ $count_green -eq 0 && $count_blue -gt 0 ]]"
guest "omarchy-theme-set $ORIGINAL_THEME" >/dev/null 2>&1; sleep 4
restart_shell; open_left

guest "ln -sfn $FIX/inner $ROOT_DIR/fixlink" >/dev/null
goto_root "$ROOT_DIR/fixlink"
wait_for "[[ \$(field capacityMount) == '$FIX' ]]" 15
expect E-98-15 "a symlink root keeps its own path in the location" rootPath "$ROOT_DIR/fixlink"
expect E-98-15 "while the bar reports the folder it points at" capacityMount "$FIX"

goto_root "$FIX"; wait_ready
count=$(field capacityRequestCount)
ctl closeBlade left; sleep 3
expect E-98-16 "closing the blade reports the bar hidden" capacityState hidden
sleep 70
expect E-98-16 "and stops the minute refresh" capacityRequestCount "$count"
open_left
expect_true E-98-16 "reopening resumes it" "$(wait_for "[[ \$(field capacityRequestCount) -gt $count ]]" 20 && echo true || echo false)"
ctl setBladeSlotCollapsed left 0 true; sleep 2
expect E-98-16 "collapsing the section hides the bar" capacityShown false
count=$(field capacityRequestCount)
sleep 65
expect E-98-16 "and stops the refresh too" capacityRequestCount "$count"
ctl setBladeSlotCollapsed left 0 false; sleep 2
wait_ready 10
expect E-98-16 "expanding it brings the bar back" capacityShown true

goto_root "$FIX/inner"; wait_ready
guest "chmod 000 $FIX/inner" >/dev/null
ctl refresh
expect_true E-98-17 "a folder that cannot be read turns the bar into the plain hairline" "$(wait_for "[[ \$(field capacityState) == unavailable ]]" 15 && echo true || echo false)"
expect E-98-17 "while the tree stays where it was" rootPath "$FIX/inner"
guest "chmod 755 $FIX/inner" >/dev/null
expect_true E-98-17 "and the fill returns on its own once the folder is readable" "$(wait_ready 40 && echo true || echo false)"

goto_root "$FIX/inner"; wait_ready
guest "rm -rf $FIX/inner" >/dev/null
wait_for "[[ \$(field rootPath) == '$FIX' ]]" 20
expect E-98-11 "removing the open folder moves the tree to its parent" rootPath "$FIX"
wait_ready 15
expect E-98-11 "with the parent's fill" capacityMount "$FIX"
"$OVM" sudo "umount -l $BIND; umount -l $FIX" >/dev/null 2>&1
ctl refresh; sleep 2
wait_for "[[ \$(field capacityMount) == '$HOME_MOUNT' ]]" 20
expect E-98-11 "unmounting the fixture under the open folder reports the filesystem left behind" capacityMount "$HOME_MOUNT"

goto_root "$ROOT_DIR"
summary
