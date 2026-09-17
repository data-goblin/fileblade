import QtQuick
import qs.Commons
import "../lib/SettingsForm.js" as Form
import "../theme"

Column {
  id: form

  required property var schema
  required property var values
  property string selectedPath: ""
  readonly property var rows: Form.rowModel(schema, values)

  signal changed(string key, var value)
  signal populated()

  spacing: Style.space(5)

  function componentFor(row) {
    switch (String(row.type)) {
    case "boolean":
      return toggleRow
    case "enum":
      return Form.inlineChoice(row) ? choiceRow : selectRow
    case "integer":
    case "number":
      return numberRow
    }
    return textRow
  }

  Component {
    id: toggleRow

    ToggleRow {
      property var row: ({})
      signal committed(var value)
      label: String(row.label || "")
      checked: row.value === true
      onToggled: committed(!checked)
    }
  }

  Component {
    id: choiceRow

    ChoiceRow {
      property var row: ({})
      signal committed(var value)
      label: String(row.label || "")
      options: Form.choiceOptions(row)
      value: String(row.value === undefined ? "" : row.value)
      onChosen: function(key) { committed(key) }
    }
  }

  Component {
    id: selectRow

    SelectRow { }
  }

  Component {
    id: numberRow

    NumberRow { }
  }

  Component {
    id: textRow

    TextRow {
      suggestion: form.selectedPath
    }
  }

  Repeater {
    model: form.rows.length
    onItemAdded: form.populated()
    onItemRemoved: form.populated()

    delegate: Column {
      id: entry
      required property int index
      readonly property var row: form.rows[index] || ({ key: "", type: "", label: "", description: "", group: "", options: [] })
      readonly property string label: String(row.label)
      readonly property string detail: String(row.description)
      readonly property string group: String(row.group || "")
      readonly property var options: row.options
      property bool groupLead: group !== "" && (index === 0 || String((form.rows[index - 1] || ({})).group || "") !== group)
      width: form.width
      spacing: Style.space(2)

      SettingsGroup {
        width: parent.width
        visible: entry.groupLead
        title: entry.group
      }

      function commit(value) {
        if (entry.row.key !== "") form.changed(entry.row.key, value)
      }

      Loader {
        width: parent.width
        sourceComponent: form.componentFor(entry.row)
        onLoaded: {
          item.row = Qt.binding(function() { return entry.row })
          item.committed.connect(entry.commit)
        }
      }

      Text {
        textFormat: Text.PlainText
        width: parent.width
        visible: entry.detail !== ""
        text: entry.detail
        color: Color.muted
        wrapMode: Text.WordWrap
        font.family: Style.font.family
        font.pixelSize: Typography.caption
      }
    }
  }
}
