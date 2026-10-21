import QtQuick
import qs.Commons
import "../theme"
import qs.Commons as Commons

Item {
  id: row

  property string label: ""
  property string glyph: ""
  property string heading: ""
  property string prompt: "Type to filter, Enter picks"
  property var options: []
  property string value: ""
  readonly property string currentLabel: {
    for (var i = 0; i < options.length; i++) if (String(options[i].key) === value) return String(options[i].label)
    return value
  }

  signal chosen(string key)

  implicitHeight: Style.space(28)

  function open() {
    menu.rows = options.map(function(option) { return { key: String(option.key), label: String(option.label), checked: String(option.key) === row.value } })
    menu.present()
  }

  Text {
    textFormat: Text.PlainText
    anchors.left: parent.left
    anchors.verticalCenter: parent.verticalCenter
    anchors.right: current.left
    anchors.rightMargin: Style.space(8)
    text: (row.glyph !== "" ? row.glyph + "  " : "") + row.label
    color: Commons.Color.bar.text
    elide: Text.ElideRight
    font.family: Style.font.family
    font.pixelSize: Typography.bodySmall
  }

  Rectangle {
    id: current
    anchors.right: parent.right
    anchors.verticalCenter: parent.verticalCenter
    width: currentLabelText.implicitWidth + pointer.glyphWidth + Style.space(18)
    height: Style.space(22)
    radius: Style.space(4)
    color: pointer.containsMouse || menu.visible ? Util.alpha(Commons.Color.accent, 0.22) : Util.alpha(Commons.Color.bar.text, 0.1)

    Text {
      id: currentLabelText
      textFormat: Text.PlainText
      anchors.left: parent.left
      anchors.leftMargin: Style.space(7)
      anchors.verticalCenter: parent.verticalCenter
      text: row.currentLabel
      color: Commons.Color.bar.text
      font.family: Style.font.family
      font.pixelSize: Typography.caption
    }

    ChevronHit {
      id: pointer
      onClicked: row.open()
    }
  }

  OptionPopup {
    id: menu
    x: Math.max(0, row.width - menuWidth)
    y: row.height + Style.space(2)
    menuWidth: Style.space(220)
    heading: row.heading !== "" ? row.heading : row.label
    prompt: row.prompt
    onPicked: function(key) { row.chosen(key) }
  }
}
