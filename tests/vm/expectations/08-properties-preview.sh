#!/usr/bin/env bash
# Properties and previews. Expectations E-08-01 .. E-08-11.
source "$(dirname "$0")/lib.sh"

require_guest

fixture >/dev/null
open_left
goto_root "$ROOT_DIR"

ctl select "$ROOT_DIR/alpha.txt"; sleep 3
text=$(pane_text)
expect E-08-01 "the pane follows the file" selectedPath "$ROOT_DIR/alpha.txt"
expect_contains E-08-01 "and shows its path" "${text// /}" "alpha.txt"

# An entry with no preview card shows the whole metadata block without scrolling.
ctl select "$ROOT_DIR/broken-link"; sleep 3
meta=$(pane_text)
expect_contains E-08-01 "type is shown" "$meta" "Symbolic link"
expect_contains E-08-01 "owner is shown" "$meta" "omarchy:omarchy"
expect_contains E-08-01 "permissions are shown" "$meta" "rwxrwxrwx"
expect_contains E-08-01 "modified time is shown" "$meta" "2026-"

ctl select "$ROOT_DIR/small.png"; sleep 4
small=$(pane_text)
expect_missing E-08-02 "a small png needs no extra click" "$small" "Load preview"
expect_missing E-08-02 "and is not refused" "$small" "too large"

ctl select "$ROOT_DIR/huge.png"; sleep 4
expect_contains E-08-03 "an oversized image is refused" "$(pane_text)" "too large"

ctl select "$ROOT_DIR/link-image.png"; sleep 4
expect_contains E-08-04 "a linked image is refused" "$(pane_text)" "Linked"

ctl select "$ROOT_DIR/link-alpha.txt"; sleep 4
linked=$(pane_text)
expect_missing E-08-05 "a linked text file never shows a symlink loop" "$linked" "symbolic links"

ctl select "$ROOT_DIR/broken-link"; sleep 4
broken=$(pane_text)
expect_missing E-08-06 "a broken link never claims a loop" "$broken" "symbolic links"
expect_contains E-08-06 "and shows the dangling target" "$broken" "/nonexistent"

ctl select "$ROOT_DIR/long.txt"; sleep 4
expect_contains E-08-07 "a long text file previews" "$(pane_text)" "line 1"

# Screen y of the first pane word matching a pattern, so the pointer lands on
# the preview or the path row wherever the properties section starts.
pane_word_y() {
  local shot crop result
  shot=$("$OVM" shot ocr-pane-words 2>/dev/null | tail -1)
  [[ -f $shot ]] || return 1
  crop=$(mktemp --suffix=.png)
  if ! magick "$shot" -crop 378x480+0+600 +repage -colorspace gray -negate -resize 300% "$crop" 2>/dev/null; then
    rm -f -- "$crop"
    return 1
  fi
  result=$(tesseract "$crop" - --psm 6 tsv 2>/dev/null | awk -F '\t' -v pattern="$1" '$1 == 5 && $12 ~ pattern { print int(($8 + $10 / 2) / 3) + 600; exit }')
  rm -f -- "$crop"
  [[ $result =~ ^[0-9]+$ ]] || return 1
  printf '%s\n' "$result"
}

# Page keys follow the pointer: over the preview they page the preview, over
# the property rows they page the pane. The owner row only enters the pane crop
# once the pane itself has paged. The pointer starts on the tree so the move
# onto the preview is real travel that hands the pane keyboard focus; the path
# row is too dim for OCR until then.
"$OVM" mouse move "$ROW_X" "$(row_y 1)"; sleep 1
preview_y=$(pane_word_y '^[Ll]ine$')
[[ $preview_y =~ ^[0-9]+$ ]] && { "$OVM" mouse move 100 $((preview_y + 70)); sleep 1.5; }
path_y=$(pane_word_y '^/home/.*/long[.]txt$')
if [[ $preview_y =~ ^[0-9]+$ && $path_y =~ ^[0-9]+$ ]]; then
  "$OVM" key pgdn; sleep 1.5
  paged=$(pane_text)
  expect_contains E-08-11 "Page Down over a long preview shows later lines" "$paged" "line 22"
  expect_missing E-08-11 "and leaves the pane where it was" "$paged" "omarchy:omarchy"
  "$OVM" key pgup; sleep 1.5
  back=$(pane_text)
  expect_contains E-08-11 "Page Up over the preview returns to the first lines" "$back" "line 3"
  expect_missing E-08-11 "and the later lines are gone again" "$back" "line 22"
  "$OVM" mouse move 100 "$path_y"; sleep 1.5
  "$OVM" key pgdn; sleep 1.5
  pane=$(pane_text)
  expect_missing E-08-11 "Page Down over the property rows leaves the preview alone" "$pane" "line 22"
  expect_contains E-08-11 "and pages the pane instead" "$pane" "omarchy:omarchy"
  "$OVM" key pgup; sleep 1.5
  "$OVM" mouse move "$ROW_X" "$(row_y 1)"; sleep 1
else
  fail harness "E-08-11 locate the preview" "no preview line or path row in the pane OCR"
fi

ctl select "$ROOT_DIR/deep"; sleep 3
dirtext=$(pane_text)
expect_contains E-08-08 "a directory shows directory metadata" "$dirtext" "Directory"

pending E-08-09 "an empty properties pane shows the dimmed FileBlade wordmark" "the wordmark is a raster image the guest OCR cannot read"

ctl select "$ROOT_DIR/logo.svg"; sleep 4
svgtext=$(pane_text)
expect_missing E-08-10 "an svg shows no preview box" "$svgtext" "externally"
expect_missing E-08-10 "and no preview placeholder" "$svgtext" "Preview"
expect_contains E-08-10 "while its properties still show" "$svgtext" "logo.svg"

summary
