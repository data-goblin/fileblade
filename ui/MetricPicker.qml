import QtQuick
import QtQuick.Window
import qs.Commons
import "../theme"

Item {
  id: picker

  property var view: null
  property int columnIndex: 0
  property int triggerWidth: Style.space(66)
  property string pinnedKey: ""
  property string pinnedGlyph: ""
  property string optionGroup: ""
  property var detailOptions: []
  property var detailValues: []
  readonly property bool pinned: pinnedKey !== ""
  readonly property bool detailPicker: pinned && Array.isArray(detailOptions) && detailOptions.length > 0
  readonly property bool adder: !pinned && columnIndex < 0
  readonly property var options: view && Array.isArray(view.options) ? view.options : []
  readonly property var columns: view && Array.isArray(view.columns) ? view.columns : []
  readonly property string value: pinned ? pinnedKey : (adder || columnIndex >= columns.length ? "off" : String(columns[columnIndex]))
  readonly property var sorts: view && Array.isArray(view.sorts) ? view.sorts : []
  readonly property var ownSort: view ? view.sortFor(value) : null
  readonly property int sortRank: ownSort ? sorts.indexOf(ownSort) + 1 : 0
  readonly property bool filterActive: view ? view.filterActive : false
  readonly property bool sortedByCurrent: !!ownSort
  readonly property bool popupOpen: popup.opened
  readonly property bool hovered: adder ? pointer.containsMouse : columnHover.hovered
  readonly property var rows: menuRows()
  readonly property bool opensUp: {
    var window = picker.Window.window
    return !!window && picker.mapToItem(null, 0, 0).y > window.height / 2
  }
  property bool dragging: false
  property real pressX: 0

  HoverHandler {
    id: columnHover
    enabled: !picker.adder
  }

  signal chosen(string value)
  signal detailToggled(string key, bool enabled)
  signal dragMoved(real x)
  signal dragFinished(real x)
  signal dragCancelled()

  function optionKey(option) {
    if (option && typeof option === "object") return String(option.key || option.value || "")
    return String(option || "")
  }

  function optionLabel(option) {
    if (option && typeof option === "object") return String(option.label || option.key || option.value || "")
    return String(option || "")
  }

  function optionShortLabel(option) {
    if (option && typeof option === "object")
      return String(option.shortLabel || option.label || option.key || option.value || "").toUpperCase()
    return String(option || "").toUpperCase()
  }

  function optionField(option, name) {
    return option && typeof option === "object" && option[name] ? String(option[name]) : ""
  }

  function currentOption() {
    for (var i = 0; i < options.length; i++)
      if (optionKey(options[i]) === value) return options[i]
    return null
  }

  function sortMarker() {
    if (!ownSort) return ""
    var arrow = ownSort.desc ? "↓" : "↑"
    return " " + arrow + (sorts.length > 1 ? String(sortRank) : "")
  }

  function triggerText() {
    if (pinned) return pinnedGlyph
    if (adder) return "+"
    var text = optionShortLabel(currentOption()) + sortMarker()
    return filterActive && columnIndex === 0 ? "󰈲 " + text : text
  }

  function nextSortText() {
    var descendingByDefault = view.kindOf(value) !== "text"
    if (!ownSort) return descendingByDefault ? "Sort descending" : "Sort ascending"
    if (ownSort.desc === descendingByDefault) return ownSort.desc ? "Sort ascending" : "Sort descending"
    return "Clear sort"
  }

  function tipActions() {
    if (adder) return [{ button: "left", text: "Add a column" }]
    var list = []
    if (view && value !== "off") list.push({ button: "left", text: nextSortText() })
    if (detailPicker) list.push({ button: "right", text: "Git status indicators" })
    else if (!pinned) list.push({ button: "right", text: "Options" })
    if (!pinned && columns.length > 1) list.push({ glyph: "󰁔", text: "Drag to reorder" })
    return list
  }

  function tipContext() {
    var list = []
    for (var i = 0; i < sorts.length; i++)
      list.push({ glyph: sorts[i].desc ? "↓" : "↑", text: (sorts.length > 1 ? (i + 1) + " " : "") + optionLabel(view.optionFor(sorts[i].key) || sorts[i].key) })
    if (filterActive) list.push({ glyph: "󰈲", text: "Filter on" })
    return list
  }

  function metricRows() {
    var list = []
    var group = ""
    var source = detailPicker && Array.isArray(detailOptions) ? detailOptions : options
    for (var i = 0; i < source.length; i++) {
      var key = optionKey(source[i])
      if (key === "off" || key === "none") continue
      if (adder && columns.indexOf(key) >= 0) continue
      if (adder && view && Array.isArray(view.pinnedSortKeys) && view.pinnedSortKeys.indexOf(key) >= 0) continue
      var next = optionField(source[i], "group")
      if (optionGroup !== "" && next !== optionGroup) continue
      if (list.length > 0 && next !== group) list.push({ kind: "separator" })
      group = next
      var checked = detailPicker && Array.isArray(detailValues) ? detailValues.indexOf(key) >= 0 : (pinned ? columns.indexOf(key) >= 0 : (!adder && key === value))
      list.push({ kind: "metric", key: key, label: optionLabel(source[i]), glyph: optionField(source[i], "glyph"), checked: checked })
    }
    return list
  }

  function menuRows() {
    if (pinned && !detailPicker) return []
    var list = metricRows()
    if (adder || detailPicker) return list
    var sortable = view && value !== "off"
    list.push({ kind: "separator" })
    list.push({ kind: "action", key: "sort-asc", label: "Sort ascending", glyph: "󰒼", checked: !!ownSort && !ownSort.desc, enabled: sortable })
    list.push({ kind: "action", key: "sort-desc", label: "Sort descending", glyph: "󰒽", checked: !!ownSort && ownSort.desc, enabled: sortable })
    list.push({ kind: "action", key: "sort-also", label: "Also sort by this", glyph: "⇅", checked: false, enabled: sortable && !ownSort && sorts.length > 0 })
    list.push({ kind: "action", key: "sort-clear", label: "Clear sort", glyph: "󰅖", checked: false, enabled: sorts.length > 0 })
    list.push({ kind: "separator" })
    list.push({ kind: "action", key: "filter", label: "Filter…", glyph: "󰈲", checked: filterActive, enabled: !!view })
    list.push({ kind: "action", key: "filter-clear", label: "Clear filter", glyph: "󰈳", checked: false, enabled: filterActive })
    return list
  }

  function open() {
    if (rows.length === 0) return
    popup.present()
  }

  function openFilter() {
    popup.close()
    filterPopup.open()
  }

  function sortsWith(descending) {
    if (!ownSort) return [{ key: value, desc: descending }]
    return sorts.map(function(item) { return item.key === value ? { key: value, desc: descending } : item })
  }

  function runAction(key) {
    var actions = {
      "sort-asc": function() { view.setSorts(sortsWith(false)) },
      "sort-desc": function() { view.setSorts(sortsWith(true)) },
      "sort-also": function() { view.setSorts(sorts.concat([{ key: value, desc: view.kindOf(value) !== "text" }])) },
      "sort-clear": function() { view.clearSort() },
      "filter": function() { picker.openFilter() },
      "filter-clear": function() { view.clearFilter() }
    }
    if (actions[key]) actions[key]()
  }

  function choose(key, row) {
    if (row.kind === "metric") {
      if (detailPicker) {
        picker.detailToggled(row.key, !row.checked)
      } else if (view && pinned) {
        var present = columns.indexOf(row.key)
        if (present >= 0) view.removeColumn(present)
        else view.addColumn(row.key)
      } else if (view && adder) view.addColumn(row.key)
      else if (view) view.setColumn(columnIndex, row.key)
      picker.chosen(row.key)
    } else runAction(key)
    if (!detailPicker) picker.forceActiveFocus()
  }

  function toggleSort() {
    if (view && !adder) view.toggleSort(value)
  }

  implicitWidth: pinned ? triggerWidth : (adder ? Math.max(triggerWidth, triggerLabel.implicitWidth + Style.space(6)) : triggerWidth)
  implicitHeight: Style.space(24)
  visible: options.length > 0
  activeFocusOnTab: visible

  Keys.onPressed: function(event) {
    var activates = event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space
    var opensMenu = event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && event.modifiers === Qt.ShiftModifier)
    if (activates) {
      if (!picker.view || picker.adder) picker.open()
      else picker.toggleSort()
      event.accepted = true
    } else if (opensMenu) {
      picker.open()
      event.accepted = true
    } else if (event.key === Qt.Key_Escape && popup.opened) {
      popup.close()
      event.accepted = true
    }
  }

  Text {
    id: triggerLabel
    anchors.fill: parent
    textFormat: Text.PlainText
    text: picker.triggerText()
    color: (picker.sortedByCurrent || (picker.filterActive && picker.columnIndex === 0 && !picker.pinned)) && !picker.adder
      ? Color.accent
      : (pointer.containsMouse || picker.activeFocus || picker.dragging ? Color.muted : Util.alpha(Color.bar.text, 0.34))
    horizontalAlignment: Text.AlignHCenter
    verticalAlignment: Text.AlignVCenter
    elide: Text.ElideRight
    font.family: Style.font.family
    font.pixelSize: picker.adder ? Typography.bodySmall : Typography.caption
    font.weight: picker.pinned ? Font.Normal : Font.DemiBold
    font.letterSpacing: picker.pinned ? 0 : 0.4
  }

  Text {
    id: removeGlyph
    visible: !picker.adder && !picker.pinned && picker.hovered
    z: 2
    anchors.left: parent.left
    anchors.leftMargin: Style.space(2)
    anchors.verticalCenter: parent.verticalCenter
    width: Style.space(14)
    height: parent.height
    textFormat: Text.PlainText
    text: "×"
    color: removePointer.containsMouse ? Color.urgent : Color.muted
    horizontalAlignment: Text.AlignHCenter
    verticalAlignment: Text.AlignVCenter
    font.family: Style.font.family
    font.pixelSize: Typography.bodySmall

    MouseArea {
      id: removePointer
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: function(mouse) {
        mouse.accepted = true
        if (picker.view) picker.view.removeColumn(picker.columnIndex)
      }
    }
  }

  MouseArea {
    id: pointer
    anchors.fill: parent
    acceptedButtons: Qt.LeftButton | Qt.RightButton
    hoverEnabled: true
    cursorShape: picker.dragging ? Qt.ClosedHandCursor : Qt.PointingHandCursor
    onPressed: function(mouse) {
      picker.forceActiveFocus()
      picker.pressX = mouse.x
      picker.dragging = false
    }
    onPositionChanged: function(mouse) {
      if (!(mouse.buttons & Qt.LeftButton) || picker.adder || picker.pinned) return
      if (!picker.dragging && Math.abs(mouse.x - picker.pressX) < Style.space(8)) return
      picker.dragging = true
      picker.dragMoved(picker.x + mouse.x)
    }
    onReleased: function(mouse) {
      if (!picker.dragging) return
      picker.dragging = false
      picker.dragFinished(picker.x + mouse.x)
    }
    onCanceled: {
      if (picker.dragging) picker.dragCancelled()
      picker.dragging = false
    }
    onClicked: function(mouse) {
      if (picker.dragging) return
      if (mouse.button === Qt.RightButton) {
        if (!picker.pinned || picker.detailPicker) picker.open()
      } else if (!picker.view || picker.adder) picker.open()
      else picker.toggleSort()
    }
  }

  HintTip {
    visible: pointer.containsMouse && !removePointer.containsMouse && !popup.opened && !filterPopup.opened && !picker.dragging
    title: picker.adder ? "Add column" : picker.optionLabel(picker.currentOption())
    actions: picker.tipActions()
    context: picker.tipContext()
  }

  HintTip {
    visible: !picker.pinned && removePointer.containsMouse && !popup.opened && !filterPopup.opened
    title: "Remove column"
    actions: [{ button: "left", text: "Remove " + picker.optionLabel(picker.currentOption()) }]
  }

  FilterPopup {
    id: filterPopup
    view: picker.view
    x: picker.width - width
    y: picker.opensUp ? -height - Style.space(2) : picker.height + Style.space(2)
    onClosed: picker.forceActiveFocus()
  }

  OptionPopup {
    id: popup
    x: picker.width - width
    y: picker.opensUp ? -height - Style.space(2) : picker.height + Style.space(2)
    rows: picker.rows
    prompt: picker.adder ? "Add column…" : (picker.detailPicker ? "Choose Git status indicators…" : "Type to filter, Enter picks")
    heading: picker.adder ? "Add column" : (picker.detailPicker ? "Git status" : "Columns")
    keepOpenForMetrics: picker.detailPicker
    onPicked: function(key, row) { picker.choose(key, row) }
    onClosed: picker.forceActiveFocus()
  }
}
