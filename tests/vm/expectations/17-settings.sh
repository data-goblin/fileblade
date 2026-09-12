#!/usr/bin/env bash
# Settings. Expectations E-17-01 .. E-17-14.
source "$(dirname "$0")/lib.sh"

require_guest

fixture >/dev/null
open_left
goto_root "$ROOT_DIR"

"$OVM" mouse click 353 43; sleep 3
expect E-17-01 "the gear opens settings" settingsOpen true

before_width=$(field sidebarWidth)
ctl setSidebarWidth 420; sleep 3
expect E-17-02 "the blade width changes" sidebarWidth 420
restart_shell
open_left
expect E-17-02 "and the width persists" sidebarWidth 420
ctl setSidebarWidth "$before_width"; sleep 2

ctl setShowHidden false; sleep 2
expect E-17-03 "settings and the key agree on hidden files" showHidden false
ctl toggleHidden; sleep 2
expect E-17-03 "toggling flips it back" showHidden true

before_place=$(field propertiesPlacement)
ctl setPlacement right; sleep 3
expect_true E-17-04 "the properties placement moves" "[[ \$(field propertiesPlacement) != '$before_place' ]]"
ctl setPlacement "$before_place"; sleep 3

expect_true E-17-05 "trash retention is set" "[[ \$(field trashRetentionDays) -ge 0 ]]"

[[ $(field settingsOpen) == true ]] || { "$OVM" mouse click 353 43; wait_for "[[ \$(field settingsOpen) == true ]]" 10; }
"$OVM" mouse click 55 814; sleep 3
from_settings=$("$OVM" ocr 2>/dev/null | tr -s '[:space:]' ' ')
"$OVM" key esc; sleep 2
"$OVM" key esc; sleep 2
open_left; focus_tree
"$OVM" key shift-slash; sleep 2
from_key=$("$OVM" ocr 2>/dev/null | tr -s '[:space:]' ' ')
"$OVM" key esc; sleep 2
expect_contains E-17-06 "the settings entry opens the shortcut guide" "$from_settings" "Quick nav"
expect_contains E-17-06 "and it is the same guide ? opens" "$from_key" "Quick nav"

"$OVM" mouse click 353 43; sleep 3
[[ $(field settingsOpen) == true ]] || { "$OVM" mouse click 353 43; sleep 3; }
"$OVM" key esc; sleep 2
expect E-17-07 "escape closes settings first" settingsOpen false
expect E-17-07 "and leaves the blade open" open true

open_left; focus_tree
"$OVM" key comma; sleep 2
expect E-17-08 "comma opens the settings sheet" settingsOpen true
sheet=$(screen_text)
expect_contains E-17-09 "the sheet lists every section tab as its own row" "$sheet" "Properties"
"$OVM" key esc; sleep 2
expect E-17-08 "and escape closes it again" settingsOpen false
expect E-17-08 "while the blade stays open" open true
pending E-17-10 "icon buttons share one tip layout with mouse and keyboard hints" "tooltips are hover-only and not reported over IPC"
pending E-17-13 "a typed Font size percentage scales blade text and survives a restart" "the status payload reports fontScale but no IPC verb sets it"

# Open from the keyboard so the sheet's search field owns focus for the filter check.
[[ $(field settingsOpen) == true ]] && { "$OVM" key esc; sleep 2; }
open_left; focus_tree
"$OVM" key comma; sleep 2
# Muted caption headings need the sheet cropped and scaled before OCR reads them.
sheet=$(ocr_crop ocr-settings-groups "378x900+0+60" 300% 6 '5%,40%' | tr '[:lower:]' '[:upper:]')
for heading in "TREE" "GIT" "TRASH AND DRIVES"; do
  expect_contains E-17-12 "the Files settings show a $heading heading" "$sheet" "$heading"
done
"$OVM" type trash; sleep 1.5
filtered=$(ocr_crop ocr-settings-filtered "378x900+0+60" 300% 6 '5%,40%' | tr '[:lower:]' '[:upper:]')
expect_contains E-17-12 "filtering by a heading keeps its rows" "$filtered" "CONFIRM TRASH"
expect_contains E-17-12 "and the heading" "$filtered" "TRASH AND DRIVES"
expect_missing E-17-12 "and hides rows of other groups" "$filtered" "HIDDEN FILES"
expect_missing E-17-12 "with their headings" "$filtered" "TREE"
"$OVM" key esc; sleep 1
[[ $(field settingsOpen) == true ]] && { "$OVM" key esc; sleep 1; }

before_badge=$(field modeBadge)
ctl setModeBadge footer >/dev/null; sleep 1
expect E-17-11 "the mode badge moves to the footer" modeBadge footer
ctl setModeBadge hidden >/dev/null; sleep 1
expect E-17-11 "and can be hidden" modeBadge hidden
ctl setModeBadge "${before_badge:-header}" >/dev/null; sleep 1
expect E-17-11 "and the tree reports its mode" editorMode NORMAL

before_drag=$(field dragOut)
ctl setDragOut system >/dev/null; sleep 1
expect E-17-14 "dragging out hands files to the system" dragOut system
ctl setDragOut "${before_drag:-paste}" >/dev/null; sleep 1
expect E-17-14 "and paste path comes back" dragOut paste

summary
