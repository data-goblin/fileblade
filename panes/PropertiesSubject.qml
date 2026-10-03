import QtQuick
import qs.Commons
import "../ui" as PluginUi
import "../theme"

FocusScope {
  id: view

  property var subject: null
  property var properties: null
  property var files: null
  property var targetScreen: null
  property color surfaceColor: Color.bar.background
  property int cursor: -1
  property int copiedIndex: -1
  property string subjectKey: ""
  property string openedLink: ""
  readonly property var fields: subject && Array.isArray(subject.fields) ? subject.fields : []
  readonly property var actions: subject && Array.isArray(subject.actions) ? subject.actions : []
  readonly property int count: fields.length + actions.length
  readonly property bool cursorShown: activeFocus && cursor >= 0
  readonly property var kindGlyphs: ({ text: "󰋽", multiline: "󰦨", tags: "󰓹", link: "󰌹", code: "󰅩" })
  readonly property alias flickable: scroller

  signal keyPressed(var event)
  signal focusRequested()

  onSubjectChanged: {
    var key = subject ? subject.title + "\n" + subject.subtitle : ""
    var total = subject ? subject.fields.length + subject.actions.length : 0
    if (key !== subjectKey) {
      subjectKey = key
      scroller.contentY = 0
      copiedIndex = -1
      openedLink = ""
      cursor = subject && subject.fields.length > 0 ? subject.actions.length : (total > 0 ? 0 : -1)
    } else {
      cursor = Math.min(cursor, total - 1)
      if (cursor < 0 && total > 0) cursor = 0
    }
  }

  function takeFocus() {
    scroller.forceActiveFocus()
  }

  function tint(value) {
    var token = String(value || "")
    if (token === "muted") return Color.muted
    if (token === "urgent") return Color.urgent
    if (token === "text") return Color.bar.text
    return token.charAt(0) === "#" ? token : Color.accent
  }

  function itemAt(index) {
    if (index < 0) return null
    if (index < actions.length) return actionGrid.itemAt(index)
    return fieldRepeater.itemAt(index - actions.length)
  }

  function reveal(index) {
    var item = itemAt(index)
    if (!item) return
    var top = item.mapToItem(content, 0, 0).y + content.y
    var bottom = top + item.height
    var limit = Math.max(0, scroller.contentHeight - scroller.height)
    var margin = edgeFade.fadeHeight
    if (index === 0) scroller.contentY = 0
    else if (top < scroller.contentY + margin) scroller.contentY = Math.max(0, top - margin)
    else if (bottom > scroller.contentY + scroller.height - margin) scroller.contentY = Math.min(limit, bottom - scroller.height + margin)
  }

  function moveCursor(delta) {
    if (count === 0) {
      scroll(delta * Style.space(34))
      return
    }
    cursor = Math.max(0, Math.min(count - 1, (cursor < 0 ? 0 : cursor + delta)))
    reveal(cursor)
  }

  function setCursor(index) {
    if (count === 0) return
    cursor = Math.max(0, Math.min(count - 1, index))
    reveal(cursor)
  }

  function scroll(distance) {
    scroller.contentY = Math.max(0, Math.min(Math.max(0, scroller.contentHeight - scroller.height), scroller.contentY + distance))
  }

  function openLink(url) {
    if (!files || typeof files.openUrl !== "function") return false
    openedLink = String(url)
    return files.openUrl(openedLink, targetScreen)
  }

  function copyField(index) {
    var field = fields[index]
    if (!field || !files || typeof files.copyText !== "function") return false
    if (!files.copyText(field.text)) return false
    copiedIndex = index
    copiedTimer.restart()
    return true
  }

  function trigger(index) {
    var action = actions[index]
    return !!action && !!properties && properties.trigger(action.id)
  }

  function activate(index) {
    if (index < 0 || index >= count) return false
    if (index < actions.length) return trigger(index)
    var field = fields[index - actions.length]
    if (field.kind === "link") return openLink(field.value)
    if (field.kind === "code") return copyField(index - actions.length)
    return false
  }

  function copyCurrent() {
    return cursor >= actions.length && cursor < count ? copyField(cursor - actions.length) : false
  }

  function runAction(action) {
    var handlers = {
      "scroll-down": function() { moveCursor(1) },
      "scroll-up": function() { moveCursor(-1) },
      first: function() { setCursor(0); scroller.contentY = 0 },
      last: function() { setCursor(count - 1); scroll(scroller.contentHeight) },
      "page-down": function() { scroll(scroller.height * 0.8) },
      "page-up": function() { scroll(-scroller.height * 0.8) },
      activate: function() { activate(cursor) },
      open: function() { activate(cursor) },
      copy: function() { copyCurrent() }
    }
    var handler = handlers[action]
    if (!handler) return false
    handler()
    return true
  }

  function press(index) {
    if (!activeFocus) focusRequested()
    setCursor(index)
  }

  Timer {
    id: copiedTimer
    interval: 1400
    onTriggered: view.copiedIndex = -1
  }

  component FieldValue: Item {
    id: valueItem
    required property var field
    required property int index
    readonly property bool copied: view.copiedIndex === index
    implicitHeight: field.kind === "tags" ? chips.implicitHeight
      : field.kind === "code" ? codeText.implicitHeight + Style.space(10)
      : valueText.implicitHeight

    Text {
      id: valueText
      visible: valueItem.field.kind !== "tags" && valueItem.field.kind !== "code"
      width: parent.width
      textFormat: Text.PlainText
      text: visible ? String(valueItem.field.value) : ""
      color: valueItem.field.kind === "link" ? (linkPointer.containsMouse ? Qt.lighter(Color.accent, 1.15) : Color.accent) : Color.bar.text
      font.underline: valueItem.field.kind === "link" && linkPointer.containsMouse
      wrapMode: valueItem.field.kind === "link" ? Text.WrapAnywhere : Text.Wrap
      font.family: Style.font.family
      font.pixelSize: Typography.bodySmall

      MouseArea {
        id: linkPointer
        anchors.fill: parent
        enabled: valueItem.field.kind === "link"
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: { view.press(view.actions.length + valueItem.index); view.openLink(valueItem.field.value) }
      }

      PluginUi.HintTip {
        visible: linkPointer.containsMouse
        title: "Open link"
        actions: [{ button: "left", text: "Open" }, { shortcut: "Enter" }]
      }
    }

    Rectangle {
      visible: valueItem.field.kind === "code"
      width: parent.width
      height: visible ? codeText.implicitHeight + Style.space(10) : 0
      color: codePointer.containsMouse ? Style.hoverFillFor(Color.bar.text, Color.accent) : Util.alpha(Color.bar.text, 0.06)
      border.width: 1
      border.color: valueItem.copied ? Color.accent : Util.alpha(Color.bar.text, 0.12)

      Text {
        id: codeText
        x: Style.space(6)
        y: Style.space(5)
        width: parent.width - Style.space(12) - (valueItem.copied ? copiedLabel.implicitWidth + Style.space(6) : 0)
        textFormat: Text.PlainText
        text: valueItem.field.kind === "code" ? String(valueItem.field.value) : ""
        color: Color.bar.text
        wrapMode: Text.WrapAnywhere
        font.family: "monospace"
        font.pixelSize: Typography.bodySmall
      }

      Text {
        id: copiedLabel
        anchors.right: parent.right
        anchors.rightMargin: Style.space(6)
        y: Style.space(5)
        visible: valueItem.copied
        textFormat: Text.PlainText
        text: "Copied"
        color: Color.accent
        font.family: Style.font.family
        font.pixelSize: Typography.caption
      }

      MouseArea {
        id: codePointer
        anchors.fill: parent
        enabled: valueItem.field.kind === "code"
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: { view.press(view.actions.length + valueItem.index); view.copyField(valueItem.index) }
      }

      PluginUi.HintTip {
        visible: codePointer.containsMouse
        title: "Copy"
        actions: [{ button: "left", text: "Copy" }, { shortcut: "Enter" }, { shortcut: "y" }]
      }
    }

    Flow {
      id: chips
      visible: valueItem.field.kind === "tags"
      width: parent.width
      spacing: Style.space(4)

      Repeater {
        model: valueItem.field.kind === "tags" ? valueItem.field.value : []
        delegate: Rectangle {
          required property var modelData
          implicitWidth: chipText.implicitWidth + Style.space(12)
          implicitHeight: chipText.implicitHeight + Style.space(4)
          color: Util.alpha(Color.accent, 0.12)
          border.width: 1
          border.color: Util.alpha(Color.accent, 0.35)

          Text {
            id: chipText
            anchors.centerIn: parent
            textFormat: Text.PlainText
            text: String(parent.modelData)
            color: Color.bar.text
            font.family: Style.font.family
            font.pixelSize: Typography.caption
          }
        }
      }
    }
  }

  Flickable {
    id: scroller
    anchors.fill: parent
    contentWidth: width
    contentHeight: content.implicitHeight + Style.space(18)
    clip: true
    focus: true
    boundsBehavior: Flickable.StopAtBounds
    Keys.onPressed: function(event) { view.keyPressed(event) }

    Column {
      id: content
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.leftMargin: Style.space(10)
      anchors.rightMargin: Style.space(10)
      anchors.top: parent.top
      anchors.topMargin: Style.space(9)
      spacing: Style.space(9)

      Item {
        width: parent.width
        height: Math.max(Style.space(24), titleText.implicitHeight)

        Text {
          id: subjectGlyph
          anchors.left: parent.left
          anchors.verticalCenter: parent.verticalCenter
          width: visible ? Style.space(19) : 0
          visible: !!view.subject && view.subject.glyph !== ""
          textFormat: Text.PlainText
          horizontalAlignment: Text.AlignHCenter
          text: view.subject ? view.subject.glyph : ""
          color: view.tint(view.subject ? view.subject.color : "")
          font.family: view.subject && view.subject.glyphFamily ? view.subject.glyphFamily : Style.font.family
          font.pixelSize: Typography.title
        }

        Text {
          id: titleText
          anchors.left: subjectGlyph.right
          anchors.leftMargin: subjectGlyph.visible ? Style.space(6) : 0
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          textFormat: Text.PlainText
          text: view.subject ? view.subject.title : ""
          color: Color.bar.text
          wrapMode: Text.Wrap
          maximumLineCount: 3
          elide: Text.ElideRight
          font.family: Style.font.family
          font.pixelSize: Typography.title
          font.weight: Font.DemiBold
        }
      }

      Text {
        width: parent.width
        visible: text !== ""
        textFormat: Text.PlainText
        text: view.subject ? view.subject.subtitle : ""
        color: Color.muted
        wrapMode: Text.Wrap
        font.family: Style.font.family
        font.pixelSize: Typography.bodySmall
      }

      PluginUi.ActionGrid {
        id: actionGrid
        width: parent.width
        visible: view.actions.length > 0
        actions: view.actions
        current: view.cursor
        showCurrent: view.cursorShown
        onActivated: function(index) { view.press(index); view.trigger(index) }
      }

      Flow {
        id: fieldGrid
        width: parent.width
        spacing: Style.space(9)
        readonly property int cellMinimum: Math.ceil(sample.width) + Style.space(12)
        readonly property int columns: Math.max(1, Math.min(3, Math.floor((width + spacing) / (cellMinimum + spacing))))
        readonly property real cell: Math.floor((width - spacing * (columns - 1)) / columns)

        TextMetrics {
          id: sample
          font.family: Style.font.family
          font.pixelSize: Typography.bodySmall
          text: "2026-09-02 16:31:27"
        }

        Repeater {
          id: fieldRepeater
          model: view.fields
          delegate: Rectangle {
            id: fieldRow
            required property var modelData
            required property int index
            readonly property bool current: view.cursorShown && view.cursor === view.actions.length + index
            readonly property string glyph: view.files && view.files.propertyIcons ? String(view.kindGlyphs[modelData.kind] || "󰋽") : ""
            width: fieldGrid.columns > 1 && modelData.kind === "text" && String(modelData.value).length <= 40 ? fieldGrid.cell : fieldGrid.width
            height: fieldColumn.implicitHeight + Style.space(4)
            color: current ? Util.alpha(Color.accent, 0.10) : "transparent"

            MouseArea {
              anchors.fill: parent
              onPressed: view.press(view.actions.length + fieldRow.index)
            }

            Column {
              id: fieldColumn
              x: Style.space(2)
              y: Style.space(2)
              width: parent.width - Style.space(4)
              spacing: Style.space(2)

              Row {
                width: parent.width
                spacing: Style.space(5)

                Text {
                  visible: fieldRow.glyph !== ""
                  width: visible ? Style.space(14) : 0
                  textFormat: Text.PlainText
                  text: fieldRow.glyph
                  color: Util.alpha(Color.muted, 0.8)
                  horizontalAlignment: Text.AlignHCenter
                  font.family: Style.font.family
                  font.pixelSize: Typography.caption
                }

                Text {
                  textFormat: Text.PlainText
                  text: String(fieldRow.modelData.label).toUpperCase()
                  color: fieldRow.current ? Color.accent : Color.muted
                  font.family: Style.font.family
                  font.pixelSize: Typography.caption
                  font.letterSpacing: 0.4
                }
              }

              FieldValue {
                width: parent.width
                field: fieldRow.modelData
                index: fieldRow.index
              }
            }
          }
        }
      }

      Text {
        width: parent.width
        visible: !!view.subject && view.subject.truncated
        textFormat: Text.PlainText
        text: "Some properties were left out because the module sent more than FileBlade shows."
        color: Color.muted
        wrapMode: Text.Wrap
        font.family: Style.font.family
        font.pixelSize: Typography.caption
      }

      PluginUi.ErrorNotice {
        width: parent.width
        text: view.files ? String(view.files.operationError || "") : ""
        onDismissed: if (view.files) view.files.operationError = ""
      }

      PluginUi.ErrorNotice {
        width: parent.width
        text: view.openedLink !== "" && view.files ? String(view.files.launchError || "") : ""
        onDismissed: view.openedLink = ""
      }
    }
  }

  PluginUi.ScrollEdgeFade {
    id: edgeFade
    anchors.fill: scroller
    flickable: scroller
    surfaceColor: view.surfaceColor
    visible: hasAbove || hasBelow
    z: 24
  }
}
