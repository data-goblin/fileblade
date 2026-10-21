import QtQuick
import qs.Commons
import "../ui" as PluginUi
import qs.Commons as Commons

Item {
  id: drivesSection

  required property var controller
  required property var pane
  readonly property var drives: controller.drivesController
  readonly property int rowHeight: Style.space(28)
  readonly property int visibleRows: Math.min(5, drives ? drives.count : 0)
  readonly property int noticeHeight: notice.visible ? notice.implicitHeight + Style.space(10) : 0
  height: visibleRows * rowHeight + noticeHeight
  visible: height > 0

  function badgeFor(row) {
    if (drives.busySource === row.source) return "…"
    return row.sizeLabel
  }

  ListView {
    id: drivesList
    anchors.top: parent.top
    anchors.left: parent.left
    anchors.right: parent.right
    height: drivesSection.visibleRows * drivesSection.rowHeight
    reuseItems: true
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    model: drivesSection.drives ? drivesSection.drives.model : null
    currentIndex: -1

    delegate: PluginUi.PaneRow {
      id: driveRow
      required property int index
      required property string name
      required property string source
      required property string mountpoint
      required property bool mounted
      required property bool external
      required property bool readOnly
      required property bool needsAuthorization
      required property string sizeLabel
      required property real usedFraction
      required property string volumeGlyph
      readonly property var action: drivesSection.drives.actionFor(driveRow)
      readonly property bool busy: drivesSection.drives.busySource === driveRow.source

      width: drivesList.width
      label: driveRow.name
      glyph: driveRow.volumeGlyph
      glyphColor: driveRow.mounted ? Commons.Color.accent : Commons.Color.muted
      labelColor: driveRow.mounted ? Commons.Color.bar.text : Util.alpha(Commons.Color.bar.text, 0.75)
      badge: drivesSection.badgeFor(driveRow)
      barColumn: driveRow.mounted
      barFraction: driveRow.mounted ? driveRow.usedFraction : -1
      columnWidths: [Style.space(96)]
      valueSample: "999 GB"
      hovered: rowHover.hovered
      actionsVisible: drivesSection.drives.actionAvailableFor(driveRow.source)
      actionsReserved: true

      HoverHandler {
        id: rowHover
        blocking: false
      }

      TapHandler {
        acceptedButtons: Qt.LeftButton
        onTapped: drivesSection.drives.openVolume(driveRow.source, drivesSection.pane.hostWindow)
      }

      actions: PluginUi.PaneCorner {
        glyph: driveRow.action.glyph
        tip: driveRow.action.tip
        tipActions: driveRow.action.actions || []
        tipContext: driveRow.action.context || []
        active: driveRow.busy
        enabled: !driveRow.busy
        onActivated: drivesSection.drives.runAction(driveRow.source)
      }
    }
  }

  PluginUi.ErrorNotice {
    id: notice
    anchors.top: drivesList.bottom
    anchors.topMargin: Style.space(4)
    anchors.left: parent.left
    anchors.leftMargin: Style.space(9)
    anchors.right: parent.right
    anchors.rightMargin: Style.space(9)
    text: drivesSection.drives ? drivesSection.drives.error : ""
    onDismissed: drivesSection.drives.clearError()
  }

  Rectangle {
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.bottom: parent.bottom
    height: 1
    color: Util.alpha(Commons.Color.bar.text, 0.08)
  }
}
