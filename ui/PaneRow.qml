import QtQuick
import qs.Commons
import "../theme"

Rectangle {
  id: row

  property string label: ""
  property string detail: ""
  property string badge: ""
  property string glyph: ""
  property bool glyphStruck: false
  property string expander: ""
  property bool expanderReserved: false
  property color glyphColor: Color.muted
  property color labelColor: Color.bar.text
  property int indent: 0
  property real barFraction: -1
  property bool barColumn: false
  property bool linked: false
  property bool linkOnRight: true
  property bool struck: false
  property int barWidth: Style.space(40)
  property int barGap: Style.space(4)
  property string valueSample: "0.00k"
  property int valueWidth: Math.ceil(Math.max(valueMetrics.width, badgeMetrics.width))
  readonly property real contentOpacity: struck ? 0.5 : 1
  property Component trailing: null
  property Component actions: null
  property var extras: []
  property var columnWidths: []
  property int columnSpacing: Style.space(2)
  property int metricRightMargin: Style.space(7)
  property int actionsRightMargin: Style.space(7)
  property bool actionsVisible: false
  property bool actionsReserved: false
  property int actionsGap: Style.space(4)
  property bool current: false
  property bool focused: false
  property bool hovered: false
  property bool emphasized: false
  readonly property bool barVisible: barFraction >= 0
  readonly property bool linkHovered: row.linkOnRight ? linkHover.hovered : glyphHover.hovered
  readonly property var linkAnchor: row.linkOnRight ? linkGlyph : rowGlyph

  function metricText(index) {
    var value = index === 0 ? badge : extras[index - 1]
    return String(value === undefined || value === null ? "" : value)
  }

  height: Style.space(30)

  TextMetrics {
    id: valueMetrics
    font.family: Style.font.family
    font.pixelSize: Typography.caption
    text: row.valueSample
  }

  TextMetrics {
    id: badgeMetrics
    font.family: Style.font.family
    font.pixelSize: Typography.caption
    text: row.barColumn ? row.badge : ""
  }
  color: current
    ? Color.menu.selectedBackground
    : hovered
      ? Style.hoverFillFor(Color.bar.text, Color.accent)
      : emphasized ? Util.alpha(Color.bar.text, 0.025) : "transparent"

  Text {
    textFormat: Text.PlainText
    id: disclosureGlyph
    anchors.left: parent.left
    anchors.leftMargin: Style.space(9) + row.indent
    anchors.verticalCenter: parent.verticalCenter
    width: Style.space(10)
    visible: row.expander !== ""
    text: row.expander
    color: Color.muted
    opacity: row.contentOpacity
    horizontalAlignment: Text.AlignHCenter
    font.family: Style.font.family
    font.pixelSize: Typography.body
  }

  Text {
    textFormat: Text.PlainText
    id: rowGlyph
    anchors.left: parent.left
    anchors.leftMargin: Style.space(9) + row.indent + (disclosureGlyph.visible || row.expanderReserved ? Style.space(11) : 0)
    anchors.verticalCenter: parent.verticalCenter
    width: Style.space(14)
    visible: row.glyph !== ""
    text: row.glyph
    color: row.glyphColor
    opacity: row.contentOpacity
    horizontalAlignment: Text.AlignHCenter
    font.family: Style.font.family
    font.pixelSize: Typography.caption

    HoverHandler {
      id: glyphHover
      enabled: row.linked && !row.linkOnRight
      blocking: false
    }
  }

  Rectangle {
    visible: row.glyphStruck && rowGlyph.visible
    anchors.centerIn: rowGlyph
    width: rowGlyph.width
    height: 1.5
    rotation: -45
    radius: 1
    color: row.glyphColor
    opacity: row.contentOpacity
  }

  Text {
    textFormat: Text.PlainText
    id: rowLabel
    readonly property real limit: (metricRow.visible ? metricRow.x : row.width) - x - Style.space(6)
      - (linkGlyph.visible ? linkGlyph.width + Style.space(5) : 0)
    anchors.left: rowGlyph.right
    anchors.leftMargin: Style.space(6)
    anchors.verticalCenter: parent.verticalCenter
    width: Math.max(0, Math.min(implicitWidth, limit))
    text: row.label
    color: row.labelColor
    opacity: row.contentOpacity
    elide: Text.ElideRight
    font.family: Style.font.family
    font.pixelSize: row.emphasized ? Typography.bodySmall : Typography.body
    font.weight: row.emphasized ? Font.DemiBold : Font.Normal
    font.letterSpacing: row.emphasized ? 0.6 : 0
    font.strikeout: row.struck
  }

  Text {
    textFormat: Text.PlainText
    id: rowDetail
    readonly property real limit: (metricRow.visible ? metricRow.x : row.width) - x - Style.space(6)
    anchors.left: linkGlyph.visible ? linkGlyph.right : rowLabel.right
    anchors.leftMargin: Style.space(8)
    anchors.verticalCenter: parent.verticalCenter
    width: Math.max(0, Math.min(implicitWidth, limit))
    visible: row.detail !== ""
    text: row.detail
    color: Color.muted
    opacity: row.contentOpacity
    elide: Text.ElideLeft
    font.family: Style.font.family
    font.pixelSize: Typography.caption
  }

  Text {
    textFormat: Text.PlainText
    id: linkGlyph
    anchors.left: rowLabel.right
    anchors.leftMargin: Style.space(5)
    anchors.verticalCenter: parent.verticalCenter
    visible: row.linked && row.linkOnRight
    text: "\uf0c1"
    color: linkHover.hovered ? Color.accent : Color.muted
    opacity: row.contentOpacity
    font.family: Style.font.family
    font.pixelSize: Typography.caption

    HoverHandler {
      id: linkHover
      blocking: false
    }
  }

  Row {
    id: metricRow
    z: 1
    anchors.right: parent.right
    anchors.rightMargin: row.actionsReserved && actionsLoader.active
      ? row.actionsRightMargin + actionsLoader.width + row.actionsGap
      : row.metricRightMargin
    anchors.verticalCenter: parent.verticalCenter
    height: parent.height
    spacing: row.columnSpacing
    visible: row.columnWidths.length > 0

    Repeater {
      model: row.columnWidths.length

      delegate: Item {
        id: cell
        required property int index
        readonly property bool primary: index === 0
        readonly property bool numericBar: primary && row.barColumn
        width: Number(row.columnWidths[index]) || 0
        height: metricRow.height

        Item {
          id: bar
          anchors.right: parent.right
          anchors.rightMargin: row.valueWidth + row.barGap
          anchors.verticalCenter: parent.verticalCenter
          width: cell.numericBar ? Math.min(row.barWidth, Math.max(0, cell.width - row.valueWidth - row.barGap)) : 0
          height: Style.space(6)
          visible: cell.numericBar && row.barVisible
          opacity: row.contentOpacity

          Rectangle {
            anchors.fill: parent
            color: Util.alpha(Color.bar.text, 0.08)
          }

          Rectangle {
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            width: Math.round(parent.width * Math.max(0, Math.min(1, row.barFraction)))
            height: parent.height
            color: Util.alpha(Color.accent, 0.55)
          }
        }

        Loader {
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          active: cell.primary && !!row.trailing
          sourceComponent: row.trailing
          width: active && item ? item.implicitWidth : 0
        }

        Text {
          textFormat: Text.PlainText
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          width: cell.numericBar ? Math.min(row.valueWidth, parent.width) : parent.width
          visible: !(cell.primary && !!row.trailing)
          horizontalAlignment: Text.AlignRight
          elide: Text.ElideLeft
          text: row.metricText(cell.index)
          color: Color.muted
          opacity: row.contentOpacity
          font.family: Style.font.family
          font.pixelSize: Typography.caption
        }
      }
    }
  }

  Loader {
    id: actionsLoader
    z: 1
    anchors.right: parent.right
    anchors.rightMargin: row.actionsRightMargin
    anchors.verticalCenter: parent.verticalCenter
    active: !!row.actions
    sourceComponent: row.actions
    width: active && item ? item.implicitWidth : 0
    opacity: row.actionsVisible ? 1 : 0
    enabled: row.actionsVisible
  }
}
