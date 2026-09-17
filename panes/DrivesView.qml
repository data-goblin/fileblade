import QtQuick
import qs.Commons
import "../ui" as PluginUi
import "../theme"

FocusScope {
  id: root

  required property var controller
  required property var pane
  property alias list: drivesList
  readonly property var drives: controller.drivesController

  function focusList() {
    if (drivesList.count > 0 && drivesList.currentIndex < 0) drivesList.currentIndex = 0
    drivesList.forceActiveFocus()
  }

  function currentRow() {
    return drivesList.currentIndex >= 0 ? drivesList.model.get(drivesList.currentIndex) : null
  }

  function openCurrent() {
    var row = currentRow()
    if (row) drives.openVolume(String(row.source), pane.hostWindow)
  }

  function actOnCurrent() {
    var row = currentRow()
    if (row) drives.runAction(String(row.source))
  }

  function moveTo(target) {
    if (drivesList.count === 0) return
    drivesList.currentIndex = Math.max(0, Math.min(drivesList.count - 1, target))
  }

  PluginUi.ErrorNotice {
    id: notice
    anchors.top: parent.top
    anchors.topMargin: visible ? Style.space(6) : 0
    anchors.left: parent.left
    anchors.leftMargin: Style.space(9)
    anchors.right: parent.right
    anchors.rightMargin: Style.space(9)
    text: root.drives ? root.drives.error : ""
    onDismissed: root.drives.clearError()
  }

  ListView {
    id: drivesList
    anchors.top: notice.bottom
    anchors.topMargin: notice.visible ? Style.space(6) : 0
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.bottom: parent.bottom
    clip: true
    reuseItems: true
    boundsBehavior: Flickable.StopAtBounds
    model: root.drives ? root.drives.allModel : null
    currentIndex: -1
    section.property: "tier"
    section.criteria: ViewSection.FullString
    section.delegate: Item {
      required property string section
      width: drivesList.width
      height: Style.space(26)

      Text {
        textFormat: Text.PlainText
        anchors.left: parent.left
        anchors.leftMargin: Style.space(9)
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Style.space(4)
        text: root.drives.tierLabel(parent.section)
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Typography.caption
        font.weight: Font.DemiBold
        font.letterSpacing: 0.6
      }
    }

    onCountChanged: if (count === 0) currentIndex = -1
    else if (currentIndex >= count) currentIndex = count - 1

    delegate: PluginUi.PaneRow {
      id: driveRow
      required property int index
      required property string name
      required property string source
      required property string mountpoint
      required property string filesystem
      required property bool mounted
      required property bool external
      required property bool readOnly
      required property bool needsAuthorization
      required property string sizeLabel
      required property real usedFraction
      required property string volumeGlyph
      readonly property var action: root.drives.actionFor(driveRow)
      readonly property bool busy: root.drives.busySource === driveRow.source

      width: drivesList.width
      label: driveRow.name
      detail: driveRow.filesystem + "  " + (driveRow.mounted ? driveRow.mountpoint : driveRow.source)
      glyph: driveRow.volumeGlyph
      glyphColor: driveRow.mounted ? Color.accent : Color.muted
      labelColor: driveRow.mounted ? Color.bar.text : Util.alpha(Color.bar.text, 0.75)
      badge: driveRow.busy ? "…" : driveRow.sizeLabel
      barColumn: driveRow.mounted
      barFraction: driveRow.mounted ? driveRow.usedFraction : -1
      columnWidths: [Style.space(96)]
      valueSample: "999 GB"
      current: driveRow.ListView.isCurrentItem
      hovered: rowHover.hovered
      actionsVisible: root.drives.actionsAvailable
      actionsReserved: true

      HoverHandler {
        id: rowHover
        blocking: false
      }

      TapHandler {
        acceptedButtons: Qt.LeftButton
        onTapped: {
          drivesList.currentIndex = driveRow.index
          drivesList.forceActiveFocus()
          root.drives.openVolume(driveRow.source, root.pane.hostWindow)
        }
      }

      actions: PluginUi.PaneCorner {
        glyph: driveRow.action.glyph
        tip: driveRow.action.tip
        tipActions: driveRow.action.actions || []
        tipContext: driveRow.action.context || []
        active: driveRow.busy
        enabled: !driveRow.busy
        onActivated: root.drives.runAction(driveRow.source)
      }
    }

    Keys.onUpPressed: function(event) {
      root.moveTo(currentIndex < 0 ? 0 : currentIndex - 1)
      event.accepted = true
    }
    Keys.onDownPressed: function(event) {
      root.moveTo(currentIndex < 0 ? 0 : currentIndex + 1)
      event.accepted = true
    }
    Keys.onReturnPressed: function(event) {
      root.openCurrent()
      event.accepted = true
    }
    Keys.onDeletePressed: function(event) {
      root.actOnCurrent()
      event.accepted = true
    }
    Keys.onEscapePressed: function(event) {
      controller.goBack()
      event.accepted = true
    }
    Keys.onPressed: function(event) {
      if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier)) return
      if (event.key === Qt.Key_J) root.moveTo(currentIndex < 0 ? 0 : currentIndex + 1)
      else if (event.key === Qt.Key_K) root.moveTo(currentIndex < 0 ? 0 : currentIndex - 1)
      else if (event.key === Qt.Key_G) root.moveTo((event.modifiers & Qt.ShiftModifier) ? count - 1 : 0)
      else if (event.key === Qt.Key_E) root.actOnCurrent()
      else return
      event.accepted = true
    }
  }

  Text {
    textFormat: Text.PlainText
    anchors.centerIn: parent
    width: Math.max(0, parent.width - Style.space(32))
    visible: drivesList.count === 0
    text: root.drives && root.drives.error ? root.drives.error : "No volumes found"
    color: root.drives && root.drives.error ? Color.urgent : Color.muted
    horizontalAlignment: Text.AlignHCenter
    wrapMode: Text.WordWrap
    font.family: Style.font.family
    font.pixelSize: Typography.body
  }
}
