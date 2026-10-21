import QtQuick
import qs.Commons
import "../panes" as PluginPanes
import qs.Commons as Commons

Item {
  id: stack

  required property var host
  required property string edge
  required property var hostWindow
  property bool bladeOpen: false
  property bool bladeFocused: false

  readonly property var slots: host.slots(edge)
  readonly property var fractions: host.normalizedFractions(edge)
  readonly property var geometry: host.slotGeometry(edge, height)
  readonly property int handleSize: host.slotHandleSize
  readonly property int usable: geometry.expandedSpace
  readonly property bool dropTarget: host.dragActive && hostWindow.surfaceActive !== false
    && host.dropEdge === edge && host.dragScreen === hostWindow.screen
  readonly property int dropIndex: dropTarget ? host.dropIndex : -1
  readonly property bool dropVisible: dropTarget && host.dropIndex >= 0 && !host.dropNoop && host.dropTabSlot < 0
  readonly property int dropTabSlot: dropTarget && !host.dropNoop ? host.dropTabSlot : -1
  readonly property string dropTabBand: dropTabSlot >= 0 ? host.dropTabBand : ""
  readonly property var ghostSlot: dropTabBand === "tabs" ? slotItem(dropTabSlot) : null
  readonly property var bodySlot: dropTabBand === "body" ? slotItem(dropTabSlot) : null
  property var dragFractions: null
  readonly property bool modulePickerOpen: {
    for (var i = 0; i < slotRepeater.count; i++) {
      var item = slotRepeater.itemAt(i)
      if (item && item.modulePickerOpen) return true
    }
    return false
  }

  function boundaryAt(localY) {
    var count = fractions.length
    if (count < 2) return -1
    for (var index = 0; index < count - 1; index++)
      if (localY < slotTop(index) + slotHeight(index) + handleSize) return index
    return count - 2
  }

  function resizedFractions(index, startFractions, deltaY) {
    if (!startFractions || index < 0 || index + 1 >= startFractions.length || usable <= 0) return null
    var expandedWeight = 0
    for (var i = 0; i < startFractions.length; i++)
      if (!host.slotCollapsed(edge, i)) expandedWeight += startFractions[i]
    var delta = deltaY / usable * expandedWeight
    var minimum = Math.min(expandedWeight * 0.45, host.minimumSlotHeight / usable * expandedWeight)
    var next = startFractions.slice()
    var upper = next[index] + delta
    var lower = next[index + 1] - delta
    if (upper < minimum) {
      lower -= minimum - upper
      upper = minimum
    }
    if (lower < minimum) {
      upper -= minimum - lower
      lower = minimum
    }
    next[index] = upper
    next[index + 1] = lower
    return next
  }

  function wholeSlotDragged(index) {
    return host.dragActive && host.dragEdge === edge && host.dragIndex === index
      && (host.dragTab < 0 || host.slotTabs(edge, index).length === 1)
  }

  function pushTabIndex() {
    if (!dropTarget || host.dropTabBand !== "tabs") return
    var target = slotItem(host.dropTabSlot)
    if (!target) return
    var origin = stack.mapToItem(null, 0, 0)
    var x = host.dragScreenX - (Number(hostWindow.surfaceOriginX) || 0) - origin.x
    host.setDropTabIndex(target.tabInsertionIndex(x))
  }

  Connections {
    target: stack.host
    function onDragScreenXChanged() { stack.pushTabIndex() }
    function onDropTabSlotChanged() { stack.pushTabIndex() }
    function onDropTabBandChanged() { stack.pushTabIndex() }
  }

  function activeFractions() {
    return dragFractions && dragFractions.length === fractions.length ? dragFractions : fractions
  }

  function slotTop(index) {
    if (!dragFractions || dragFractions.length !== fractions.length)
      return index >= 0 && index < geometry.tops.length ? geometry.tops[index] : 0
    var current = activeFractions()
    var expandedWeight = 0
    for (var i = 0; i < current.length; i++) if (!host.slotCollapsed(edge, i)) expandedWeight += current[i]
    var y = 0
    for (var j = 0; j < index && j < current.length; j++) {
      y += host.slotCollapsed(edge, j)
        ? geometry.heights[j]
        : (expandedWeight > 0 ? usable * current[j] / expandedWeight : 0)
      y += handleSize
    }
    return Math.round(y)
  }

  function slotHeight(index) {
    var current = activeFractions()
    if (index < 0 || index >= current.length) return 0
    if (host.slotCollapsed(edge, index)) return geometry.heights[index]
    if (!dragFractions || dragFractions.length !== fractions.length) return geometry.heights[index]
    var expandedWeight = 0
    for (var i = 0; i < current.length; i++) if (!host.slotCollapsed(edge, i)) expandedWeight += current[i]
    return expandedWeight > 0 ? Math.round(usable * current[index] / expandedWeight) : 0
  }

  function slotItem(index) {
    return index >= 0 && index < slotRepeater.count ? slotRepeater.itemAt(index) : null
  }

  function focusActionMenu() {
    if (!fileActionsMenu.visible) return false
    fileActionsMenu.focusCurrent()
    return true
  }

  function dropLineY() {
    if (dropIndex < 0) return 0
    if (dropIndex >= slots.length) return Math.max(0, height - 3)
    if (dropIndex === 0) return 0
    return Math.max(0, slotTop(dropIndex) - Math.round(handleSize / 2) - 1)
  }

  Repeater {
    id: slotRepeater
    model: stack.slots.length

    delegate: BladeSlot {
      required property int index
      host: stack.host
      edge: stack.edge
      slotIndex: index
      hostWindow: stack.hostWindow
      bladeOpen: stack.bladeOpen
      bladeFocused: stack.bladeFocused
      x: 0
      y: stack.slotTop(index)
      width: stack.width
      height: stack.slotHeight(index)
      opacity: stack.wholeSlotDragged(index) ? 0.45 : 1
    }
  }

  PluginPanes.FileActionsMenu {
    id: fileActionsMenu
    controller: stack.host.services.files
    hostWindow: stack.hostWindow
  }

  Repeater {
    model: Math.max(0, stack.slots.length - 1)

    delegate: Item {
      id: handle
      required property int index
      x: 0
      y: stack.slotTop(index + 1) - stack.handleSize
      width: stack.width
      height: stack.handleSize
      z: 10

      property real pressedY: 0
      property var startFractions: null
      readonly property bool resizable: !stack.host.slotCollapsed(stack.edge, index)
        && !stack.host.slotCollapsed(stack.edge, index + 1)
      readonly property bool active: resizable && (handlePointer.containsMouse || handlePointer.pressed)

      Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        height: 2
        visible: handle.active
        color: Commons.Color.accent
      }

      Row {
        anchors.centerIn: parent
        spacing: Style.space(2)
        visible: handle.resizable && !handle.active

        Repeater {
          model: 3

          Rectangle {
            width: Style.space(2)
            height: width
            radius: width / 2
            color: Commons.Color.muted
          }
        }
      }

      MouseArea {
        id: handlePointer
        anchors.fill: parent
        enabled: handle.resizable
        hoverEnabled: true
        cursorShape: handle.resizable ? Qt.SizeVerCursor : Qt.ArrowCursor
        acceptedButtons: Qt.LeftButton

        onPressed: function(mouse) {
          handle.pressedY = handlePointer.mapToItem(stack, mouse.x, mouse.y).y
          handle.startFractions = stack.fractions.slice()
          stack.dragFractions = handle.startFractions.slice()
        }

        onPositionChanged: function(mouse) {
          if (!(mouse.buttons & Qt.LeftButton) || !handle.startFractions || stack.usable <= 0) return
          var point = handlePointer.mapToItem(stack, mouse.x, mouse.y)
          var next = stack.resizedFractions(handle.index, handle.startFractions, point.y - handle.pressedY)
          if (next) stack.dragFractions = next
        }

        function finish() {
          if (stack.dragFractions) stack.host.setSlotFractions(stack.edge, stack.dragFractions)
          stack.dragFractions = null
          handle.startFractions = null
        }

        onReleased: finish()
        onCanceled: finish()
      }
    }
  }

  Rectangle {
    anchors.fill: parent
    visible: stack.dropVisible
    color: Util.alpha(Commons.Color.accent, 0.06)
    z: 25
  }

  Rectangle {
    readonly property int bodyTop: stack.bodySlot ? stack.bodySlot.tabBarHeight : 0
    visible: stack.dropTabBand === "body"
    x: 0
    y: stack.slotTop(stack.dropTabSlot >= 0 ? stack.dropTabSlot : 0) + bodyTop
    width: stack.width
    height: Math.max(0, stack.slotHeight(stack.dropTabSlot >= 0 ? stack.dropTabSlot : 0) - bodyTop)
    color: Util.alpha(Commons.Color.accent, 0.16)
    border.width: 2
    border.color: Commons.Color.accent
    radius: Style.cornerRadius
    z: 29
  }

  Loader {
    active: !!stack.ghostSlot && !stack.ghostSlot.tabbed
    x: 0
    y: stack.slotTop(stack.dropTabSlot >= 0 ? stack.dropTabSlot : 0)
    width: stack.width
    height: stack.host.tabBarHeight
    z: 29

    sourceComponent: BladeTabBar {
      slot: stack.ghostSlot
      context: stack.ghostSlot.context
      interactive: false
      dropIndex: stack.host.dropTabIndex
    }
  }

  Rectangle {
    x: 0
    y: stack.dropLineY()
    width: stack.width
    height: 3
    radius: 1.5
    visible: stack.dropVisible
    color: Commons.Color.accent
    z: 30
  }
}
