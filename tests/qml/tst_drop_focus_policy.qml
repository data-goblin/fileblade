import QtQuick
import QtTest
import "../../lib/DropFocusPolicy.js" as DropFocusPolicy

TestCase {
  name: "DropFocusPolicy"

  function test_folder_only_open_stays_in_fileblade() {
    verify(!DropFocusPolicy.transfersFocus("open", 0))
  }

  function test_file_or_mixed_open_yields_to_the_launched_app() {
    verify(DropFocusPolicy.transfersFocus("open", 1))
    verify(DropFocusPolicy.transfersFocus("open", 3))
  }

  function test_external_and_target_actions_data() {
    return [
      { tag: "application", action: "application" },
      { tag: "app", action: "app-open" },
      { tag: "nvim", action: "nvim-open" },
      { tag: "review", action: "review" },
      { tag: "terminal", action: "terminal" },
      { tag: "mux-open", action: "mux-open" }
    ]
  }

  function test_external_and_target_actions(data) {
    verify(DropFocusPolicy.transfersFocus(data.action, 0))
  }

  function test_non_launching_actions_keep_fileblade_focus() {
    verify(!DropFocusPolicy.transfersFocus("copy-paths", 2))
    verify(!DropFocusPolicy.transfersFocus("open-with", 2))
    verify(!DropFocusPolicy.transfersFocus("", 2))
  }

  function test_future_backend_actions_yield_by_default() {
    verify(DropFocusPolicy.transfersFocus("future-action", 0))
  }

  function test_focused_blade_grabs_focus_without_a_wheel() {
    verify(DropFocusPolicy.bladeGrabsFocus(true, false, false, false))
    verify(!DropFocusPolicy.bladeGrabsFocus(false, false, false, false))
    verify(!DropFocusPolicy.bladeGrabsFocus(true, true, false, false))
  }

  function test_open_wheel_keeps_the_pointer_when_the_blade_takes_focus_back() {
    verify(!DropFocusPolicy.bladeGrabsFocus(true, false, true, false))
  }

  function test_held_drag_keeps_the_blade_grab_while_its_wheel_is_open() {
    verify(DropFocusPolicy.bladeGrabsFocus(true, false, true, true))
  }
}
