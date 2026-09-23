import QtQuick
import QtTest
import "../../controllers" as Controllers

TestCase {
  name: "DropDragKeys"

  QtObject {
    id: fake
    property bool dragActive: true
    property bool wheelOpen: true
    property bool wheelFromDrag: true
    property int modifierKey: Qt.Key_Space
    property bool modifierHeld: false
    property int opens: 0
    property int cancellations: 0
    property int actions: 0
    property int closes: 0
    function modifierPressed() { opens++; return true }
    function cancelDrag() { cancellations++; dragActive = false; wheelOpen = false; wheelFromDrag = false }
    function activateKey(text) { actions++; return text === "h" }
    function close() { closes++ }
  }

  Controllers.DropWheelDragKeys { id: keys; controller: fake }

  function init() {
    keys.cancelRelease()
    fake.dragActive = false
    fake.dragActive = true
    fake.wheelOpen = true
    fake.wheelFromDrag = true
    fake.opens = fake.cancellations = fake.actions = fake.closes = 0
  }

  function key(code, text, repeat) {
    return { key: code, text: text, isAutoRepeat: !!repeat }
  }

  function test_action_letters_and_escape_run_once() {
    verify(keys.handlePress(key(Qt.Key_H, "h", false)))
    verify(keys.handlePress(key(Qt.Key_H, "h", true)))
    compare(fake.actions, 1)
    verify(keys.handlePress(key(Qt.Key_Escape, "", false)))
    verify(!fake.dragActive)
    verify(!fake.wheelOpen)
    verify(!fake.wheelFromDrag)
    verify(!keys.handlePress(key(Qt.Key_Escape, "", true)))
    compare(fake.cancellations, 1)
  }

  function test_held_or_repressed_modifier_cannot_reopen_a_canceled_drag() {
    fake.wheelOpen = false
    verify(keys.handlePress(key(Qt.Key_Space, " ", true)))
    compare(fake.opens, 0)
    verify(keys.handlePress(key(Qt.Key_Space, " ", false)))
    compare(fake.opens, 1)
    fake.wheelOpen = true
    verify(keys.handlePress(key(Qt.Key_Escape, "", false)))
    verify(!fake.dragActive)
    verify(!fake.wheelOpen)
    compare(fake.cancellations, 1)
    verify(!keys.handlePress(key(Qt.Key_Space, " ", true)))
    verify(!keys.handlePress(key(Qt.Key_Space, " ", false)))
    compare(fake.opens, 1)
    verify(!keys.handleRelease(key(Qt.Key_Space, " ", false)))
    wait(30)
    verify(!keys.handlePress(key(Qt.Key_Space, " ", false)))
    compare(fake.opens, 1)
    fake.dragActive = true
    verify(keys.handlePress(key(Qt.Key_Space, " ", false)))
    compare(fake.opens, 2)
  }

  function test_only_a_real_modifier_release_closes_the_drag_wheel() {
    verify(keys.handleRelease(key(Qt.Key_Space, " ", true)))
    wait(160)
    compare(fake.closes, 0)
    verify(keys.handleRelease(key(Qt.Key_Space, " ", false)))
    tryCompare(fake, "closes", 1)
  }

  function test_ending_the_drag_cancels_the_release_timer() {
    verify(keys.handleRelease(key(Qt.Key_Space, " ", false)))
    keys.cancelRelease()
    wait(160)
    compare(fake.closes, 0)
  }

  function test_without_a_drag_keys_bubble_to_the_tree() {
    fake.dragActive = false
    verify(!keys.handlePress(key(Qt.Key_H, "h", false)))
    verify(!keys.handleRelease(key(Qt.Key_Space, " ", false)))
    compare(fake.actions, 0)
  }
}
