# Dragging files

This file was written by an agent.

FileBlade carries two kinds of file drag. One stays inside FileBlade and ends in
the drop wheel or a folder row. The other belongs to the compositor and hands the
files to whatever application you drop them on. A Wayland drag owns the pointer
and the keyboard while it lasts, so a single gesture cannot be both, and the
button you press decides which one you get.

## The setting

**Files settings → Drag out**, or `dragOut` in the plugin config.

| Value | Dragging a row with the left button |
|---|---|
| `paste` (default) | stays inside FileBlade, exactly as it always has |
| `system` | hands the files to the application you drop on |

## Gestures

| Gesture | Result |
|---|---|
| Left button, `dragOut: system` | the application you drop on receives the files |
| Left button, `dragOut: paste` | the drag stays inside FileBlade |
| Right button | stays inside FileBlade and opens the drop wheel where you release it outside a blade |
| Shift + left button | stays inside FileBlade and pastes the absolute path |
| Ctrl + left button | stays inside FileBlade and pastes the relative path |
| The drop-wheel key during an internal drag | opens the wheel, as before |

Dropping on a folder row inside a blade moves or copies the files whichever
gesture started the drag, and folder rows also accept files dragged in from
other applications.

## What a system drag cannot do

The compositor owns the pointer and the keyboard for the whole drag, so during a
left-button drag under `dragOut: system` the drop wheel does not open, its keys
and scrolling do not reach FileBlade, and the file list does not scroll itself
when you reach its edge. Use the right button when you want the wheel, and shift
or control when you want a path pasted. Under the default setting none of this
applies, because every drag stays inside FileBlade.
