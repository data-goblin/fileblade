import QtQuick
import qs.Commons
import "../theme"

Item {
  id: row

  property string label: ""
  property string glyph: ""
  property var options: []
  property string value: ""
  property real gap: Style.space(12)
  readonly property bool stacked: title.implicitWidth + Style.space(16) + measure.implicitWidth > width

  signal chosen(string key)

  implicitHeight: stacked ? title.implicitHeight + Style.space(6) + choices.implicitHeight + Style.space(6) : Style.space(26)

  Text {
    id: title
    textFormat: Text.PlainText
    anchors.left: parent.left
    anchors.top: row.stacked ? parent.top : undefined
    anchors.topMargin: row.stacked ? Style.space(4) : 0
    anchors.verticalCenter: row.stacked ? undefined : parent.verticalCenter
    text: (row.glyph !== "" ? row.glyph + "  " : "") + row.label
    color: Color.bar.text
    font.family: Style.font.family
    font.pixelSize: Typography.bodySmall
  }

  Row {
    id: measure
    visible: false
    spacing: row.gap

    Repeater {
      model: row.options

      delegate: Text {
        required property var modelData
        textFormat: Text.PlainText
        text: String(modelData.label !== undefined ? modelData.label : modelData)
        font.family: Style.font.family
        font.pixelSize: Typography.caption
        font.weight: Font.DemiBold
      }
    }
  }

  Component {
    id: choice

    Text {
      id: option
      required property var modelData
      readonly property string key: String(modelData.key !== undefined ? modelData.key : modelData)
      readonly property bool active: key === row.value
      textFormat: Text.PlainText
      text: String(modelData.label !== undefined ? modelData.label : modelData)
      color: active ? Color.accent : (optionPointer.containsMouse ? Color.bar.text : Color.muted)
      font.family: Style.font.family
      font.pixelSize: Typography.caption
      font.weight: active ? Font.DemiBold : Font.Normal

      MouseArea {
        id: optionPointer
        anchors.fill: parent
        anchors.margins: -Style.space(4)
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: row.chosen(option.key)
      }
    }
  }

  Row {
    id: inline
    anchors.right: parent.right
    anchors.verticalCenter: parent.verticalCenter
    visible: !row.stacked
    spacing: row.gap

    Repeater {
      model: row.stacked ? [] : row.options
      delegate: choice
    }
  }

  Flow {
    id: choices
    anchors.left: parent.left
    anchors.leftMargin: Style.space(22)
    anchors.right: parent.right
    anchors.top: title.bottom
    anchors.topMargin: Style.space(6)
    visible: row.stacked
    spacing: row.gap

    Repeater {
      model: row.stacked ? row.options : []
      delegate: choice
    }
  }
}
