import QtQuick
import QtQuick.Controls as QQC
import qs.Commons
import qs.Ui
import "../theme"

QQC.Popup {
  id: popup

  property var rows: []
  property string prompt: "Type to filter, Enter picks"
  property string heading: ""
  property string query: ""
  property int menuWidth: Style.space(220)
  property int maximumHeight: Style.space(400)
  property bool keepOpenForMetrics: false
  readonly property var visibleRows: filteredRows()

  signal picked(string key, var row)

  function filteredRows() {
    var needle = String(query || "").trim().toLowerCase()
    if (needle === "") return rows
    return rows.filter(function(row) { return row.kind !== "separator" && String(row.label).toLowerCase().indexOf(needle) >= 0 })
  }

  function selectable(index) {
    var row = index >= 0 && index < visibleRows.length ? visibleRows[index] : null
    return !!row && row.kind !== "separator" && row.enabled !== false
  }

  function step(from, delta) {
    var index = from
    for (var guard = 0; guard < visibleRows.length; guard++) {
      index = Math.max(0, Math.min(visibleRows.length - 1, index + delta))
      if (selectable(index)) return index
      if (index === 0 || index === visibleRows.length - 1) break
    }
    return from
  }

  function indexOfChecked() {
    for (var i = 0; i < visibleRows.length; i++)
      if (visibleRows[i].checked) return i
    return step(-1, 1)
  }

  function present() {
    query = ""
    list.currentIndex = indexOfChecked()
    popup.open()
    Qt.callLater(focusSearch)
  }

  function focusSearch() {
    if (visible) searchField.forceActiveFocus()
  }

  function choose(index) {
    if (!selectable(index)) return
    var row = visibleRows[index]
    if (!keepOpenForMetrics || row.kind !== "metric") popup.close()
    popup.picked(String(row.key || ""), row)
  }

  function chooseCurrent() {
    choose(selectable(list.currentIndex) ? list.currentIndex : step(-1, 1))
  }

  function move(delta) {
    list.currentIndex = step(list.currentIndex, delta)
  }

  function edge(last) {
    list.currentIndex = last ? step(visibleRows.length, -1) : step(-1, 1)
  }

  onQueryChanged: list.currentIndex = step(-1, 1)
  onClosed: query = ""

  width: menuWidth
  height: Math.min(header.height + searchRow.height + list.contentHeight + Style.space(4), maximumHeight)
  padding: 1
  focus: true
  closePolicy: QQC.Popup.CloseOnEscape | QQC.Popup.CloseOnPressOutside

  background: BorderSurface {
    color: Color.popups.background
    borderSpec: Border.surfaceSpec("popups", "border", Color.popups.border, Style.normalBorderWidth)
    radius: Style.cornerRadius
  }

  contentItem: Item {
    Item {
      id: header
      anchors.top: parent.top
      anchors.left: parent.left
      anchors.right: parent.right
      height: popup.heading !== "" ? Style.space(30) : 0
      visible: popup.heading !== ""

      Text {
        textFormat: Text.PlainText
        id: headingText
        anchors.left: parent.left
        anchors.leftMargin: Style.space(10)
        anchors.verticalCenter: parent.verticalCenter
        text: popup.heading
        color: Color.bar.text
        font.family: Style.font.family
        font.pixelSize: Typography.bodySmall
        font.weight: Font.DemiBold
      }

      Text {
        textFormat: Text.PlainText
        id: closeGlyph
        anchors.right: parent.right
        anchors.rightMargin: Style.space(10)
        anchors.verticalCenter: parent.verticalCenter
        text: "×"
        color: closePointer.containsMouse ? Color.bar.text : Color.muted
        font.family: Style.font.family
        font.pixelSize: Typography.body

        MouseArea {
          id: closePointer
          anchors.fill: parent
          anchors.margins: -Style.space(6)
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: popup.close()
        }
      }

      Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.leftMargin: Style.space(4)
        anchors.rightMargin: Style.space(4)
        height: 1
        color: Util.alpha(Color.bar.text, 0.12)
      }
    }

    Item {
      id: searchRow
      anchors.top: header.bottom
      anchors.left: parent.left
      anchors.right: parent.right
      height: Style.space(36)

      MenuSearchField {
        id: searchField
        anchors.fill: parent
        anchors.margins: Style.space(4)
        prompt: popup.prompt
        text: popup.query
        onTextEdited: popup.query = text
        onMoved: function(delta) { popup.move(delta) }
        onEdgeRequested: function(last) { popup.edge(last) }
        onPicked: popup.chooseCurrent()
        onDismissed: popup.close()
      }
    }

    ListView {
      id: list
      anchors.top: searchRow.bottom
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.bottom: parent.bottom
      clip: true
      boundsBehavior: Flickable.StopAtBounds
      model: popup.visibleRows
      currentIndex: -1

      delegate: Rectangle {
        id: optionRow
        required property int index
        required property var modelData
        readonly property bool separator: modelData.kind === "separator"
        readonly property bool labeled: separator && String(modelData.label || "") !== ""
        enabled: popup.selectable(index)
        width: list.width
        height: separator ? (labeled ? Style.space(22) : Style.space(9)) : Style.space(31)
        color: index === list.currentIndex
          ? Style.hoverFillFor(Color.popups.text, Color.accent)
          : "transparent"

        Rectangle {
          anchors.left: parent.left
          anchors.right: parent.right
          anchors.leftMargin: Style.space(8)
          anchors.rightMargin: Style.space(8)
          anchors.verticalCenter: parent.verticalCenter
          height: 1
          visible: optionRow.separator && !optionRow.labeled
          color: Util.alpha(Color.popups.text, 0.14)
        }

        Text {
          anchors.left: parent.left
          anchors.right: parent.right
          anchors.leftMargin: Style.space(10)
          anchors.rightMargin: Style.space(9)
          anchors.bottom: parent.bottom
          anchors.bottomMargin: Style.space(3)
          textFormat: Text.PlainText
          visible: optionRow.labeled
          text: optionRow.labeled ? String(optionRow.modelData.label).toUpperCase() : ""
          color: Color.muted
          elide: Text.ElideRight
          font.family: Style.font.family
          font.pixelSize: Typography.caption
          font.letterSpacing: 0.4
        }

        Text {
          id: rowGlyph
          anchors.left: parent.left
          anchors.leftMargin: Style.space(10)
          anchors.verticalCenter: parent.verticalCenter
          width: Style.space(16)
          textFormat: Text.PlainText
          visible: !optionRow.separator
          text: optionRow.separator ? "" : String(optionRow.modelData.glyph || "")
          color: optionRow.modelData.danger ? Color.urgent : (optionRow.enabled ? Color.muted : Util.alpha(Color.popups.text, 0.3))
          horizontalAlignment: Text.AlignHCenter
          font.family: Style.font.family
          font.pixelSize: Typography.bodySmall
        }

        Text {
          anchors.left: rowGlyph.right
          anchors.right: check.left
          anchors.leftMargin: Style.space(8)
          anchors.rightMargin: Style.space(6)
          anchors.verticalCenter: parent.verticalCenter
          textFormat: Text.PlainText
          visible: !optionRow.separator
          text: optionRow.separator ? "" : String(optionRow.modelData.label)
          color: optionRow.modelData.danger && optionRow.enabled ? Color.urgent : (optionRow.enabled ? Color.popups.text : Util.alpha(Color.popups.text, 0.4))
          elide: Text.ElideRight
          font.family: Style.font.family
          font.pixelSize: Typography.bodySmall
        }

        Text {
          id: check
          anchors.right: parent.right
          anchors.rightMargin: Style.space(9)
          anchors.verticalCenter: parent.verticalCenter
          textFormat: Text.PlainText
          text: optionRow.separator ? "" : (optionRow.modelData.checked ? "✓" : String(optionRow.modelData.hint || ""))
          color: optionRow.modelData.checked ? Color.accent : Color.muted
          font.family: Style.font.family
          font.pixelSize: Typography.caption
        }

        MouseArea {
          anchors.fill: parent
          enabled: optionRow.enabled
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onPositionChanged: list.currentIndex = optionRow.index
          onClicked: popup.choose(optionRow.index)
        }
      }
    }
  }
}
