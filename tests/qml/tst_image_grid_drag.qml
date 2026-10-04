import QtQuick
import QtTest
import "../../ui" as Ui

TestCase {
  id: suite
  name: "ImageGridDrag"
  width: 800
  height: 500
  visible: true
  when: windowShown

  property var wheel: null
  property var callbacks: []
  property var launches: []
  property int dismissals: 0

  QtObject {
    id: service
    property var bladeHost: QtObject {
      property bool pressActive: false
      function windowTitle(edge) { return edge }
    }
    function backendRequest(command, args, generation, callback) {
      if (command === "drop-context") suite.callbacks.push(callback)
      else suite.launches.push(args)
      return String(suite.callbacks.length + suite.launches.length)
    }
    function cancelBackendRequest(id, generation) {}
    function yieldFocusForExternalLaunch() {}
    function referenceScreen(value) { return null }
  }

  QtObject {
    id: files
    property var dropWheel: suite.wheel
  }

  QtObject {
    id: moduleContext
    function service(name) { return name === "files" ? files : null }
  }

  Component {
    id: gridFactory
    Ui.ImageGrid {
      id: grid
      width: 400
      height: 400
      showTimeline: false
      sizeStep: 2
      context: moduleContext
      onDismissRequested: suite.dismissals++
      onDragBegan: function(item, scene, modifiers) {
        suite.wheel.beginDrag([String(item.path)], [{ name: String(item.name), isDir: false }], null, true, scene.x, scene.y, null)
      }
      onDragMoved: function(item, scene, modifiers) { suite.wheel.updateDrag(scene.x, scene.y, scene.x > grid.width, modifiers) }
      onDragEnded: function(item, canceled) {
        if (canceled) suite.wheel.cancelDrag()
        else suite.wheel.endDrag(undefined, undefined, undefined)
      }
    }
  }

  function initTestCase() {
    var url = Qt.resolvedUrl("../../controllers/DropWheelController.qml")
    var request = new XMLHttpRequest()
    request.open("GET", url, false)
    request.send()
    var source = request.responseText.replace("import Quickshell\n", "")
    source = source.slice(0, source.lastIndexOf("  Variants {")) + "}\n"
    wheel = Qt.createQmlObject(source.replace("required property var service", "property var service"), suite, url)
    wheel.service = service
  }

  function cleanupTestCase() { wheel.destroy() }

  function init() {
    wheel.cancelDrag()
    wheel.close()
    callbacks = []
    launches = []
    dismissals = 0
  }

  function pictures() {
    var rows = []
    for (var i = 0; i < 6; i++) rows.push({ path: "/library/picture-" + i + ".png", name: "picture-" + i + ".png", date: "2026-09-0" + (i + 1), text: "picture", fields: {} })
    return rows
  }

  function firstTile(grid) {
    var found = null
    function walk(item) {
      if (found || !item) return
      if (item.path === "/library/picture-5.png" && item.pending !== undefined) { found = item; return }
      for (var i = 0; i < item.children.length; i++) walk(item.children[i])
      if (item.contentItem) walk(item.contentItem)
    }
    walk(grid)
    return found
  }

  function startDrag() {
    var grid = createTemporaryObject(gridFactory, suite)
    grid.items = pictures()
    waitForRendering(grid)
    var tile = null
    tryVerify(function() { tile = firstTile(grid); return tile !== null && tile.width > 0 })
    var start = tile.mapToItem(suite, tile.width / 2, tile.height / 2)
    mousePress(suite, start.x, start.y)
    for (var step = 1; step <= 8; step++) mouseMove(suite, start.x + step * 10, start.y + step * 2)
    tryCompare(wheel, "dragActive", true)
    for (var x = start.x + 80; x <= 600; x += 40) mouseMove(suite, x, start.y + 20)
    verify(grid.activeFocus)
    return { grid: grid, tile: tile, end: Qt.point(600, start.y + 20) }
  }

  function test_space_outside_the_blade_opens_the_wheel_and_release_clears_the_drag() {
    var drag = startDrag()
    verify(wheel.dragOutside)
    keyPress(Qt.Key_Space)
    verify(wheel.wheelOpen)
    verify(wheel.wheelFromDrag)
    keyRelease(Qt.Key_Space)
    tryCompare(wheel, "wheelOpen", false)
    mouseRelease(suite, drag.end.x, drag.end.y)
    tryCompare(wheel, "dragActive", false)
    verify(!drag.grid.dragging)
  }

  function test_escape_cancels_the_drag_without_dismissing_the_gallery() {
    var drag = startDrag()
    keyClick(Qt.Key_Escape)
    compare(wheel.dragActive, false)
    compare(dismissals, 0)
    mouseRelease(suite, drag.end.x, drag.end.y)
    compare(wheel.dragActive, false)
    compare(launches.length, 0)
    verify(!drag.grid.dragging)
  }

  function test_hiding_the_gallery_mid_drag_cancels_it() {
    var drag = startDrag()
    drag.grid.visible = false
    compare(wheel.dragActive, false)
    mouseRelease(suite, drag.end.x, drag.end.y)
    compare(wheel.dragActive, false)
  }
}
