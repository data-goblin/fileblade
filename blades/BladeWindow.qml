import QtQuick
import Quickshell
import qs.Commons
import "../ui" as PluginUi
import "../theme"

FloatingWindow {
  id: window

  required property var host
  required property string edge
  property alias actionKeys: windowKeys
  PluginUi.ActionKeyGuard { id: windowKeys; active: window.bladeOpen && window.contentItem.Window.active; releaseRoot: window.contentItem }

  readonly property bool isRight: edge === "right"
  readonly property var blade: host.bladeFor(edge)
  readonly property bool windowMode: host.isWindowMode(edge)
  readonly property bool surfaceActive: windowMode
  readonly property bool bladeOpen: windowMode && !!blade.open
  readonly property int bladeWidth: Number(blade.width) || 380
  readonly property var slots: Array.isArray(blade.slots) ? blade.slots : []
  readonly property bool bladeFocused: bladeOpen && scope.activeFocus
  readonly property int surfaceOriginX: 0
  readonly property int surfaceOriginY: 0
  readonly property string dragScope: "local"
  readonly property int stackHeight: stack.height
  readonly property int footerHeight: Style.space(32)
  property int pendingSlotIndex: -1
  property string pendingPart: ""
  property bool shortcutsOpen: false

  function matchesScreen(targetScreen) {
    return true
  }

  function containsScenePoint(x, y) {
    return x >= 0 && y >= 0 && x <= window.width && y <= window.height
  }

  function slotItem(index) {
    return stack.slotItem(index)
  }

  function focusSlot(index, part) {
    if (!bladeOpen) return
    if (trashConsent.visible) { trashConsent.forceActiveFocus(); return }
    var target = slotItem(index)
    if (!target && window.slots.length > 0) target = slotItem(0)
    Qt.callLater(function() {
      if (window.host.focusedEdge !== window.edge) return
      var item = target
      if (item) item.takeFocus(part)
      else scope.forceActiveFocus()
    })
  }

  function requestSlotFocus(index, part) {
    pendingSlotIndex = index
    pendingPart = String(part || "")
    focusRequestTimer.restart()
  }

  title: host.windowTitle(edge)
  visible: bladeOpen
  property var creationScreen: null
  screen: creationScreen

  function chooseCreationScreen() {
    creationScreen = host.referenceScreen(null, edge) || (Quickshell.screens.length > 0 ? Quickshell.screens[0] : null)
  }

  onWindowModeChanged: {
    if (windowMode) chooseCreationScreen()
    else creationScreen = null
  }
  Component.onCompleted: if (windowMode) chooseCreationScreen()
  color: Color.bar.background
  minimumSize: Qt.size(host.minimumWidth, Style.space(240))
  implicitWidth: bladeWidth
  implicitHeight: Style.space(900)

  onVisibleChanged: {
    if (visible) {
      placementTimer.restart()
      requestSlotFocus(host.activeSlot(edge), "")
    } else {
      host.setWindowAddress(edge, "")
    }
  }

  onClosed: host.setOpen(edge, false, true)

  Timer {
    id: placementTimer
    interval: 120
    onTriggered: if (window.host.focusedEdge === window.edge) window.host.placeBladeWindow(window.edge)
  }

  Timer {
    id: focusRequestTimer
    interval: 180
    onTriggered: window.focusSlot(window.pendingSlotIndex, window.pendingPart)
  }

  Connections {
    target: window.host

    function onBladeFocusRequested(edge, targetScreen, slotIndex, part) {
      if (edge !== window.edge || !window.windowMode) return
      if (!window.host.windowAddress(window.edge)) placementTimer.restart()
      window.requestSlotFocus(slotIndex, part)
    }

    function onBladeFocusReleased(edge) {
      if (edge !== window.edge) return
      placementTimer.stop()
      focusRequestTimer.stop()
    }
  }

  FocusScope {
    id: scope
    enabled: !trashConsent.visible
    anchors.fill: parent
    anchors.bottomMargin: window.footerHeight

    onActiveFocusChanged: {
      window.host.reportFocus(window.edge, activeFocus, window.screen)
      if (activeFocus && !window.host.windowAddress(window.edge)) placementTimer.restart()
    }

    PointHandler {
      id: pressWatch
      acceptedButtons: Qt.AllButtons
      onActiveChanged: window.host.pointerHeld = active
    }

    BladeStack {
      id: stack
      anchors.fill: parent
      host: window.host
      edge: window.edge
      hostWindow: window
      bladeOpen: window.bladeOpen
      bladeFocused: window.bladeFocused
    }

    BladeSettings {
      anchors.fill: parent
      host: window.host
      surface: window
      visible: window.host.settingsOpen && window.host.settingsEdge === window.edge && window.bladeOpen
      z: 40
    }

    BladeShortcuts {
      anchors.fill: parent
      host: window.host
      surface: window
      visible: window.shortcutsOpen && window.bladeOpen
      z: 41
      onVisibleChanged: if (visible) forceActiveFocus()
    }
  }

  PluginUi.TrashConsent {
    id: trashConsent
    anchors.fill: parent
    preferences: window.host.service.preferences
    presented: window.edge === "left" && window.bladeOpen
  }

  Rectangle {
    id: footer
    enabled: !trashConsent.visible
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.bottom: parent.bottom
    height: window.footerHeight
    color: Qt.lighter(Color.bar.background, 1.035)

    BladeCorner {
      anchors.left: window.isRight ? undefined : parent.left
      anchors.right: window.isRight ? parent.right : undefined
      anchors.leftMargin: window.isRight ? 0 : Style.space(13)
      anchors.rightMargin: window.isRight ? Style.space(13) : 0
      anchors.verticalCenter: parent.verticalCenter
      glyph: "󰁍"
      tip: "Dock"
      tipActions: [{ button: "left", text: "Dock to the " + window.edge + " edge" }, { shortcut: "Super+T" }]
      onActivated: window.host.redock(window.edge, window.screen)
    }

    Text {
      textFormat: Text.PlainText
      readonly property var files: window.host.services ? window.host.services.files : null
      visible: !!files && files.modeBadge === "footer"
      anchors.left: window.isRight ? parent.left : undefined
      anchors.right: window.isRight ? undefined : parent.right
      anchors.leftMargin: Style.space(12)
      anchors.rightMargin: Style.space(12)
      anchors.verticalCenter: parent.verticalCenter
      text: files ? "\ue6ae " + files.editorMode : ""
      color: files ? files.editorModeColor : Color.muted
      font.family: Style.font.family
      font.pixelSize: Typography.caption
      font.weight: Font.DemiBold
    }
  }
}
