import QtQuick
import "../lib/MonitorMode.js" as MonitorMode
import QtQuick.Controls
import qs.Commons
import qs.Ui
import "../ui" as PluginUi
import "../theme"

Item {
  id: root

  required property var host
  required property var surface

  readonly property string edge: surface.edge
  readonly property var registry: host.registry
  property string query: ""
  property int moduleShown: 0
  readonly property bool filtering: query.trim() !== ""
  property var drag: null
  property var dropTarget: null
  property int pendingRetentionDays: 0

  function confirmTrashRetention(days) {
    pendingRetentionDays = days
    retentionConsent.open("Enable automatic Trash cleanup?\nPermanently delete items older than " + days + " days from your shared desktop Trash, including items trashed by other apps, and FileBlade artifact bins. Cleanup also runs while blades are closed. This cannot be undone.",
      [{ key: "cancel", label: "Cancel" }, { key: "enable", label: "Enable cleanup", danger: true }])
  }

  PluginUi.ActionDialog {
    id: retentionConsent
    anchors.fill: parent
    z: 1000
    onChosen: function(key) { if (key === "enable") root.host.service.setTrashRetentionDays(root.pendingRetentionDays, true) }
  }

  PluginUi.ActionDialog {
    id: managementConsent
    anchors.fill: parent
    z: 1000
    onChosen: function(key) { if (key === "enable") root.host.service.preferences.setAgentManagement(true) }
  }

  function beginTabDrag(edge, slotIndex, tabIndex, title, glyph, x, y, grabX, grabY, width) {
    drag = { edge: edge, slotIndex: slotIndex, tabIndex: tabIndex, title: title, glyph: glyph, x: x, y: y, grabX: grabX, grabY: grabY, width: width }
    dropTarget = null
  }

  function moveTabDrag(x, y) {
    if (!drag) return
    var next = {}
    for (var key in drag) next[key] = drag[key]
    next.x = x
    next.y = y
    drag = next
    dropTarget = dropTargetAt(x, y)
  }

  function cancelTabDrag() {
    drag = null
    dropTarget = null
  }

  function finishTabDrag() {
    var lifted = drag
    var target = dropTarget
    drag = null
    dropTarget = null
    if (!lifted || !target) return
    if (target.newSlot) {
      host.moveSlotTo(lifted.edge, lifted.slotIndex, target.edge, target.slotIndex, lifted.tabIndex)
      return
    }
    host.moveTabInto(lifted.edge, lifted.slotIndex, lifted.tabIndex, target.edge, target.slotIndex, target.insertAt)
  }

  function dropTargetAt(x, y) {
    var columns = [leftColumn, rightColumn]
    for (var c = 0; c < columns.length; c++) {
      var column = columns[c]
      if (!column.visible) continue
      var origin = column.mapToItem(card, 0, 0)
      if (x < origin.x - Style.space(6) || x > origin.x + column.width + Style.space(6)) continue
      var rows = column.rows()
      var left = origin.x
      var width = column.width
      if (rows.length === 0) return { edge: column.bladeEdge, slotIndex: 0, insertAt: 0, newSlot: true, x: left, width: width, y: origin.y + column.height }
      var previous = null
      var previousBottom = 0
      for (var i = 0; i < rows.length; i++) {
        var row = rows[i]
        var position = row.item.mapToItem(card, 0, 0)
        var top = position.y
        var bottom = top + row.item.height
        if (y < top) {
          if (previous && previous.slotIndex !== row.slotIndex)
            return { edge: column.bladeEdge, slotIndex: row.slotIndex, insertAt: 0, newSlot: true, x: left, width: width, y: (previousBottom + top) / 2 }
          return { edge: column.bladeEdge, slotIndex: row.slotIndex, insertAt: row.tabIndex, newSlot: false, x: left, width: width, y: top }
        }
        if (y <= bottom) {
          var before = y < top + row.item.height / 2
          return { edge: column.bladeEdge, slotIndex: row.slotIndex, insertAt: before ? row.tabIndex : row.tabIndex + 1, newSlot: false,
                   x: left, width: width, y: before ? top : bottom }
        }
        previous = row
        previousBottom = bottom
      }
      if (y > previousBottom + Style.space(40)) return null
      return { edge: column.bladeEdge, slotIndex: previous.slotIndex + 1, insertAt: 0, newSlot: true, x: left, width: width, y: previousBottom + Style.space(4) }
    }
    return null
  }

  function close() {
    host.setSettingsOpen(false, edge)
    host.focusBlade(edge, surface.screen, -1, "")
  }

  function openShortcuts() {
    host.setSettingsOpen(false, edge)
    surface.shortcutsOpen = true
  }

  function confirmRevert() {
    revertDialog.open(
      "Are you sure?\nThis will revert to default settings and layouts. It doesn't affect key bindings.",
      [{ key: "revert", label: "Yes", danger: true }, { key: "cancel", label: "Cancel" }])
    revertDialog.selectedIndex = 1
  }

  function edgeTitle(value) {
    return value === "right" ? "RIGHT BLADE" : "LEFT BLADE"
  }

  function moduleLabel(moduleId) {
    var module = registry.module(moduleId)
    return module ? String(module.name) : String(moduleId)
  }

  function moduleGlyph(moduleId) {
    var module = registry.module(moduleId)
    return module && module.glyph ? String(module.glyph) : "󰏗"
  }

  function availableModules() {
    var revision = registry.revision
    var groups = []
    var current = null
    for (var i = 0; i < registry.order.length && revision >= 0; i++) {
      var module = registry.module(registry.order[i])
      if (!module) continue
      if (module.singleton && host.findModule(module.id)) continue
      if (!current || current.category !== String(module.category)) {
        current = { category: String(module.category), modules: [] }
        groups.push(current)
      }
      current.modules.push(module)
    }
    return groups
  }

  function addLabel(scope, module) {
    return scope + "add module " + String(module.category) + " " + String(module.name)
  }

  function slotSettings() {
    var result = []
    for (var i = 0; i < surface.slots.length; i++) {
      var item = surface.slotItem(i)
      if (!item) continue
      var context = item.settingsContext || null
      var declared = !!context && !!context.settings && context.settings.schema.length > 0
      if (item.settingsComponent || declared)
        result.push({ title: String(item.title || ""), component: item.settingsComponent || null, context: context })
    }
    return result
  }

  function matches(haystack) {
    var needle = query.trim().toLowerCase()
    if (needle === "") return true
    var hay = String(haystack).toLowerCase()
    var terms = needle.split(/\s+/)
    for (var i = 0; i < terms.length; i++)
      if (hay.indexOf(terms[i]) < 0) return false
    return true
  }

  function rowHaystack(item, title) {
    var parts = [String(title || "")]
    if (item.label !== undefined) parts.push(String(item.label))
    else if (item.shortcut !== undefined && item.text !== undefined) parts.push(String(item.text))
    else return null
    if (item.detail !== undefined) parts.push(String(item.detail))
    if (item.group !== undefined) parts.push(String(item.group))
    var options = item.options
    if (options && options.length !== undefined)
      for (var i = 0; i < options.length; i++)
        parts.push(String(options[i] && options[i].label !== undefined ? options[i].label : options[i]))
    return parts.join(" ")
  }

  function recount() {
    var total = 0
    for (var i = 0; i < moduleRepeater.count; i++) {
      var section = moduleRepeater.itemAt(i)
      if (section) total += section.shown
    }
    moduleShown = total
  }

  function countMatching(labels) {
    var count = 0
    for (var i = 0; i < labels.length; i++)
      if (matches(labels[i])) count++
    return count
  }

  onVisibleChanged: {
    if (!visible) return
    search.text = ""
    Qt.callLater(function() { search.forceActiveFocus() })
  }

  focus: visible
  Keys.onPressed: function(event) {
    if (event.key === Qt.Key_Escape) {
      root.close()
      event.accepted = true
    }
  }

  component SectionLabel: Text {
    textFormat: Text.PlainText
    width: parent ? parent.width : 0
    color: Color.muted
    elide: Text.ElideRight
    font.family: Style.font.family
    font.pixelSize: Typography.caption
    font.letterSpacing: 0.4
  }

  component Divider: Rectangle {
    width: parent ? parent.width : 0
    height: 1
    color: Util.alpha(Color.bar.text, 0.12)
  }

  component IconAction: Item {
    id: action
    required property string glyph
    property string tooltipText: ""
    property string tooltipAction: ""
    property bool danger: false
    signal clicked()
    width: Style.space(18)
    height: Style.space(20)
    opacity: enabled ? 1 : 0.3

    Text {
      textFormat: Text.PlainText
      anchors.centerIn: parent
      text: action.glyph
      color: action.danger && actionPointer.containsMouse ? Color.urgent : (actionPointer.containsMouse ? Color.bar.text : Color.muted)
      font.family: Style.font.family
      font.pixelSize: Typography.bodySmall
    }

    MouseArea {
      id: actionPointer
      anchors.fill: parent
      hoverEnabled: true
      enabled: action.enabled
      cursorShape: Qt.PointingHandCursor
      onClicked: action.clicked()
    }

    PluginUi.HintTip {
      visible: tooltipText !== "" && actionPointer.containsMouse
      title: tooltipText
      actions: tooltipAction !== "" ? [{ button: "left", text: tooltipAction }] : []
    }
  }

  component Link: Text {
    id: link
    signal clicked()
    textFormat: Text.PlainText
    color: linkPointer.containsMouse ? Color.bar.text : Color.muted
    font.family: Style.font.family
    font.pixelSize: Typography.caption

    MouseArea {
      id: linkPointer
      anchors.fill: parent
      anchors.margins: -Style.space(4)
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: link.clicked()
    }
  }

  component BladeColumn: Column {
    id: section
    required property string bladeEdge
    readonly property var bladeData: root.host.bladeFor(bladeEdge)
    readonly property var bladeSlots: Array.isArray(bladeData.slots) ? bladeData.slots : []
    readonly property bool windowMode: root.host.isWindowMode(bladeEdge)
    readonly property string scope: root.edgeTitle(bladeEdge) + " " + bladeEdge + " blade "
    readonly property bool headerShown: root.matches(scope + "open closed width " + bladeData.width + " px")
    readonly property bool windowShown: root.matches(scope + "window docked tiled float")
    readonly property var slotLabels: {
      var labels = []
      for (var i = 0; i < bladeSlots.length; i++) {
        var tabs = root.host.slotTabs(bladeEdge, i)
        for (var j = 0; j < tabs.length; j++)
          labels.push(scope + "slot tab module " + root.moduleLabel(root.host.slotModuleAt(bladeEdge, i, j)))
      }
      return labels
    }

    function tabLabel(slotIndex, tabIndex) {
      return scope + "slot tab module " + root.moduleLabel(root.host.slotModuleAt(bladeEdge, slotIndex, tabIndex))
    }

    function rows() {
      var result = []
      for (var i = 0; i < slotGroups.count; i++) {
        var group = slotGroups.itemAt(i)
        if (!group || !group.visible) continue
        for (var k = 0; k < group.children.length; k++) {
          var row = group.children[k]
          if (row && row.moduleId !== undefined && row.visible) result.push({ item: row, edge: bladeEdge, slotIndex: i, tabIndex: row.index })
        }
      }
      return result
    }
    readonly property var addLabels: {
      var labels = []
      var groups = root.availableModules()
      for (var i = 0; i < groups.length; i++)
        for (var j = 0; j < groups[i].modules.length; j++) labels.push(root.addLabel(scope, groups[i].modules[j]))
      return labels
    }
    readonly property int slotsShown: root.countMatching(slotLabels)
    readonly property int addsShown: root.countMatching(addLabels)
    readonly property bool shown: headerShown || windowShown || slotsShown > 0 || addsShown > 0
    spacing: Style.space(3)

    PluginUi.ToggleRow {
      width: parent.width
      visible: section.headerShown
      heading: true
      label: root.edgeTitle(section.bladeEdge)
      detail: section.bladeData.width + " px"
      checked: !!section.bladeData.open
      onToggled: root.host.toggleOpen(section.bladeEdge)
    }

    PluginUi.ToggleRow {
      width: parent.width
      visible: section.windowShown
      glyph: section.windowMode ? "󰖲" : "󰁍"
      label: "Window"
      detail: section.windowMode ? "tiled" : "docked"
      checked: section.windowMode
      onToggled: root.host.toggleDock(section.bladeEdge)
    }

    Repeater {
      id: slotGroups
      model: section.bladeSlots.length

      delegate: Column {
        id: slotGroup
        required property int index
        readonly property var tabs: root.host.slotTabs(section.bladeEdge, index)
        readonly property bool anyShown: {
          for (var j = 0; j < tabs.length; j++)
            if (root.matches(section.tabLabel(index, j))) return true
          return false
        }
        width: parent ? parent.width : 0
        spacing: Style.space(2)
        visible: anyShown

        Item {
          width: parent.width
          height: Style.space(9)
          visible: slotGroup.index > 0
          Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            height: 1
            color: Util.alpha(Color.bar.text, 0.12)
          }
        }

        Repeater {
          id: tabRepeater
          model: slotGroup.tabs.length

          delegate: Rectangle {
            id: tabRow
            required property int index
            readonly property string moduleId: root.host.slotModuleAt(section.bladeEdge, slotGroup.index, index)
            readonly property bool lifted: !!root.drag && root.drag.edge === section.bladeEdge
              && root.drag.slotIndex === slotGroup.index && root.drag.tabIndex === index
            width: parent ? parent.width : 0
            height: Style.space(28)
            visible: root.matches(section.tabLabel(slotGroup.index, index))
            radius: Math.min(Style.cornerRadius, Style.space(4))
            opacity: lifted ? 0.35 : 1
            color: rowHover.containsMouse || lifted ? Util.alpha(Color.bar.text, 0.06) : Util.alpha(Color.bar.text, 0.03)

            MouseArea {
              id: rowHover
              anchors.fill: parent
              hoverEnabled: true
              acceptedButtons: Qt.NoButton
            }

            Text {
              textFormat: Text.PlainText
              id: handle
              anchors.left: parent.left
              anchors.leftMargin: Style.space(4)
              anchors.verticalCenter: parent.verticalCenter
              width: Style.space(14)
              text: "󰇝"
              color: handlePointer.containsMouse || tabRow.lifted ? Color.accent : Color.muted
              horizontalAlignment: Text.AlignHCenter
              font.family: Style.font.family
              font.pixelSize: Typography.body

              MouseArea {
                id: handlePointer
                anchors.fill: parent
                anchors.margins: -Style.space(4)
                hoverEnabled: true
                preventStealing: true
                cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                onPressed: function(mouse) {
                  var point = handlePointer.mapToItem(card, mouse.x, mouse.y)
                  var origin = tabRow.mapToItem(card, 0, 0)
                  root.beginTabDrag(section.bladeEdge, slotGroup.index, tabRow.index, root.moduleLabel(tabRow.moduleId),
                                    root.moduleGlyph(tabRow.moduleId), point.x, point.y, point.x - origin.x, point.y - origin.y, tabRow.width)
                }
                onPositionChanged: function(mouse) {
                  if (!root.drag) return
                  var point = handlePointer.mapToItem(card, mouse.x, mouse.y)
                  root.moveTabDrag(point.x, point.y)
                }
                onReleased: root.finishTabDrag()
                onCanceled: root.cancelTabDrag()
              }
            }

            Text {
              textFormat: Text.PlainText
              id: tabGlyph
              anchors.left: handle.right
              anchors.leftMargin: Style.space(2)
              anchors.verticalCenter: parent.verticalCenter
              width: Style.space(18)
              text: root.moduleGlyph(tabRow.moduleId)
              color: Color.accent
              horizontalAlignment: Text.AlignHCenter
              font.family: Style.font.family
              font.pixelSize: Typography.body
            }

            Text {
              textFormat: Text.PlainText
              anchors.left: tabGlyph.right
              anchors.leftMargin: Style.space(6)
              anchors.right: rowHover.containsMouse ? tabActions.left : parent.right
              anchors.rightMargin: Style.space(6)
              anchors.verticalCenter: parent.verticalCenter
              text: root.moduleLabel(tabRow.moduleId)
              color: Color.bar.text
              elide: Text.ElideRight
              font.family: Style.font.family
              font.pixelSize: Typography.bodySmall
            }

            Row {
              id: tabActions
              anchors.right: parent.right
              anchors.rightMargin: Style.space(4)
              anchors.verticalCenter: parent.verticalCenter
              spacing: Style.space(1)
              opacity: rowHover.containsMouse && !root.drag ? 1 : 0

              Behavior on opacity { NumberAnimation { duration: 90 } }

              IconAction {
                glyph: section.bladeEdge === "left" ? "" : ""
                tooltipText: section.bladeEdge === "left" ? "To right blade" : "To left blade"
                tooltipAction: "Move this tab across"
                onClicked: {
                  var other = section.bladeEdge === "left" ? "right" : "left"
                  root.host.moveSlotTo(section.bladeEdge, slotGroup.index, other, root.host.slots(other).length, tabRow.index)
                }
              }

              IconAction {
                glyph: "×"
                tooltipText: "Remove"
                tooltipAction: "Remove this tab"
                danger: true
                onClicked: root.host.removeTab(section.bladeEdge, slotGroup.index, tabRow.index)
              }
            }
          }
        }
      }
    }

    Repeater {
      model: root.availableModules()

      delegate: Column {
        id: group
        required property var modelData
        readonly property var labels: {
          var result = []
          for (var i = 0; i < modelData.modules.length; i++) result.push(root.addLabel(section.scope, modelData.modules[i]))
          return result
        }
        width: parent ? parent.width : 0
        spacing: Style.space(2)
        topPadding: Style.space(2)
        visible: root.countMatching(labels) > 0

        SectionLabel {
          leftPadding: Style.space(8)
          text: String(group.modelData.category).toUpperCase()
        }

        Flow {
          width: parent.width
          spacing: Style.space(12)
          leftPadding: Style.space(8)

          Repeater {
            model: group.modelData.modules

            delegate: Link {
              required property var modelData
              required property int index
              visible: root.matches(group.labels[index] || "")
              text: "+ " + String(modelData.name)
              onClicked: root.host.addSlot(section.bladeEdge, modelData.id, -1)
            }
          }
        }
      }
    }
  }

  Rectangle {
    anchors.fill: parent
    color: Util.alpha(Color.background, 0.48)

    MouseArea {
      anchors.fill: parent
      onClicked: root.close()
    }
  }

  Rectangle {
    id: card
    readonly property int pad: Style.space(10)
    anchors.top: parent.top
    anchors.topMargin: Style.space(35)
    anchors.right: root.edge === "left" ? parent.right : undefined
    anchors.rightMargin: Style.space(8)
    anchors.left: root.edge === "right" ? parent.left : undefined
    anchors.leftMargin: Style.space(8)
    width: Math.min(parent.width - Style.space(16), Style.space(520))
    height: Math.min(parent.height - Style.space(50), chrome.height + Math.ceil(settingsContent.implicitHeight) + pad * 2)
    radius: Style.cornerRadius
    color: Color.popups.background
    border.width: 1
    border.color: Color.popups.border
    clip: true

    MouseArea {
      anchors.fill: parent
      acceptedButtons: Qt.AllButtons
    }

    Column {
      id: chrome
      anchors.top: parent.top
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.margins: card.pad
      anchors.bottomMargin: 0
      spacing: Style.space(8)

      Item {
        width: parent.width
        height: Style.space(24)

        Text {
          textFormat: Text.PlainText
          anchors.left: parent.left
          anchors.verticalCenter: parent.verticalCenter
          text: "SETTINGS"
          color: Color.bar.text
          font.family: Style.font.family
          font.pixelSize: Typography.bodySmall
          font.weight: Font.DemiBold
          font.letterSpacing: 0.5
        }

        Text {
          textFormat: Text.PlainText
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          text: "×"
          color: closePointer.containsMouse ? Color.bar.text : Color.muted
          font.family: Style.font.family
          font.pixelSize: Typography.title

          MouseArea {
            id: closePointer
            anchors.fill: parent
            anchors.margins: -Style.space(7)
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.close()
          }
        }
      }

      PluginUi.MenuSearchField {
        id: search
        width: parent.width
        height: Style.space(30)
        prompt: "Search settings"
        textColor: Color.bar.text
        onTextChanged: root.query = text
        onDismissed: {
          if (text !== "") text = ""
          else root.close()
        }
      }

      Item {
        width: parent.width
        height: Style.space(2)
      }
    }

    Flickable {
      id: flick
      anchors.top: chrome.bottom
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.bottom: parent.bottom
      anchors.leftMargin: card.pad
      anchors.rightMargin: card.pad
      anchors.bottomMargin: card.pad
      contentWidth: width
      contentHeight: Math.ceil(settingsContent.implicitHeight)
      interactive: contentHeight > height
      clip: true
      boundsBehavior: Flickable.StopAtBounds
      ScrollBar.vertical: PluginUi.AccentScrollBar { }

      Column {
        id: settingsContent
        width: parent.width
        spacing: Style.space(6)

        Item {
          id: bladesBlock
          readonly property bool sideBySide: width >= Style.space(300)
          readonly property real columnWidth: sideBySide ? Math.floor((width - Style.space(16)) / 2) : width
          readonly property bool shown: leftColumn.shown || rightColumn.shown
          width: parent.width
          height: sideBySide
            ? Math.max(leftColumn.shown ? leftColumn.implicitHeight : 0, rightColumn.shown ? rightColumn.implicitHeight : 0)
            : (leftColumn.shown ? leftColumn.implicitHeight : 0)
              + (leftColumn.shown && rightColumn.shown ? Style.space(6) : 0)
              + (rightColumn.shown ? rightColumn.implicitHeight : 0)
          visible: shown

          BladeColumn {
            id: leftColumn
            bladeEdge: "left"
            x: 0
            y: 0
            width: bladesBlock.columnWidth
            visible: shown
          }

          Rectangle {
            visible: bladesBlock.sideBySide && leftColumn.shown && rightColumn.shown
            x: bladesBlock.columnWidth + Style.space(8) - 1
            y: 0
            width: 1
            height: bladesBlock.height
            color: Util.alpha(Color.bar.text, 0.12)
          }

          BladeColumn {
            id: rightColumn
            bladeEdge: "right"
            x: bladesBlock.sideBySide ? bladesBlock.width - bladesBlock.columnWidth : 0
            y: bladesBlock.sideBySide ? 0 : (leftColumn.shown ? leftColumn.implicitHeight + Style.space(6) : 0)
            width: bladesBlock.columnWidth
            visible: shown
          }
        }

        Column {
          id: general
          readonly property bool animateShown: root.matches("general animate blades motion")
          readonly property bool monitorsShown: root.matches("general monitors screens display active primary all")
          readonly property bool managementShown: root.matches("general manage agent files skills memory permissions")
          readonly property bool fontScaleShown: root.matches("general font size text scale typography percent")
          readonly property bool shown: animateShown || monitorsShown || managementShown || fontScaleShown
          width: parent.width
          spacing: Style.space(5)
          visible: shown

          Divider { visible: bladesBlock.shown }

          SectionLabel { text: "GENERAL" }

          PluginUi.ToggleRow {
            width: parent.width
            visible: general.managementShown
            glyph: "󰚩"
            label: "Manage agent files"
            checked: root.host.service.agentManagementEnabled
            onToggled: {
              if (checked) root.host.service.preferences.setAgentManagement(false)
              else managementConsent.open("Allow Skills and Memory changes?\nFileBlade can create or remove agent links and disable, trash or restore skill folders and instruction files. These files can change what coding agents do. Browsing stays available when this is off.",
                [{ key: "cancel", label: "Cancel" }, { key: "enable", label: "Allow management" }])
            }
          }

          PluginUi.ToggleRow {
            width: parent.width
            visible: general.animateShown
            glyph: "󰑮"
            label: "Animate blades"
            checked: root.host.animateBlades
            onToggled: root.host.setAnimateBlades(!root.host.animateBlades)
          }

          PluginUi.DropdownRow {
            width: parent.width
            visible: general.monitorsShown
            glyph: "󰍹"
            label: "Monitors"
            prompt: "Find monitor…"
            options: MonitorMode.choices(root.host.screenNames)
            value: MonitorMode.choiceKey(root.host.monitorMode, root.host.monitorLock)
            onChosen: function(key) { var choice = MonitorMode.parseChoice(key); root.host.setMonitorMode(choice.mode, choice.lock) }
          }

          PluginUi.NumberRow {
            width: parent.width
            visible: general.fontScaleShown
            row: ({ key: "fontScale", type: "integer", label: "Font size %", glyph: "󰛖",
                    min: Typography.minimumPercent, max: Typography.maximumPercent,
                    step: Typography.percentStep, defaultValue: 100, value: Typography.percent })
            onCommitted: function(value) { root.host.setFontScale(Typography.scaleFromPercent(value)) }
          }
        }

        Repeater {
          id: moduleRepeater
          model: root.visible ? root.slotSettings() : []
          onItemAdded: root.recount()
          onItemRemoved: root.recount()

          delegate: BladeModuleSection {
            sheet: root
            dividerShown: bladesBlock.shown || general.shown || index > 0
          }
        }

        Column {
          id: footer
          readonly property bool layoutShown: root.matches("layout file config json " + root.host.layoutPath)
          readonly property bool shortcutsShown: root.matches("shortcuts keys keyboard help ?")
          readonly property bool revertShown: root.matches("revert reset default settings layouts")
          readonly property bool shown: layoutShown || shortcutsShown || revertShown
          width: parent.width
          spacing: Style.space(5)
          visible: shown

          Divider { visible: bladesBlock.shown || general.shown || root.moduleShown > 0 }

          PluginUi.HintLine {
            width: parent.width
            visible: footer.layoutShown
            glyph: "󰈔"
            text: root.host.layoutPath
          }

          PluginUi.HintLine {
            id: shortcutsLine
            width: parent.width
            visible: footer.shortcutsShown
            shortcut: "?"
            text: "Shortcuts"
            foreground: shortcutsPointer.containsMouse ? Color.accent : Color.bar.text

            MouseArea {
              id: shortcutsPointer
              anchors.fill: parent
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onClicked: root.openShortcuts()
            }
          }

          Link {
            visible: footer.revertShown
            text: "Revert to default settings"
            onClicked: root.confirmRevert()
          }
        }

        Text {
          textFormat: Text.PlainText
          width: parent.width
          visible: root.filtering && !bladesBlock.shown && !general.shown && root.moduleShown === 0 && !footer.shown
          text: "No setting matches \"" + root.query.trim() + "\""
          color: Color.muted
          elide: Text.ElideRight
          font.family: Style.font.family
          font.pixelSize: Typography.caption
        }
      }
    }
    PluginUi.ActionDialog {
      id: revertDialog
      anchors.fill: parent
      z: 40
      onChosen: function(key) { if (key === "revert") root.host.revertDefaults() }
    }

    Rectangle {
      visible: !!root.dropTarget
      x: root.dropTarget ? root.dropTarget.x : 0
      y: root.dropTarget ? root.dropTarget.y - 1 : 0
      width: root.dropTarget ? root.dropTarget.width : 0
      height: 2
      color: Color.accent
      z: 5

      Text {
        textFormat: Text.PlainText
        visible: !!root.dropTarget && root.dropTarget.newSlot
        anchors.right: parent.right
        anchors.bottom: parent.top
        anchors.bottomMargin: Style.space(1)
        text: "new slot"
        color: Color.accent
        font.family: Style.font.family
        font.pixelSize: Typography.caption
      }
    }

    Rectangle {
      visible: !!root.drag
      x: root.drag ? root.drag.x - root.drag.grabX : 0
      y: root.drag ? root.drag.y - root.drag.grabY : 0
      width: root.drag ? root.drag.width : 0
      height: Style.space(28)
      radius: Math.min(Style.cornerRadius, Style.space(4))
      color: Color.popups.background
      border.width: 1
      border.color: Color.accent
      opacity: 0.92
      z: 6

      Text {
        textFormat: Text.PlainText
        anchors.left: parent.left
        anchors.leftMargin: Style.space(8)
        anchors.verticalCenter: parent.verticalCenter
        text: (root.drag ? root.drag.glyph + "  " + root.drag.title : "")
        color: Color.bar.text
        font.family: Style.font.family
        font.pixelSize: Typography.bodySmall
      }
    }
  }
}
