import QtQuick
import qs.Commons
import "../theme"

Item {
  id: grid

  property var actions: []
  property int current: -1
  property bool showCurrent: false
  property real spacing: Style.space(10)
  property real rowSpacing: Style.space(2)
  property real minimumCellWidth: Style.space(130)
  readonly property int columns: actions.length > 1 && width >= minimumCellWidth * 2 + spacing ? 2 : 1
  readonly property real cellWidth: columns > 1 ? Math.floor((width - spacing) / 2) : width
  readonly property real cellHeight: Style.space(26)
  readonly property real iconWidth: Style.space(20)
  readonly property int rows: Math.ceil(actions.length / columns)

  signal activated(int index)

  implicitHeight: rows > 0 ? rows * cellHeight + (rows - 1) * rowSpacing : 0
  height: implicitHeight

  function itemAt(index) {
    return cells.itemAt(index)
  }

  Grid {
    columns: grid.columns
    columnSpacing: grid.spacing
    rowSpacing: grid.rowSpacing

    Repeater {
      id: cells
      model: grid.actions.length
      delegate: Item {
        id: cell
        required property int index
        readonly property var modelData: cell.index < grid.actions.length && grid.actions[cell.index] ? grid.actions[cell.index] : ({})
        readonly property bool current: grid.showCurrent && grid.current === index
        readonly property bool hot: pointer.containsMouse
        readonly property color tone: modelData.urgent ? Color.urgent : Color.accent
        readonly property color lit: pointer.pressed ? Util.alpha(cell.tone, 0.7) : cell.tone
        readonly property bool raised: cell.current || cell.hot || pointer.pressed
        readonly property var tipActions: Array.isArray(modelData.tipActions) ? modelData.tipActions : []

        width: grid.cellWidth
        height: grid.cellHeight

        Text {
          id: icon
          width: grid.iconWidth
          anchors.verticalCenter: parent.verticalCenter
          horizontalAlignment: Text.AlignHCenter
          textFormat: Text.PlainText
          text: String(cell.modelData.glyph || "")
          color: cell.raised ? cell.lit : cell.modelData.urgent ? Color.urgent : Color.muted
          font.family: cell.modelData.glyphFamily ? String(cell.modelData.glyphFamily) : Style.font.family
          font.pixelSize: Typography.body
        }

        Text {
          id: label
          anchors.left: icon.right
          anchors.leftMargin: Style.space(6)
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          textFormat: Text.PlainText
          text: String(cell.modelData.text || "")
          color: cell.raised ? cell.lit : Color.bar.text
          elide: Text.ElideRight
          maximumLineCount: 1
          font.family: Style.font.family
          font.pixelSize: Typography.bodySmall
          font.weight: cell.current ? Font.DemiBold : Font.Normal
        }

        MouseArea {
          id: pointer
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: grid.activated(cell.index)
        }

        HintTip {
          visible: pointer.containsMouse && title !== ""
          title: String(cell.modelData.tip || cell.modelData.text || "")
          actions: cell.tipActions
        }
      }
    }
  }
}
