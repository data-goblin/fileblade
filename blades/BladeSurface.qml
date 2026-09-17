import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.Commons
import qs.Ui
import "../ui" as PluginUi
import "../lib/PathText.js" as PathText
import "../theme"

PanelWindow {
  id: surface

  required property var host
  required property string edge
  property alias actionKeys: surfaceKeys
  PluginUi.ActionKeyGuard { id: surfaceKeys; active: surface.bladeOpen && !surface.keyboardFocusReleased; releaseRoot: surface.contentItem }

  readonly property bool isRight: edge === "right"
  readonly property var blade: host.bladeFor(edge)
  readonly property bool panelEnabled: host.panelActiveFor(screen, edge)
  readonly property bool windowMode: host.isWindowMode(edge)
  readonly property bool surfaceActive: panelEnabled && !windowMode
  readonly property bool bladeOpen: panelEnabled && !windowMode && !!blade.open
  property int liveWidth: -1
  readonly property int surfaceWidth: host.maximumWidth(screen ? screen.width : 0)
  readonly property int storedWidth: Number(blade.width) || 380
  readonly property int bladeWidth: Math.max(host.minimumWidth, Math.min(liveWidth > 0 ? liveWidth : storedWidth, surfaceWidth))
  readonly property var slots: Array.isArray(blade.slots) ? blade.slots : []
  readonly property bool bladeFocused: bladeOpen && !keyboardFocusReleased && scope.activeFocus
  property bool shortcutsOpen: false
  readonly property string barPosition: host.shell && host.shell.barConfig ? String(host.shell.barConfig.position || "top") : "top"
  readonly property bool barVertical: barPosition === "left" || barPosition === "right"
  readonly property int defaultBarSize: barVertical ? Style.bar.sizeVertical : Style.bar.sizeHorizontal
  readonly property int liveBarSize: {
    if (!host.shell || !host.shell.bar) return defaultBarSize
    return host.shell.bar.barHidden ? 0 : Math.max(0, Number(host.shell.bar.barSize) || defaultBarSize)
  }
  readonly property int barInsetLeft: barPosition === "left" ? liveBarSize : 0
  readonly property int barInsetRight: barPosition === "right" ? liveBarSize : 0
  readonly property int surfaceOriginX: isRight ? Math.max(0, (screen ? screen.width : 0) - barInsetRight - width) : barInsetLeft
  readonly property int surfaceOriginY: barPosition === "top" ? liveBarSize : 0
  readonly property string dragScope: "screen"
  readonly property int stackHeight: stack.height
  readonly property int footerHeight: Style.space(32)

  function openUpdateDialog() {
    var updates = surface.host.updates
    if (!updates) return
    var title = updates.available ? "Update available" : (updates.backendStale ? "Backend update needed" : "FileBlade is up to date")
    updateDialog.open([title].concat(updates.dialogLines()).join("\n"), [
      { key: "cancel", label: "Close" },
      { key: "recheck", label: "Check again" }
    ])
  }
  readonly property var frameSpec: Border.withWidth(
    Border.hyprlandActiveSpec(Color.accent, host.frameWidth),
    isRight ? "0 0 0 " + host.frameWidth : "0 " + host.frameWidth + " 0 0"
  )
  readonly property bool animationsEnabled: host.animateBlades
  readonly property bool actionMenuOpen: host.services && host.services.files ? !!host.services.files.actionMenuOpen : false
  readonly property bool actionMenuHere: actionMenuOpen
    && host.services.files.actionMenuScreen === screen
    && isRight === !!host.services.files.actionMenuOpenLeft
  readonly property int actionMenuLayerWidth: host.maximumWidth(screen ? screen.width : 0)
  readonly property bool dropWheelOpen: host.services && host.services.files ? !!host.services.files.dropWheelOpen : false
  readonly property var slideCurve: [0.32, 0.72, 0.0, 1.0, 1.0, 1.0]
  readonly property int slideInMs: 500
  readonly property int slideOutMs: 380
  property bool keyboardFocusReleased: true
  property bool parked: true
  property real slideOffset: 0
  readonly property real slideProgress: bladeWidth > 0 ? Math.max(0, Math.min(1, 1 - slideOffset / bladeWidth)) : 1
  property int pendingSlotIndex: -1
  property string pendingPart: ""
  property int pendingFocusRevision: -1
  property double openedAt: 0
  property bool pointerRefocusRequired: false
  property real lastSheetHoverX: -1
  property real lastSheetHoverY: -1
  readonly property point sheetHoverPosition: sheetHover.point.scenePosition

  function ownsFocus(revision) {
    return bladeOpen && host.focusMatches(edge, screen, revision)
  }

  function releaseKeyboardFocus() {
    focusRequestTimer.stop()
    pendingFocusRevision = -1
    keyboardFocusReleased = true
  }

  function containsScenePoint(x, y) {
    return x >= sheet.x && x <= sheet.x + sheet.width && y >= sheet.y && y <= sheet.y + sheet.height
  }

  function takeKeyboardFocus() {
    if (!ownsFocus(pendingFocusRevision)) return
    keyboardFocusReleased = false
  }

  function handleFocusGrabCleared() {
    var revision = host.focusRevision
    Qt.callLater(function() {
      if (!surface.ownsFocus(revision) || surface.keyboardFocusReleased) return
      if (sheetHover.hovered || stack.modulePickerOpen || surface.actionMenuOpen || surface.dropWheelOpen) return
      surface.host.releaseFocus(surface.edge, surface.screen, revision)
    })
  }

  function followSheetPointer() {
    if (!sheetHover.hovered || !pointerRefocusRequired) return
    var previousX = lastSheetHoverX
    var previousY = lastSheetHoverY
    lastSheetHoverX = sheetHoverPosition.x
    lastSheetHoverY = sheetHoverPosition.y
    if (previousX < 0 || Math.abs(lastSheetHoverX - previousX) + Math.abs(lastSheetHoverY - previousY) < 2) return
    pointerRefocusRequired = false
    if (!surface.bladeFocused && !surface.host.dragActive && !surface.actionMenuOpen && !surface.dropWheelOpen)
      surface.host.focusBlade(surface.edge, surface.screen, -1, "", false)
  }

  onSheetHoverPositionChanged: followSheetPointer()

  function slotItem(index) {
    return stack.slotItem(index)
  }

  function focusSlot(index, part) {
    var revision = pendingFocusRevision
    if (!ownsFocus(revision)) return
    takeKeyboardFocus()
    if (trashConsent.visible) { trashConsent.forceActiveFocus(); return }
    var target = slotItem(index)
    if (!target && surface.slots.length > 0) target = slotItem(0)
    Qt.callLater(function() {
      if (surface.keyboardFocusReleased) return
      if (!surface.ownsFocus(revision)) return
      if (target) target.takeFocus(part)
      else scope.forceActiveFocus()
    })
  }

  function commitLiveWidth() {
    if (liveWidth <= 0) return
    var width = liveWidth
    host.setWidth(edge, width, screen ? screen.width : 0, true)
    liveWidth = -1
  }

  function requestSlotFocus(index, part) {
    pendingFocusRevision = host.focusRevision
    pendingSlotIndex = index
    pendingPart = String(part || "")
    var recentlyOpened = Date.now() - openedAt < 260
    focusRequestTimer.interval = recentlyOpened ? 150 : 0
    focusRequestTimer.restart()
  }

  function showSheet() {
    slideOut.stop()
    if (parked) slideOffset = bladeWidth
    parked = false
    if (animationsEnabled && slideOffset > 0) slideIn.restart()
    else {
      slideIn.stop()
      slideOffset = 0
    }
  }

  function hideSheet() {
    slideIn.stop()
    if (animationsEnabled && slideOffset < bladeWidth) slideOut.restart()
    else {
      slideOut.stop()
      slideOffset = bladeWidth
      parked = true
    }
  }

  onBladeOpenChanged: {
    if (bladeOpen) {
      openedAt = Date.now()
      pointerRefocusRequired = true
      lastSheetHoverX = -1
      lastSheetHoverY = -1
      showSheet()
    } else {
      releaseKeyboardFocus()
      shortcutsOpen = false
      hideSheet()
    }
  }

  Component.onCompleted: {
    parked = !bladeOpen
    slideOffset = bladeOpen ? 0 : bladeWidth
  }

  NumberAnimation {
    id: slideIn
    target: surface
    property: "slideOffset"
    to: 0
    duration: surface.slideInMs
    easing.type: Easing.BezierSpline
    easing.bezierCurve: surface.slideCurve
  }

  NumberAnimation {
    id: slideOut
    target: surface
    property: "slideOffset"
    to: surface.bladeWidth
    duration: surface.slideOutMs
    easing.type: Easing.BezierSpline
    easing.bezierCurve: surface.slideCurve
    onFinished: if (!surface.bladeOpen) surface.parked = true
  }

  HyprlandFocusGrab {
    active: surface.bladeOpen && !surface.keyboardFocusReleased
    windows: [surface]
    onCleared: surface.handleFocusGrabCleared()
  }

  visible: panelEnabled && !windowMode && (bladeOpen || !parked)
  exclusionMode: ExclusionMode.Normal
  exclusiveZone: bladeOpen ? bladeWidth : 0
  implicitWidth: surfaceWidth
  color: "transparent"
  surfaceFormat.opaque: false
  mask: liveWidth > 0 ? null : (actionMenuHere ? actionMenuMask : sheetMask)

  Region {
    id: sheetMask
    item: surface.parked ? null : sheet
  }

  Item {
    id: actionMenuLayer
    x: surface.host.services.files.actionMenuOpenLeft ? surface.width - width : 0
    width: surface.actionMenuLayerWidth
    height: surface.height
    visible: surface.actionMenuHere
  }

  Region {
    id: actionMenuMask
    item: surface.actionMenuHere ? actionMenuLayer : null
  }

  HoverHandler {
    parent: surface.contentItem
    enabled: surface.actionMenuHere
    onHoveredChanged: if (!hovered && surface.actionMenuOpen)
      surface.host.actionMenuPointerExited()
  }

  anchors {
    top: true
    bottom: true
    left: !surface.isRight
    right: surface.isRight
  }

  WlrLayershell.namespace: "omarchy-fileblade-" + edge
  WlrLayershell.layer: WlrLayer.Top
  WlrLayershell.keyboardFocus: bladeOpen
    ? (keyboardFocusReleased ? WlrKeyboardFocus.None : WlrKeyboardFocus.OnDemand)
    : WlrKeyboardFocus.None

  Timer {
    id: focusRequestTimer
    interval: 0
    onTriggered: surface.focusSlot(surface.pendingSlotIndex, surface.pendingPart)
  }

  Connections {
    target: surface.host

    function onBladeFocusRequested(edge, targetScreen, slotIndex, part) {
      if (edge !== surface.edge || targetScreen !== surface.screen || surface.windowMode) {
        surface.releaseKeyboardFocus()
        return
      }
      surface.requestSlotFocus(slotIndex, part)
    }

    function onBladeFocusReleased(edge) {
      if (edge !== surface.edge) return
      surface.releaseKeyboardFocus()
      surface.pointerRefocusRequired = surface.host.externalFocusHandoff || sheetHover.hovered
      surface.lastSheetHoverX = sheetHover.hovered ? sheetHover.point.scenePosition.x : -1
      surface.lastSheetHoverY = sheetHover.hovered ? sheetHover.point.scenePosition.y : -1
    }
  }

  Item {
    id: sheet
    x: surface.isRight ? surface.width - surface.bladeWidth + surface.slideOffset : -surface.slideOffset
    y: 0
    width: surface.bladeWidth
    height: surface.height
    clip: true
    opacity: 0.4 + 0.6 * surface.slideProgress

    Rectangle {
      anchors.fill: parent
      color: Color.bar.background
    }

    PointHandler {
      id: pressWatch
      acceptedButtons: Qt.AllButtons
      onActiveChanged: surface.host.pointerHeld = active
    }

    HoverHandler {
      id: sheetHover
      enabled: surface.bladeOpen
      onHoveredChanged: {
        if (hovered) {
          surface.host.bladePointerEntered(surface.edge)
          surface.lastSheetHoverX = point.scenePosition.x
          surface.lastSheetHoverY = point.scenePosition.y
          if (!surface.pointerRefocusRequired && !surface.bladeFocused && !surface.host.dragActive)
            surface.host.focusBlade(surface.edge, surface.screen, -1, "", false)
        } else {
          surface.pointerRefocusRequired = false
          surface.lastSheetHoverX = -1
          surface.lastSheetHoverY = -1
          if (!stack.modulePickerOpen) surface.host.bladePointerExited(surface.edge, surface.screen)
        }
      }
    }

    FocusScope {
      id: scope
      enabled: !trashConsent.visible
      anchors.fill: parent
      anchors.leftMargin: surface.isRight ? Style.space(9) : 0
      anchors.rightMargin: surface.isRight ? 0 : Style.space(9)
      anchors.bottomMargin: surface.footerHeight

      onActiveFocusChanged: {
        surface.host.reportFocus(surface.edge, activeFocus)
      }

      BladeStack {
        id: stack
        anchors.fill: parent
        host: surface.host
        edge: surface.edge
        hostWindow: surface
        bladeOpen: surface.bladeOpen
        bladeFocused: surface.bladeFocused
      }

      Keys.onPressed: function(event) {
        if (event.text === "?" && !surface.shortcutsOpen) {
          surface.shortcutsOpen = true
          event.accepted = true
        } else if (event.text === "," && !surface.host.settingsOpen) {
          surface.host.setSettingsOpen(true, surface.edge)
          event.accepted = true
        }
      }

      BladeSettings {
        anchors.fill: parent
        host: surface.host
        surface: surface
        visible: surface.host.settingsOpen && surface.host.settingsEdge === surface.edge && surface.bladeOpen
        z: 40
      }

      BladeShortcuts {
        anchors.fill: parent
        host: surface.host
        surface: surface
        visible: surface.shortcutsOpen && surface.bladeOpen
        z: 41
      }
    }

    PluginUi.TrashConsent {
      id: trashConsent
      anchors.fill: parent
      preferences: surface.host.service.preferences
      presented: surface.edge === "left" && surface.bladeOpen && surface.screen === surface.host.referenceScreen(null, "left")
    }

    Rectangle {
      anchors.top: parent.top
      anchors.bottom: parent.bottom
      anchors.left: surface.isRight ? parent.left : undefined
      anchors.right: surface.isRight ? undefined : parent.right
      width: 1
      color: Util.alpha(Color.bar.text, 0.20)
    }

    Rectangle {
      id: footer
      enabled: !trashConsent.visible
      anchors.bottom: parent.bottom
      anchors.left: parent.left
      anchors.right: parent.right
      height: surface.footerHeight
      visible: surface.bladeOpen
      z: 30
      color: Qt.lighter(Color.bar.background, 1.035)

      Row { id: footerControls
        anchors.left: surface.isRight ? parent.left : undefined
        anchors.right: surface.isRight ? undefined : parent.right
        anchors.leftMargin: surface.isRight ? Style.space(13) : 0
        anchors.rightMargin: surface.isRight ? 0 : Style.space(13)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Style.space(5)
        layoutDirection: surface.isRight ? Qt.LeftToRight : Qt.RightToLeft

        BladeCorner {
          glyph: surface.isRight ? "»" : "«"
          tip: "Collapse"
          tipActions: [{ button: "left", text: "Close this blade" }, { shortcut: "Super+W" }]
          onActivated: surface.host.setOpen(surface.edge, false, true)
        }

        BladeCorner {
          glyph: "󰀿"
          tip: "Undock"
          tipActions: [{ button: "left", text: "Float as a window" }, { shortcut: "Super+T" }]
          onActivated: surface.host.toggleDock(surface.edge)
        }

        Rectangle {
          id: updateChip
          readonly property var updates: surface.host.updates
          visible: !!updates && updates.chipVisible
          anchors.verticalCenter: parent.verticalCenter
          width: updateChipLabel.implicitWidth + Style.space(14)
          height: Style.space(20)
          radius: height / 2
          readonly property color tone: updates && updates.available ? Color.accent : Color.muted
          color: updateChipPointer.containsMouse ? Util.alpha(tone, 0.28) : Util.alpha(tone, 0.16)
          border.width: 1
          border.color: Util.alpha(tone, 0.6)

          Text {
            id: updateChipLabel
            textFormat: Text.PlainText
            anchors.centerIn: parent
            text: updateChip.updates ? updateChip.updates.chipText : ""
            color: updateChip.tone
            font.family: Style.font.family
            font.pixelSize: Typography.caption
            font.weight: Font.DemiBold
          }

          MouseArea {
            id: updateChipPointer
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: surface.openUpdateDialog()
          }

          PanelToolTip {
            visible: updateChipPointer.containsMouse && !!updateChip.updates
            text: updateChip.updates ? updateChip.updates.summaryLines().join("\n") : ""
          }
        }
      }

      Text {
        id: modeBadge
        textFormat: Text.PlainText
        readonly property var files: surface.host.services ? surface.host.services.files : null
        readonly property real reserve: visible ? implicitWidth + Style.space(8) : 0
        visible: !!files && files.modeBadge === "footer"
        anchors.left: surface.isRight ? undefined : parent.left
        anchors.right: surface.isRight ? parent.right : undefined
        anchors.leftMargin: Style.space(12)
        anchors.rightMargin: Style.space(12)
        anchors.verticalCenter: parent.verticalCenter
        text: files ? "\ue6ae " + files.editorMode : ""
        color: files ? files.editorModeColor : Color.muted
        font.family: Style.font.family
        font.pixelSize: Typography.caption
        font.weight: Font.DemiBold
      }

      Text {
        id: contextLabel
        textFormat: Text.PlainText
        anchors.left: surface.isRight ? undefined : parent.left
        anchors.right: surface.isRight ? parent.right : undefined
        anchors.leftMargin: Style.space(12) + modeBadge.reserve
        anchors.rightMargin: Style.space(12) + modeBadge.reserve
        anchors.verticalCenter: parent.verticalCenter
        width: Math.max(0, parent.width - footerControls.width - Style.space(26) - modeBadge.reserve)
        horizontalAlignment: surface.isRight ? Text.AlignRight : Text.AlignLeft
        elide: Text.ElideLeft
        text: surface.host.services && surface.host.services.files ? PathText.name(surface.host.services.files.contextPath) : ""
        color: contextPointer.containsMouse ? Color.bar.text : Color.muted
        font.family: Style.font.family
        font.pixelSize: Typography.caption
        font.letterSpacing: 0.4

        MouseArea {
          id: contextPointer
          anchors.fill: parent
          hoverEnabled: true
        }

        PanelToolTip {
          visible: contextPointer.containsMouse && contextLabel.text !== ""
          text: (surface.host.services.files.projectContext && surface.host.services.files.projectMarker === ".git" ? "Git project context: " : "Folder context: ")
            + PathText.relative(surface.host.services.files.contextPath, "")
        }
      }
    }

    PluginUi.ActionDialog {
      id: updateDialog
      anchors.fill: parent
      z: 95
      onChosen: function(key) {
        var updates = surface.host.updates
        if (!updates) return
        if (key === "recheck") updates.check()
      }
    }

    Item {
      id: widthHandle
      anchors.top: parent.top
      anchors.bottom: parent.bottom
      anchors.left: surface.isRight ? parent.left : undefined
      anchors.right: surface.isRight ? undefined : parent.right
      width: Style.space(11)
      z: 20

      property real pressedSceneX: 0
      property real initialWidth: 0

      Item {
        anchors.centerIn: parent
        width: Style.space(9)
        height: Style.space(38)

        Column {
          anchors.centerIn: parent
          spacing: Style.space(2)

          Repeater {
            model: 3
            Rectangle {
              width: Style.space(2)
              height: width
              radius: width / 2
              color: resizeMouse.containsMouse || resizeMouse.pressed ? Color.accent : Color.muted
            }
          }
        }
      }

      MouseArea {
        id: resizeMouse
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton
        cursorShape: Qt.SizeHorCursor

        onPressed: function(mouse) {
          widthHandle.pressedSceneX = resizeMouse.mapToItem(null, mouse.x, mouse.y).x
          widthHandle.initialWidth = surface.bladeWidth
          surface.liveWidth = surface.bladeWidth
        }

        onPositionChanged: function(mouse) {
          if (!(mouse.buttons & Qt.LeftButton)) return
          var delta = resizeMouse.mapToItem(null, mouse.x, mouse.y).x - widthHandle.pressedSceneX
          var target = widthHandle.initialWidth + (surface.isRight ? -delta : delta)
          surface.liveWidth = Math.round(Math.max(surface.host.minimumWidth, Math.min(surface.surfaceWidth, target)))
        }

        onReleased: surface.commitLiveWidth()
        onCanceled: surface.commitLiveWidth()
      }
    }

    Item {
      anchors.fill: parent
      visible: surface.bladeFocused
      z: 60

      BorderOverlay {
        radius: Style.cornerRadius
        borderSpec: surface.frameSpec
      }
    }
  }
}
