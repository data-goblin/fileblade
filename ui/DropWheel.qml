import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.Commons
import "../lib/FileIcons.js" as FileIcons
import "../theme"

PanelWindow {
  id: overlay

  required property var controller

  property bool wheelHere: false
  property bool ghostHere: false
  property bool toastHere: false
  ActionKeyGuard { id: actionKeys; active: overlay.wheelHere }

  function refreshActive() {
    wheelHere = controller.wheelOpen && controller.wheelScreen === screen
    ghostHere = controller.dragActive && controller.dragDocked && !controller.wheelOpen && controller.dragScreen === screen
    toastHere = controller.toast !== "" && !controller.wheelOpen && controller.wheelScreen === screen
  }

  Component.onCompleted: refreshActive()
  readonly property int screenWidth: screen ? screen.width : 0
  readonly property int screenHeight: screen ? screen.height : 0
  readonly property real innerRadius: controller.hubRadius + controller.gapRadius
  readonly property real labelRadius: (innerRadius + controller.outerRadius) / 2
  readonly property real subLabelRadius: (controller.subInnerRadius + controller.subOuterRadius) / 2
  readonly property real childLabelRadius: (controller.childInnerRadius + controller.childOuterRadius) / 2
  readonly property int wedgeCount: controller.ringItems.length
  readonly property real wedgeStep: wedgeCount > 0 ? 2 * Math.PI / wedgeCount : 0
  readonly property real separatorWidth: Math.max(2, Style.space(2))

  visible: wheelHere || ghostHere || toastHere
  color: "transparent"
  surfaceFormat.opaque: false
  exclusionMode: ExclusionMode.Ignore
  mask: wheelHere && !controller.wheelFromDrag ? fullInput : passThrough

  anchors {
    top: true
    bottom: true
    left: true
    right: true
  }

  WlrLayershell.namespace: "omarchy-fileblade-drop"
  WlrLayershell.layer: WlrLayer.Overlay
  WlrLayershell.keyboardFocus: wheelHere && !controller.dragActive && !controller.keyboardFocusReleased ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

  Region { id: passThrough }

  Region {
    id: fullInput
    x: 0
    y: 0
    width: overlay.width
    height: overlay.height
  }

  function alpha(value, amount) {
    return Qt.rgba(value.r, value.g, value.b, amount)
  }

  function pointOnRing(angle, radius) {
    return Qt.point(controller.wheelX + Math.cos(angle) * radius, controller.wheelY + Math.sin(angle) * radius)
  }

  function bundledMark(name) {
    var mark = String(name || "")
    return ["herdr", "tmux", "pane-horizontal", "pane-vertical"].indexOf(mark) >= 0
      ? Qt.resolvedUrl("../assets/marks/" + mark + ".svg") : ""
  }

  function applicationIcon(item) {
    var resolved = {
      icon: String(item.icon || ""),
      icon_source: item.icon_override ? "" : String(item.icon_source || ""),
      glyph: String(item.glyph || "")
    }
    if (typeof FileIcons.resolveApplication === "function") {
      var override = item.icon_override ? { icon: resolved.icon, glyph: resolved.glyph } : null
      var desktopId = String(item.desktop_id || "").replace(/\.desktop$/, "")
      var desktop = desktopId ? DesktopEntries.applications.values.find(function(entry) { return entry.id === desktopId }) || null : null
      resolved = FileIcons.resolveApplication(item, override, desktop, Quickshell.iconPath)
    }
    return { icon: resolved.icon, icon_source: String(resolved.icon_source || "") || bundledMark(resolved.icon), glyph: resolved.glyph }
  }

  function labelWidth(radius, step) {
    return Math.min(Style.space(48), Math.max(Style.space(24), radius * step * 0.8))
  }

  readonly property var backKeys: [Qt.Key_Escape, Qt.Key_Backspace]
  readonly property var acceptKeys: [Qt.Key_Return, Qt.Key_Enter, Qt.Key_Space]
  readonly property var previousKeys: [Qt.Key_Left, Qt.Key_Up, Qt.Key_H, Qt.Key_K]
  readonly property var nextKeys: [Qt.Key_Right, Qt.Key_Down, Qt.Key_L, Qt.Key_J]

  function moveFor(event) {
    if (previousKeys.indexOf(event.key) >= 0) return -1
    if (nextKeys.indexOf(event.key) >= 0) return 1
    if (event.key === Qt.Key_Tab) return event.modifiers & Qt.ShiftModifier ? -1 : 1
    return 0
  }

  function handleKey(event) {
    var repeated = actionKeys.isRepeat(event)
    if (backKeys.indexOf(event.key) >= 0) {
      if (!repeated) controller.back()
      return true
    }
    if (acceptKeys.indexOf(event.key) >= 0) {
      if (!repeated) controller.accept()
      return true
    }
    if (controller.activateKey(event.text, repeated)) return true
    var move = moveFor(event)
    if (move !== 0) {
      controller.moveHighlight(move)
      return true
    }
    return false
  }

  Connections {
    target: overlay.controller
    function onRingItemsChanged() { ring.requestPaint() }
    function onHighlightedChanged() { ring.requestPaint() }
    function onOuterItemsChanged() { ring.requestPaint() }
    function onOuterHighlightedChanged() { ring.requestPaint() }
    function onSubItemsChanged() { ring.requestPaint() }
    function onSubHighlightedChanged() { ring.requestPaint() }
    function onSubParentIndexChanged() { ring.requestPaint() }
    function onParentIndexChanged() { ring.requestPaint() }
    function onWheelXChanged() { ring.requestPaint() }
    function onWheelYChanged() { ring.requestPaint() }
    function onWheelScreenChanged() { overlay.refreshActive() }
    function onDragActiveChanged() { overlay.refreshActive() }
    function onDragOutsideChanged() { overlay.refreshActive() }
    function onDragDockedChanged() { overlay.refreshActive() }
    function onDragScreenChanged() { overlay.refreshActive() }
    function onToastChanged() { overlay.refreshActive() }
    function onKeyboardFocusReleasedChanged() {
      if (overlay.wheelHere && !overlay.controller.keyboardFocusReleased) keyTarget.forceActiveFocus()
    }
    function onWheelOpenChanged() {
      overlay.refreshActive()
      ring.requestPaint()
      if (overlay.wheelHere) keyTarget.forceActiveFocus()
    }
  }

  Item {
    id: keyTarget
    anchors.fill: parent
    focus: overlay.wheelHere && !overlay.controller.keyboardFocusReleased
    Keys.onPressed: function(event) { event.accepted = overlay.controller.dragActive ? overlay.controller.handleDragKey(event) : overlay.handleKey(event) }
    Keys.onReleased: function(event) {
      actionKeys.release(event)
      event.accepted = overlay.controller.handleDragKeyRelease(event)
    }
  }

  MouseArea {
    anchors.fill: parent
    enabled: overlay.wheelHere && !overlay.controller.wheelFromDrag
    hoverEnabled: enabled
    acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
    onPositionChanged: function(mouse) { overlay.controller.hover(mouse.x, mouse.y) }
    onClicked: function(mouse) {
      overlay.controller.clearPendingRelease()
      if (mouse.button !== Qt.LeftButton) {
        overlay.controller.back()
        return
      }
      var hit = overlay.controller.pointAt(mouse.x, mouse.y)
      if (hit.ring === "sub") {
        overlay.controller.activateSub(hit.index)
        return
      }
      if (hit.ring === "outer") {
        overlay.controller.activateChild(hit.index)
        return
      }
      if (hit.ring === "inner") {
        overlay.controller.activate(hit.index)
        return
      }
      var dx = mouse.x - overlay.controller.wheelX
      var dy = mouse.y - overlay.controller.wheelY
      if (Math.sqrt(dx * dx + dy * dy) > overlay.controller.extent + 28) overlay.controller.close()
    }
    onWheel: function(wheel) { overlay.controller.moveHighlight(wheel.angleDelta.y < 0 ? 1 : -1) }
  }

  function entryGlyph(entry) {
    if (entry.isSymlink) return FileIcons.entryIcon(entry.name, entry.isDir, true, false, false)
    if (entry.glyph) return String(entry.glyph)
    return FileIcons.entryIcon(entry.name, entry.isDir, false, false, entry.isGitRepo)
  }

  Item {
    id: ghost
    visible: overlay.ghostHere
    x: Math.max(0, Math.min(overlay.screenWidth - width, overlay.controller.pointerX + Style.space(12)))
    y: Math.max(0, Math.min(overlay.screenHeight - height, overlay.controller.pointerY + Style.space(10)))
    readonly property var entry: overlay.controller.dragEntries.length > 0 ? overlay.controller.dragEntries[0] : ({})
    readonly property bool armed: overlay.controller.pathForm !== ""
    readonly property int badgeReserve: overlay.controller.count > 1 ? Style.space(34) : 0
    width: Math.min(Style.space(260), Math.max(Style.space(150), ghostName.implicitWidth + Style.space(58) + badgeReserve))
    height: Style.space(34)

    Rectangle {
      anchors.fill: parent
      radius: Style.cornerRadius
      color: overlay.alpha(Color.popups.background, 0.92)
      border.width: ghost.armed ? 2 : 1
      border.color: ghost.armed ? Color.accent : Color.popups.border
    }

    Text {
      id: ghostIcon
      textFormat: Text.PlainText
      anchors.left: parent.left
      anchors.leftMargin: Style.space(10)
      anchors.verticalCenter: parent.verticalCenter
      text: overlay.entryGlyph(ghost.entry)
      color: Color.accent
      font.family: Style.font.family
      font.pixelSize: Typography.body + 3
    }

    Text {
      id: ghostName
      textFormat: Text.PlainText
      anchors.left: ghostIcon.right
      anchors.leftMargin: Style.space(7)
      anchors.right: ghostCount.visible ? ghostCount.left : parent.right
      anchors.rightMargin: Style.space(9)
      anchors.verticalCenter: parent.verticalCenter
      text: String(ghost.entry.name || "")
      color: Color.bar.text
      elide: Text.ElideMiddle
      font.family: Style.font.family
      font.pixelSize: Typography.body
      font.weight: Font.Medium
    }

    Rectangle {
      id: ghostCount
      visible: overlay.controller.count > 1
      anchors.right: parent.right
      anchors.rightMargin: Style.space(8)
      anchors.verticalCenter: parent.verticalCenter
      width: Math.max(Style.space(18), badge.implicitWidth + Style.space(8))
      height: Style.space(18)
      radius: height / 2
      color: Color.accent
      border.width: 1
      border.color: Color.popups.background

      Text {
        id: badge
        textFormat: Text.PlainText
        anchors.centerIn: parent
        text: String(overlay.controller.count)
        color: Color.popups.background
        font.family: Style.font.family
        font.pixelSize: Typography.caption
        font.weight: Font.Bold
      }
    }
  }

  Canvas {
    id: ring
    visible: overlay.wheelHere
    anchors.fill: parent

    function paintWedge(context, cx, cy, inner, outer, center, step, active) {
      var start = center - step / 2
      var end = center + step / 2
      context.beginPath()
      context.arc(cx, cy, outer, start, end, false)
      context.arc(cx, cy, inner, end, start, true)
      context.closePath()
      context.fillStyle = overlay.alpha(Color.popups.background, 0.94)
      context.fill()
      if (active) {
        context.fillStyle = overlay.alpha(Color.accent, 0.30)
        context.fill()
      }
      context.lineWidth = 1
      context.strokeStyle = overlay.alpha(Color.accent, active ? 0.85 : 0.42)
      context.stroke()
    }

    function cutSeparators(context, cx, cy, inner, outer, angles, width) {
      if (angles.length < 2) return
      context.save()
      context.globalCompositeOperation = "destination-out"
      context.lineWidth = width
      for (var i = 0; i < angles.length; i++) {
        var cos = Math.cos(angles[i])
        var sin = Math.sin(angles[i])
        context.beginPath()
        context.moveTo(cx + cos * (inner - 1), cy + sin * (inner - 1))
        context.lineTo(cx + cos * (outer + 1), cy + sin * (outer + 1))
        context.stroke()
      }
      context.restore()
    }

    onPaint: {
      var context = getContext("2d")
      context.reset()
      var control = overlay.controller
      var cx = control.wheelX
      var cy = control.wheelY
      var count = overlay.wedgeCount
      var width = overlay.separatorWidth
      var i
      var edges = []
      for (i = 0; i < count; i++) {
        paintWedge(context, cx, cy, overlay.innerRadius, control.outerRadius, control.wedgeAngle(i), overlay.wedgeStep, i === control.highlighted)
        edges.push(control.wedgeAngle(i) - overlay.wedgeStep / 2)
      }
      cutSeparators(context, cx, cy, overlay.innerRadius, control.outerRadius, edges, width)
      var children = control.outerItems.length
      var childEdges = []
      for (i = 0; i < children; i++) {
        paintWedge(context, cx, cy, control.childInnerRadius, control.childOuterRadius, control.childAngle(i), control.childStep, i === control.outerHighlighted)
        childEdges.push(control.childAngle(i) - control.childStep / 2)
      }
      if (children > 0) childEdges.push(control.childAngle(children - 1) + control.childStep / 2)
      cutSeparators(context, cx, cy, control.childInnerRadius, control.childOuterRadius, childEdges, width)
      var subEdges = []
      for (i = 0; i < control.subItems.length; i++) {
        paintWedge(context, cx, cy, control.subInnerRadius, control.subOuterRadius, control.subAngle(i), control.subStep, i === control.subHighlighted)
        subEdges.push(control.subAngle(i) - control.subStep / 2)
      }
      if (control.subItems.length > 0) subEdges.push(control.subAngle(control.subItems.length - 1) + control.subStep / 2)
      cutSeparators(context, cx, cy, control.subInnerRadius, control.subOuterRadius, subEdges, width)
      context.beginPath()
      context.arc(cx, cy, control.hubRadius, 0, 2 * Math.PI, false)
      context.fillStyle = Color.popups.background
      context.fill()
      context.lineWidth = 2
      context.strokeStyle = Color.accent
      context.stroke()
    }
  }

  Repeater {
    model: overlay.wheelHere ? overlay.controller.ringItems : []

    delegate: Item {
      id: wedge
      required property var modelData
      required property int index
      opacity: modelData.enabled === false ? 0.35 : 1
      readonly property var applicationIcon: overlay.applicationIcon(modelData)
      readonly property bool active: modelData.enabled !== false && index === overlay.controller.highlighted
      readonly property bool parentWedge: overlay.controller.hasChildren(index)
      readonly property real angle: overlay.controller.wedgeAngle(index)
      readonly property point center: overlay.pointOnRing(angle, overlay.labelRadius)
      readonly property point edge: overlay.pointOnRing(angle, overlay.controller.outerRadius - Style.space(7))
      readonly property point keyPoint: overlay.pointOnRing(angle, overlay.innerRadius + Style.space(9))
      width: overlay.labelWidth(overlay.labelRadius, overlay.wedgeStep)
      height: Style.space(24)
      x: center.x - width / 2
      y: center.y - height / 2

      SafeApplicationIcon {
        anchors.centerIn: parent
        anchors.horizontalCenterOffset: Math.cos(wedge.angle) * Style.space(4)
        anchors.verticalCenterOffset: Math.sin(wedge.angle) * Style.space(4)
        iconName: String(wedge.applicationIcon.icon || "")
        trustedIconSource: wedge.applicationIcon.icon_source
        monochrome: !wedge.active || monochromeMask !== "alpha"
        monochromeMask: String(wedge.modelData.icon_mask || "alpha")
        iconColor: wedge.modelData.enabled === false ? Color.muted : wedge.active ? Qt.lighter(Color.accent, 1.5) : Color.accent
        iconSize: Typography.body + 8
        fallbackGlyph: String(wedge.applicationIcon.glyph || "") || String(wedge.modelData.key || "").toUpperCase()
        fallbackColor: wedge.modelData.enabled === false ? Color.muted : Color.accent
        fallbackSize: Typography.body + 6
      }

      WheelKeyLetter {
        letter: String(wedge.modelData.key || "")
        active: wedge.active
        at: Qt.point(wedge.keyPoint.x - wedge.x, wedge.keyPoint.y - wedge.y)
      }

      Text {
        id: chevron
        visible: wedge.parentWedge
        textFormat: Text.PlainText
        x: wedge.edge.x - wedge.x - width / 2
        y: wedge.edge.y - wedge.y - height / 2
        rotation: wedge.angle * 180 / Math.PI
        opacity: wedge.active ? 1 : 0.8
        text: "󰅂"
        color: Color.popups.text
        font.family: Style.font.family
        font.pixelSize: Typography.bodySmall
      }
    }
  }

  Repeater {
    model: overlay.wheelHere ? overlay.controller.outerItems.concat(overlay.controller.subItems) : []

    delegate: Item {
      id: child
      required property var modelData
      required property int index
      opacity: modelData.enabled === false ? 0.35 : 1
      readonly property var applicationIcon: overlay.applicationIcon(modelData)
      readonly property bool third: index >= overlay.controller.outerItems.length
      readonly property int localIndex: third ? index - overlay.controller.outerItems.length : index
      readonly property bool active: localIndex === (third ? overlay.controller.subHighlighted : overlay.controller.outerHighlighted)
      readonly property real angle: third ? overlay.controller.subAngle(localIndex) : overlay.controller.childAngle(localIndex)
      readonly property real labelRadius: third ? overlay.subLabelRadius : overlay.childLabelRadius
      readonly property point center: overlay.pointOnRing(angle, labelRadius)
      readonly property point keyPoint: overlay.pointOnRing(angle, (third ? overlay.controller.subInnerRadius : overlay.controller.childInnerRadius) + Style.space(8))
      width: overlay.labelWidth(labelRadius, third ? overlay.controller.subStep : overlay.controller.childStep)
      height: Style.space(22)
      x: center.x - width / 2
      y: center.y - height / 2

      SafeApplicationIcon {
        anchors.centerIn: parent
        anchors.horizontalCenterOffset: Math.cos(child.angle) * Style.space(3)
        anchors.verticalCenterOffset: Math.sin(child.angle) * Style.space(3)
        iconName: String(child.applicationIcon.icon || "")
        trustedIconSource: child.applicationIcon.icon_source
        monochrome: !child.active || monochromeMask !== "alpha"
        monochromeMask: String(child.modelData.icon_mask || "alpha")
        iconColor: Qt.lighter(Color.accent, child.active ? 1.75 : 1.45)
        iconSize: Typography.body + 7
        fallbackGlyph: String(child.applicationIcon.glyph || "") || String(child.modelData.key || "").toUpperCase()
        fallbackColor: Qt.lighter(Color.accent, 1.45)
        fallbackSize: Typography.body + 4
      }

      Text {
        visible: !child.third && overlay.controller.childrenOf(child.modelData).length > 0
        textFormat: Text.PlainText
        readonly property point at: overlay.pointOnRing(child.angle, overlay.controller.childOuterRadius - Style.space(5))
        x: at.x - child.x - width / 2
        y: at.y - child.y - height / 2
        rotation: child.angle * 180 / Math.PI
        text: "󰅂"
        color: Color.popups.text
        font.family: Style.font.family
        font.pixelSize: Typography.bodySmall
      }

      WheelKeyLetter {
        letter: String(child.modelData.key || "")
        active: child.active
        at: Qt.point(child.keyPoint.x - child.x, child.keyPoint.y - child.y)
      }
    }
  }

  Column {
    id: hub
    visible: overlay.wheelHere
    width: overlay.controller.hubRadius * 2 - Style.space(6)
    x: overlay.controller.wheelX - width / 2
    y: overlay.controller.wheelY - implicitHeight / 2
    spacing: 0

    Text {
      textFormat: Text.PlainText
      width: parent.width
      horizontalAlignment: Text.AlignHCenter
      text: overlay.controller.loading ? "…" : String(overlay.controller.count)
      color: Color.accent
      font.family: Style.font.family
      font.pixelSize: Typography.body + 2
      font.weight: Font.Bold
    }

    Text {
      textFormat: Text.PlainText
      width: parent.width
      horizontalAlignment: Text.AlignHCenter
      text: overlay.controller.count === 1 ? "item" : "items"
      color: Color.muted
      font.family: Style.font.family
      font.pixelSize: Typography.caption
    }
  }

  Rectangle {
    id: caption
    visible: overlay.wheelHere || overlay.toastHere
    width: captionColumn.implicitWidth + Style.space(20)
    height: captionColumn.implicitHeight + Style.space(10)
    x: Math.max(Style.space(4), Math.min(overlay.screenWidth - width - Style.space(4), overlay.controller.wheelX - width / 2))
    y: Math.min(overlay.screenHeight - height - Style.space(4), overlay.controller.wheelY + (overlay.wheelHere ? overlay.controller.extent + Style.space(10) : Style.space(18)))
    radius: Style.cornerRadius
    color: Color.popups.background
    border.width: 1
    readonly property bool failed: overlay.wheelHere ? overlay.controller.error !== "" : overlay.controller.toastError
    border.color: failed ? Color.urgent : Color.popups.border

    readonly property var highlightedItem: overlay.controller.subHighlighted >= 0
      ? overlay.controller.subItems[overlay.controller.subHighlighted]
      : overlay.controller.outerHighlighted >= 0
      ? overlay.controller.outerItems[overlay.controller.outerHighlighted]
      : overlay.controller.highlighted >= 0 ? overlay.controller.ringItems[overlay.controller.highlighted] : null
    readonly property string headline: !overlay.wheelHere
      ? overlay.controller.toast
      : overlay.controller.error !== ""
        ? overlay.controller.error
        : overlay.controller.status !== ""
          ? overlay.controller.status
          : overlay.controller.loading
            ? "Reading the window under the pointer…"
            : highlightedItem ? String(highlightedItem.label || "") + "   " + String(highlightedItem.key || "") : overlay.controller.ringTitle
    readonly property string detail: !overlay.wheelHere
      ? ""
      : highlightedItem && overlay.controller.error === ""
        ? String(highlightedItem.description || "")
        : overlay.controller.outerFocus
          ? "letters pick  ·  Esc goes back"
          : (overlay.controller.targetLabel !== "" ? "onto " + overlay.controller.targetLabel + "  ·  " : "") + "letters pick  ·  Esc closes"

    Column {
      id: captionColumn
      anchors.centerIn: parent
      spacing: Style.space(2)

      Text {
        textFormat: Text.PlainText
        text: caption.headline
        color: caption.failed ? Color.urgent : Color.popups.text
        font.family: Style.font.family
        font.pixelSize: Typography.bodySmall
        font.weight: Font.DemiBold
      }

      Text {
        textFormat: Text.PlainText
        visible: text !== ""
        text: caption.detail
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Typography.caption
      }

      Text {
        visible: overlay.wheelHere && text !== ""
        textFormat: Text.PlainText
        text: overlay.controller.diagnostics
        width: Math.min(implicitWidth, Style.space(520))
        wrapMode: Text.Wrap
        maximumLineCount: 3
        elide: Text.ElideRight
        color: Color.urgent
        font.family: Style.font.family
        font.pixelSize: Typography.caption
      }
    }
  }
}
