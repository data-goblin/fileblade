import QtQuick
import QtQuick.Controls as QQC
import qs.Commons
import qs.Ui
import "../lib/Format.js" as Format
import "../theme"

QQC.Popup {
  id: popup

  property var view: null
  property var draft: ({})
  readonly property var sections: filterableOptions()
  readonly property var presets: [
    { label: "7d", value: "7d" }, { label: "30d", value: "30d" }, { label: "90d", value: "90d" }, { label: "1y", value: "1y" }
  ]

  width: Style.space(270)
  height: Math.min(content.implicitHeight + Style.space(24), Style.space(420))
  padding: Style.space(11)
  focus: true
  closePolicy: QQC.Popup.CloseOnEscape | QQC.Popup.CloseOnPressOutside

  function filterableOptions() {
    var list = view ? view.options : []
    var result = []
    for (var i = 0; i < list.length; i++) {
      var kind = Format.metricKind(list[i])
      if (list[i].filter === false) continue
      if (kind === "date" || kind === "number") result.push({ key: String(list[i].key), label: String(list[i].label || list[i].key), kind: kind })
    }
    return result
  }

  function field(key, name) {
    var clause = draft[key]
    return clause && clause[name] !== undefined ? String(clause[name]) : ""
  }

  function setField(key, name, text) {
    var next = ({})
    for (var existing in draft) next[existing] = draft[existing]
    var clause = ({})
    for (var known in (next[key] || ({}))) clause[known] = next[key][known]
    if (String(text) === "") delete clause[name]
    else clause[name] = String(text)
    next[key] = clause
    draft = next
  }

  function normalized(section, name, text) {
    if (String(text).trim() === "") return ""
    if (section.kind === "date") return Format.parseDate(text)
    var number = Format.parseNumber(text)
    return isFinite(number) ? number : ""
  }

  function invalid(section, name) {
    var text = field(section.key, name)
    return text.trim() !== "" && normalized(section, name, text) === ""
  }

  function apply() {
    var spec = ({})
    for (var i = 0; i < sections.length; i++) {
      var section = sections[i]
      var names = section.kind === "date" ? ["since", "until"] : ["min", "max"]
      var clause = ({})
      for (var j = 0; j < names.length; j++) {
        var value = normalized(section, names[j], field(section.key, names[j]))
        if (value !== "") clause[names[j]] = value
      }
      if (Object.keys(clause).length > 0) spec[section.key] = clause
    }
    if (view) view.setFilter(spec)
    popup.close()
  }

  function clearAll() {
    draft = ({})
    if (view) view.clearFilter()
    popup.close()
  }

  onAboutToShow: {
    var current = view ? view.filter : ({})
    var copy = ({})
    for (var key in current) {
      copy[key] = ({})
      for (var name in current[key]) copy[key][name] = String(current[key][name])
    }
    draft = copy
  }

  background: BorderSurface {
    color: Color.popups.background
    borderSpec: Border.surfaceSpec("popups", "border", Color.popups.border, Style.normalBorderWidth)
    radius: Style.cornerRadius
  }

  component FilterField: QQC.TextField {
    id: input
    property bool problem: false
    height: Style.space(26)
    leftPadding: Style.space(7)
    rightPadding: Style.space(7)
    selectByMouse: true
    color: Color.popups.text
    placeholderTextColor: Util.alpha(Color.popups.text, 0.4)
    font.family: Style.font.family
    font.pixelSize: Typography.bodySmall
    background: Rectangle {
      color: Util.alpha(Color.popups.text, input.activeFocus ? 0.10 : 0.06)
      radius: Math.min(Style.cornerRadius, Style.space(4))
      border.width: input.problem ? 1 : 0
      border.color: Color.urgent
    }
    Keys.onReturnPressed: popup.apply()
    Keys.onEnterPressed: popup.apply()
  }

  component ActionChip: Rectangle {
    id: action
    property string label: ""
    property bool primary: false
    signal clicked()
    width: actionLabel.implicitWidth + Style.space(18)
    height: Style.space(26)
    radius: Math.min(Style.cornerRadius, Style.space(4))
    color: primary
      ? (actionPointer.containsMouse ? Color.accent : Util.alpha(Color.accent, 0.82))
      : Util.alpha(Color.popups.text, actionPointer.containsMouse ? 0.12 : 0.06)

    Text {
      id: actionLabel
      textFormat: Text.PlainText
      anchors.centerIn: parent
      text: action.label
      color: action.primary ? Color.background : Color.popups.text
      font.family: Style.font.family
      font.pixelSize: Typography.bodySmall
      font.weight: action.primary ? Font.DemiBold : Font.Normal
    }

    MouseArea {
      id: actionPointer
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: action.clicked()
    }
  }

  contentItem: Flickable {
    clip: true
    contentHeight: content.implicitHeight
    boundsBehavior: Flickable.StopAtBounds

    Column {
      id: content
      width: parent.width
      spacing: Style.space(8)

      Text {
        textFormat: Text.PlainText
        text: "FILTER"
        color: Color.popups.text
        font.family: Style.font.family
        font.pixelSize: Typography.caption
        font.weight: Font.DemiBold
        font.letterSpacing: 0.6
      }

      Text {
        textFormat: Text.PlainText
        width: parent.width
        visible: popup.sections.length === 0
        text: "No filterable metrics"
        wrapMode: Text.WordWrap
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Typography.caption
      }

      Repeater {
        model: popup.sections

        delegate: Column {
          id: section
          required property var modelData
          readonly property bool dated: modelData.kind === "date"
          readonly property string firstName: dated ? "since" : "min"
          readonly property string secondName: dated ? "until" : "max"
          width: content.width
          spacing: Style.space(4)

          Text {
            textFormat: Text.PlainText
            text: section.modelData.label
            color: Color.popups.text
            font.family: Style.font.family
            font.pixelSize: Typography.bodySmall
            font.weight: Font.DemiBold
          }

          Row {
            width: parent.width
            spacing: Style.space(6)

            FilterField {
              id: firstField
              width: (parent.width - Style.space(6)) / 2
              placeholderText: section.dated ? "since (2026-08, 7d)" : "min (1.2k)"
              text: popup.field(section.modelData.key, section.firstName)
              problem: popup.invalid(section.modelData, section.firstName)
              onTextEdited: popup.setField(section.modelData.key, section.firstName, text)
            }

            FilterField {
              width: (parent.width - Style.space(6)) / 2
              placeholderText: section.dated ? "until (today)" : "max"
              text: popup.field(section.modelData.key, section.secondName)
              problem: popup.invalid(section.modelData, section.secondName)
              onTextEdited: popup.setField(section.modelData.key, section.secondName, text)
            }
          }

          Row {
            visible: section.dated
            spacing: Style.space(4)

            Repeater {
              model: popup.presets

              delegate: Rectangle {
                id: chip
                required property var modelData
                readonly property bool active: popup.field(section.modelData.key, "since") === modelData.value
                width: chipLabel.implicitWidth + Style.space(12)
                height: Style.space(20)
                radius: height / 2
                color: active ? Util.alpha(Color.accent, 0.28) : Util.alpha(Color.popups.text, chipPointer.containsMouse ? 0.12 : 0.06)

                Text {
                  id: chipLabel
                  textFormat: Text.PlainText
                  anchors.centerIn: parent
                  text: chip.modelData.label
                  color: chip.active ? Color.accent : Color.popups.text
                  font.family: Style.font.family
                  font.pixelSize: Typography.caption
                }

                MouseArea {
                  id: chipPointer
                  anchors.fill: parent
                  hoverEnabled: true
                  cursorShape: Qt.PointingHandCursor
                  onClicked: {
                    popup.setField(section.modelData.key, "since", chip.active ? "" : chip.modelData.value)
                    popup.setField(section.modelData.key, "until", "")
                  }
                }
              }
            }
          }
        }
      }

      Row {
        anchors.right: parent.right
        spacing: Style.space(6)
        visible: popup.sections.length > 0

        ActionChip {
          label: "Clear"
          onClicked: popup.clearAll()
        }

        ActionChip {
          label: "Apply"
          primary: true
          onClicked: popup.apply()
        }
      }
    }
  }
}
