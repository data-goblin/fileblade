import QtQuick
import qs.Commons
import "../ui" as PluginUi
import "../theme"

Column {
  id: section

  required property var sheet
  required property var modelData
  required property int index
  property bool dividerShown: false
  property int shown: 0
  property int formShown: 0
  readonly property var moduleContext: modelData.context || null
  readonly property var schema: moduleContext && moduleContext.settings ? moduleContext.settings.schema : []
  readonly property var files: moduleContext ? moduleContext.service("files") : null

  width: parent ? parent.width : 0
  spacing: Style.space(5)
  visible: shown > 0

  function settleGroup(heading, hits) {
    if (heading) heading.visible = hits > 0
  }

  function countShown(children) {
    var count = 0
    var heading = null
    var headingHits = 0
    var lastGroup = ""
    for (var i = 0; i < children.length; i++) {
      var child = children[i]
      if (child.settingsGroup === true) {
        settleGroup(heading, headingHits)
        heading = child
        headingHits = 0
        continue
      }
      var haystack = sheet.rowHaystack(child, modelData.title + (heading ? " " + heading.title : ""))
      if (haystack === null) continue
      var hit = sheet.matches(haystack)
      child.visible = hit
      if (child.group !== undefined) {
        var group = String(child.group)
        if (hit) {
          child.groupLead = group !== "" && group !== lastGroup
          lastGroup = group
        }
      }
      if (hit) {
        count++
        headingHits++
      }
    }
    settleGroup(heading, headingHits)
    return count
  }

  function apply() {
    formShown = countShown(settingsForm.children)
    var item = moduleLoader.item
    shown = formShown + (item ? countShown(item.children) : 0)
    sheet.recount()
  }

  Connections {
    target: section.sheet
    function onQueryChanged() { section.apply() }
  }

  Rectangle {
    width: parent ? parent.width : 0
    height: 1
    visible: section.dividerShown
    color: Util.alpha(Color.bar.text, 0.12)
  }

  Text {
    textFormat: Text.PlainText
    width: parent ? parent.width : 0
    text: String(section.modelData.title || "").toUpperCase()
    color: Color.muted
    elide: Text.ElideRight
    font.family: Style.font.family
    font.pixelSize: Typography.caption
    font.letterSpacing: 0.4
  }

  PluginUi.SettingsForm {
    id: settingsForm
    width: parent.width
    visible: section.formShown > 0
    schema: section.schema
    values: section.moduleContext ? section.moduleContext.slotState : ({})
    selectedPath: section.files && section.files.selectedPath ? String(section.files.selectedPath) : ""
    onChanged: function(key, value) { if (section.moduleContext) section.moduleContext.settings.set(key, value) }
    onPopulated: section.apply()
  }

  Loader {
    id: moduleLoader
    width: parent.width
    sourceComponent: section.modelData.component
    onLoaded: {
      if (item && typeof item.retentionConsentRequested === "function")
        item.retentionConsentRequested.connect(section.sheet.confirmTrashRetention)
      section.apply()
    }
  }
}
