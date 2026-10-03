#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"

require_guest

fixture >/dev/null
open_left
goto_root "$ROOT_DIR"

ctl select "$ROOT_DIR/alpha.txt"; sleep 3
text=$(pane_text)
expect E-08-01 "the pane follows the file" selectedPath "$ROOT_DIR/alpha.txt"
expect_contains E-08-01 "and shows its path" "${text// /}" "alpha.txt"

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

ctl select "$ROOT_DIR/deep"; sleep 3
dirtext=$(pane_text)
expect_contains E-08-08 "a directory shows directory metadata" "$dirtext" "Directory"

pending E-08-09 "an empty properties pane shows the dimmed FileBlade wordmark" "the wordmark is a raster image the guest OCR cannot read"

ctl select "$ROOT_DIR/logo.svg"; sleep 4
svgtext=$(pane_text)
expect_missing E-08-10 "an svg shows no preview box" "$svgtext" "externally"
expect_missing E-08-10 "and no preview placeholder" "$svgtext" "Preview"
expect_contains E-08-10 "while its properties still show" "$svgtext" "logo.svg"

guest "ln -sfn ../deep/inner $ROOT_DIR/dest/linked; cp $ROOT_DIR/small.png $ROOT_DIR/deep/inner/inner.png" >/dev/null
ctl select "$ROOT_DIR/dest/linked/deep.txt"; sleep 4
through=$(pane_text)
expect_contains E-08-11 "a text file inside a linked folder explains the link" "$through" "reached"
expect_missing E-08-11 "and shows no error code" "$through" "os error"
ctl select "$ROOT_DIR/dest/linked/inner.png"; sleep 4
through=$(pane_text)
expect_contains E-08-11 "an image inside a linked folder explains the link" "$through" "reached"
expect_missing E-08-11 "and shows no error code" "$through" "os error"
guest "rm -f $ROOT_DIR/deep/inner/inner.png" >/dev/null

summary
