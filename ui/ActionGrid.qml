import QtQuick
import qs.Commons
import "../theme"

Item {
  id: grid

  property var actions: []
  property int current: -1
  property bool showCurrent: false
  property real spacing: Style.space(6)
  property real minimumCellWidth: Style.space(130)
  readonly property int columns: actions.length > 1 && width >= minimumCellWidth * 2 + spacing ? 2 : 1
  readonly property real cellWidth: columns > 1 ? Math.floor((width - spacing) / 2) : width
  readonly property real cellHeight: Style.space(30)
  readonly property int rows: Math.ceil(actions.length / columns)

  signal activated(int index)

  implicitHeight: rows > 0 ? rows * cellHeight + (rows - 1) * spacing : 0
  height: implicitHeight

  function itemAt(index) {
    return cells.itemAt(index)
  }

  Grid {
    columns: grid.columns
    columnSpacing: grid.spacing
    rowSpacing: grid.spacing

    Repeater {
      id: cells
      model: grid.actions
      delegate: Rectangle {
        id: cell
        required property var modelData
        required property int index
        readonly property bool current: grid.showCurrent && grid.current === index
        readonly property bool hot: pointer.containsMouse
        readonly property color tone: modelData.urgent ? Color.urgent : Color.accent
        readonly property var tipActions: Array.isArray(modelData.tipActions) ? modelData.tipActions : []

        width: grid.cellWidth
        height: grid.cellHeight
        color: pointer.pressed ? Util.alpha(cell.tone, 0.24)
          : cell.current ? Util.alpha(cell.tone, 0.12)
          : cell.hot ? Style.hoverFillFor(Color.bar.text, cell.tone)
          : Util.alpha(Color.bar.text, 0.05)
        border.width: 1
        border.color: cell.current ? cell.tone
          : cell.hot ? Util.alpha(cell.tone, 0.5)
          : Util.alpha(Color.bar.text, 0.14)

        Rectangle {
          id: well
          x: 1
          y: 1
          width: grid.cellHeight - 2
          height: parent.height - 2
          color: cell.current ? cell.tone : Util.alpha(cell.tone, cell.hot || pointer.pressed ? 0.22 : 0.10)

          Text {
            anchors.centerIn: parent
            textFormat: Text.PlainText
            text: String(cell.modelData.glyph || "")
            color: cell.current ? Color.background : cell.tone
            font.family: cell.modelData.glyphFamily ? String(cell.modelData.glyphFamily) : Style.font.family
            font.pixelSize: Typography.body
          }
        }

        Text {
          id: label
          anchors.left: well.right
          anchors.leftMargin: Style.space(8)
          anchors.right: parent.right
          anchors.rightMargin: Style.space(8)
          anchors.verticalCenter: parent.verticalCenter
          textFormat: Text.PlainText
          text: String(cell.modelData.text || "")
          color: cell.current ? cell.tone : Color.bar.text
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
          visible: pointer.containsMouse && (label.truncated || cell.tipActions.length > 0 || !!cell.modelData.tip)
          title: String(cell.modelData.tip || cell.modelData.text || "")
          actions: cell.tipActions
        }
      }
    }
  }
}
