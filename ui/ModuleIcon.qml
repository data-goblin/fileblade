import QtQuick
import qs.Commons
import "../theme"
import qs.Commons as Commons

Item {
  id: icon

  property string iconUrl: ""
  property string glyph: ""
  property string fallbackGlyph: "󰏗"
  property color color: Commons.Color.muted
  property real size: Typography.body
  readonly property bool pictorial: iconUrl !== ""

  implicitWidth: Math.round(size * 1.15)
  implicitHeight: implicitWidth

  Text {
    textFormat: Text.PlainText
    anchors.centerIn: parent
    visible: !icon.pictorial || picture.status === Image.Error
    text: icon.glyph !== "" ? icon.glyph : icon.fallbackGlyph
    color: icon.color
    font.family: Style.font.family
    font.pixelSize: icon.size
  }

  Image {
    id: picture
    anchors.fill: parent
    visible: false
    source: icon.pictorial ? icon.iconUrl : ""
    asynchronous: true
    smooth: true
    fillMode: Image.PreserveAspectFit
    sourceSize.width: Math.max(1, Math.round(width * 2))
    sourceSize.height: Math.max(1, Math.round(height * 2))
  }

  ShaderEffect {
    anchors.fill: parent
    visible: icon.pictorial && picture.status === Image.Ready
    property var source: picture
    property color glyphColor: icon.color
    fragmentShader: Qt.resolvedUrl("shaders/AlphaGlyph.frag.qsb")
  }
}
