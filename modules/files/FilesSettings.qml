import QtQuick
import qs.Commons
import "../../ui" as PluginUi

Column {
  id: root

  required property var controller
  signal retentionConsentRequested(int days)

  spacing: Style.space(3)

  PluginUi.SettingsGroup { title: "Tree" }

  PluginUi.ToggleRow {
    width: parent.width
    glyph: "󰍉"
    label: "Auto-hide search bar"
    checked: root.controller.autoHideSearch
    onToggled: root.controller.setAutoHideSearch(!root.controller.autoHideSearch)
  }

  PluginUi.ChoiceRow {
    width: parent.width
    glyph: "󰕭"
    label: "Column"
    options: [{ key: "none", label: "Off" }, { key: "size", label: "Size" }, { key: "type", label: "Type" },
              { key: "modified", label: "Updated" }, { key: "created", label: "Created" },
              { key: "repo", label: "Repo" }, { key: "branch", label: "Branch" }, { key: "worktree", label: "Worktree" }]
    value: root.controller.priorityProperty
    onChosen: function(key) { root.controller.setPriorityProperty(key) }
  }

  PluginUi.ToggleRow {
    width: parent.width
    glyph: "󰈉"
    label: "Hidden files"
    checked: root.controller.showHidden
    onToggled: root.controller.toggleHidden()
  }

  PluginUi.ChoiceRow {
    width: parent.width
    glyph: "󰉋"
    label: "Folder context"
    options: [{ key: "selection", label: "Selection" }, { key: "project", label: "Git project" }]
    value: root.controller.projectContext ? "project" : "selection"
    onChosen: function(key) { root.controller.setProjectContext(key === "project") }
  }

  PluginUi.ToggleRow {
    width: parent.width
    glyph: "󰋽"
    label: "Property icons"
    checked: root.controller.propertyIcons
    onToggled: root.controller.setPropertyIcons(!root.controller.propertyIcons)
  }

  PluginUi.ChoiceRow {
    width: parent.width
    glyph: "󰏘"
    label: "Color applies to"
    options: [{ key: "icon", label: "Icon" }, { key: "name", label: "Icon and name" }, { key: "row", label: "Icon, name and row" }]
    value: root.controller.folderColorScope
    onChosen: function(key) { root.controller.setFolderColorScope(key) }
  }

  PluginUi.ChoiceRow {
    width: parent.width
    glyph: "\ue6ae"
    label: "Mode badge"
    options: [{ key: "header", label: "Header" }, { key: "footer", label: "Footer" }, { key: "hidden", label: "Hidden" }]
    value: root.controller.modeBadge
    onChosen: function(key) { root.controller.setModeBadge(key) }
  }

  PluginUi.ChoiceRow {
    width: parent.width
    glyph: "\uf0c1"
    label: "Drag out"
    options: [{ key: "paste", label: "Paste path" }, { key: "system", label: "System drag" }]
    value: root.controller.dragOut
    onChosen: function(key) { root.controller.setDragOut(key) }
  }

  PluginUi.SettingsGroup { title: "Git" }

  PluginUi.ToggleRow {
    width: parent.width
    glyph: "󰊢"
    label: "Git status"
    checked: root.controller.gitEnabled
    onToggled: root.controller.setGitEnabled(!root.controller.gitEnabled)
  }

  PluginUi.ToggleRow {
    width: parent.width
    glyph: "󰍶"
    label: "Git marks on the scroll ruler"
    enabled: root.controller.gitEnabled
    checked: root.controller.gitEnabled && root.controller.scrollMarks
    onToggled: root.controller.setScrollMarks(!root.controller.scrollMarks)
  }

  Repeater {
    model: root.controller.gitSummaryChoices
    delegate: PluginUi.ToggleRow {
      required property var modelData
      width: root.width
      glyph: modelData.glyph
      label: "Summary: " + modelData.label.toLowerCase()
      enabled: root.controller.gitEnabled
      checked: root.controller.gitSummaryFields.indexOf(modelData.key) >= 0
      onToggled: {
        var next = root.controller.gitSummaryFields.filter(function(key) { return key !== modelData.key })
        if (!checked) next.push(modelData.key)
        root.controller.setGitSummaryFields(next)
      }
    }
  }

  PluginUi.SettingsGroup { title: "Trash and drives" }

  PluginUi.ToggleRow {
    width: parent.width
    glyph: "󰆴"
    label: "Confirm trash"
    checked: root.controller.confirmTrash
    onToggled: root.controller.setConfirmTrash(!root.controller.confirmTrash)
  }

  PluginUi.ChoiceRow {
    width: parent.width
    glyph: "󰩺"
    label: "Trash retention"
    options: [
      { key: "0", label: "Never" },
      { key: "1", label: "1 day" },
      { key: "7", label: "7 days" },
      { key: "30", label: "30 days" },
      { key: "90", label: "90 days" }
    ]
    value: String(root.controller.trashRetentionDays)
    onChosen: function(key) {
      if (Number(key) === 0) root.controller.setTrashRetentionDays(0, false)
      else root.retentionConsentRequested(Number(key))
    }
  }

  PluginUi.ToggleRow {
    width: parent.width
    glyph: "󰋩"
    label: "Show system volumes"
    checked: root.controller.showSystemVolumes
    onToggled: root.controller.setShowSystemVolumes(!root.controller.showSystemVolumes)
  }

  PluginUi.HintLine {
    width: parent.width
    glyph: "󰉋"
    text: root.controller.rootPath
  }
}
