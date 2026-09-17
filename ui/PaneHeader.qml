import QtQuick
import qs.Commons
import "../theme"

Rectangle {
  id: header

  property var context: null
  property var view: null
  property string title: ""
  property string status: ""
  property string statusGlyph: ""
  property color statusColor: Color.muted
  property bool highlighted: !!context && context.slotFocused === true
  property int tabIndex: -1
  property int reservedLeft: 0
  property int reservedRight: 0
  property int preferredHeight: Style.space(32)
  property bool showIdentity: true
  property bool showNavigation: true
  property bool extendAddGuide: true
  property var widthFor: function(key) { return -1 }
  property var titleActions: []
  signal titleTriggered(string key)

  readonly property bool handleHovered: handle.containsMouse
  readonly property bool collapsible: context && context.host && context.slotIndex >= 0
  readonly property bool integrated: context && context.tabCount > 1
  readonly property bool identityVisible: showIdentity && !integrated
  readonly property int effectiveReservedLeft: reservedLeft
  readonly property int effectiveReservedRight: reservedRight
  readonly property real leadingRight: navigation.visible
    ? navigation.x + navigation.naturalWidth
    : (moduleAdd.visible ? moduleAdd.x + moduleAdd.width
        : (label.visible ? label.x + label.width : Style.space(7) + effectiveReservedLeft))
  readonly property real extrasRight: width - Style.space(7) - effectiveReservedRight
  readonly property real nonColumnExtrasWidth: extensionRow.visible
    ? extensionRow.implicitWidth + extrasRow.spacing
    : 0
  readonly property real columnSpace: Math.max(0, extrasRight - leadingRight - Style.space(8) - nonColumnExtrasWidth)
  readonly property bool columnsCollapsed: metricPicker.implicitWidth > 0
    && metricPicker.implicitWidth > columnSpace
  readonly property bool addGuideVisible: metricPicker.addGuideVisible
  readonly property real addGuideX: extrasRow.x + metricPicker.x + metricPicker.addGuideX
  readonly property real addSlotWidth: metricPicker.addSlotWidth
  default property alias extras: extensionRow.data

  function openFilter() {
    metricPicker.openFilter()
  }

  function syncColumnGeometry() {
    if (visible && view && view.columnRightReserve !== undefined)
      view.columnRightReserve = effectiveReservedRight
  }

  anchors.left: parent ? parent.left : undefined
  anchors.right: parent ? parent.right : undefined
  height: visible ? preferredHeight : 0
  color: Qt.lighter(Color.bar.background, 1.035)

  onViewChanged: syncColumnGeometry()
  onEffectiveReservedRightChanged: syncColumnGeometry()
  onVisibleChanged: syncColumnGeometry()
  Component.onCompleted: syncColumnGeometry()

  Binding {
    target: header.parent
    property: "z"
    value: 25
    when: header.extendAddGuide && header.addGuideVisible && !!header.parent
  }

  Text {
    id: disclosure
    textFormat: Text.PlainText
    anchors.left: parent.left
    anchors.leftMargin: Style.space(7) + header.effectiveReservedLeft
    anchors.verticalCenter: parent.verticalCenter
    visible: header.collapsible && header.identityVisible
    text: header.context && header.context.collapsed ? "›" : "⌄"
    color: disclosurePointer.containsMouse ? Color.accent : Color.muted
    font.family: Style.font.family
    font.pixelSize: Typography.body

    MouseArea {
      id: disclosurePointer
      anchors.fill: parent
      anchors.margins: -Style.space(4)
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: if (header.context) header.context.toggleCollapsed()
    }
  }

  Text {
    textFormat: Text.PlainText
    id: label
    anchors.left: disclosure.visible ? disclosure.right : parent.left
    anchors.leftMargin: disclosure.visible ? Style.space(5) : Style.space(10) + header.effectiveReservedLeft
    anchors.verticalCenter: parent.verticalCenter
    visible: header.identityVisible
    text: header.title.toUpperCase()
    color: header.highlighted ? Color.accent : (handle.containsMouse ? Color.bar.text : Color.muted)
    font.family: Style.font.family
    font.pixelSize: Typography.bodySmall
    font.weight: Font.DemiBold
    font.letterSpacing: 0.6
  }

  PaneNavigation {
    id: moduleAdd
    anchors.left: label.visible ? label.right : parent.left
    anchors.leftMargin: label.visible ? Style.space(8) : Style.space(7) + header.effectiveReservedLeft
    anchors.verticalCenter: parent.verticalCenter
    height: Style.space(24)
    width: implicitWidth
    visible: header.identityVisible && header.collapsible
    actions: [{ key: "add-module", glyph: "+", title: "Add module", actions: [{ button: "left", text: "Choose a module for this section" }] }]
    onTriggered: if (header.context) header.context.openModulePicker(moduleAdd.x)
  }

  PaneNavigation {
    id: navigation
    anchors.left: moduleAdd.visible ? moduleAdd.right : (label.visible ? label.right : parent.left)
    anchors.leftMargin: moduleAdd.visible ? Style.space(2)
      : (label.visible ? Style.space(8) : Style.space(7) + header.effectiveReservedLeft)
    anchors.verticalCenter: parent.verticalCenter
    height: Style.space(24)
    width: implicitWidth
    maximumWidth: header.columnsCollapsed
      ? Math.max(0, header.extrasRight - Style.space(20) - header.nonColumnExtrasWidth
          - Style.space(8) - x)
      : -1
    actions: header.view && Array.isArray(header.view.navigationActions)
      ? header.view.navigationActions
      : (Array.isArray(header.titleActions) ? header.titleActions : [])
    visible: header.showNavigation && actions.length > 0 && (!!header.view || header.identityVisible)
    onTriggered: function(key) {
      if (header.view) header.view.navigationTriggered(key)
      else header.titleTriggered(key)
    }
  }

  MouseArea {
    id: handle
    anchors.fill: label
    anchors.margins: -Style.space(5)
    visible: label.visible
    hoverEnabled: true
    acceptedButtons: Qt.LeftButton
    cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
    onPressed: function(mouse) { if (header.context) header.context.handlePressed(handle, mouse.x, mouse.y, header.tabIndex) }
    onPositionChanged: function(mouse) { if (header.context && (mouse.buttons & Qt.LeftButton)) header.context.handleMoved(handle, mouse.x, mouse.y) }
    onReleased: function(mouse) { if (header.context) header.context.handleReleased(handle, mouse.x, mouse.y) }
    onCanceled: if (header.context) header.context.handleCanceled()
  }

  Row {
    id: extrasRow
    anchors.right: parent.right
    anchors.rightMargin: Style.space(7) + header.effectiveReservedRight
    anchors.verticalCenter: parent.verticalCenter
    spacing: Style.space(2)

    ColumnControls {
      id: metricPicker
      view: header.view
      widthFor: header.widthFor
      visible: !header.columnsCollapsed
    }

    Text {
      textFormat: Text.PlainText
      width: Style.space(20)
      height: Style.space(24)
      visible: header.columnsCollapsed
      text: "..."
      color: Color.muted
      horizontalAlignment: Text.AlignHCenter
      verticalAlignment: Text.AlignVCenter
      font.family: Style.font.family
      font.pixelSize: Typography.caption
      font.weight: Font.DemiBold
    }

    Row {
      id: extensionRow
      height: Style.space(24)
      spacing: Style.space(2)
      visible: children.length > 0
    }
  }

  Text {
    textFormat: Text.PlainText
    anchors.left: navigation.visible ? navigation.right : (moduleAdd.visible ? moduleAdd.right : (label.visible ? label.right : parent.left))
    anchors.leftMargin: navigation.visible || moduleAdd.visible || label.visible
      ? Style.space(10)
      : Style.space(10) + header.effectiveReservedLeft
    anchors.right: extrasRow.left
    anchors.rightMargin: Style.space(6)
    anchors.verticalCenter: parent.verticalCenter
    horizontalAlignment: Text.AlignRight
    elide: Text.ElideLeft
    text: (header.statusGlyph !== "" ? header.statusGlyph + " " : "") + header.status
    color: header.statusColor
    font.family: Style.font.family
    font.pixelSize: Typography.caption
  }

  Rectangle {
    x: Math.round(header.addGuideX - width / 2)
    y: header.height
    width: Style.space(2)
    height: Math.max(0, header.extendAddGuide && header.parent && header.parent.parent
      ? header.parent.parent.height - header.parent.y - header.y - header.height
      : 0)
    radius: width / 2
    visible: header.addGuideVisible
    color: Util.alpha(Color.accent, 0.7)
    z: 20
  }

}
