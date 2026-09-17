import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import qs.Commons
import qs.Ui
import "../ui" as PluginUi
import "../lib/PathText.js" as PathText
import "../theme"

Popup {
  id: root

  required property var controller
  required property var hostWindow
  property alias actionKeys: menuKeys
  PluginUi.ActionKeyGuard { id: menuKeys; active: root.visible; shared: root.hostWindow ? root.hostWindow.actionKeys : null }

  function focusCurrent() {
    if (!visible) return
    if (inputMode) {
      actionField.forceActiveFocus()
      actionField.selectAll()
    } else root.focusFirst(false)
  }

  readonly property bool customColorMode: controller.actionMenuMode === "custom-color"
  readonly property bool inputMode: ["rename", "new-file", "new-folder", "custom-color"].indexOf(controller.actionMenuMode) >= 0
  readonly property bool cancelOperationMode: controller.actionMenuMode === "cancel-operation"
  readonly property bool openWithMode: controller.actionMenuMode === "open-with"
  readonly property bool actionsMode: !inputMode && !cancelOperationMode && !openWithMode
  readonly property string targetPath: String(controller.actionMenuPath || controller.selectedPath || "")
  readonly property var entry: controller.actionMenuEntry || controller.selectedMetadata
  readonly property var paths: controller.actionMenuPaths.length ? controller.actionMenuPaths : controller.selectedPaths
  readonly property var entries: controller.actionMenuEntries.length ? controller.actionMenuEntries : controller.selectedEntries
  readonly property int targetCount: paths.length
  readonly property bool archivesOnly: root.paths.length > 0 && root.paths.every(function(path) { return /\.(zip|tar|tgz|tbz2|txz|tzst|gz|bz2|xz|zst|7z|rar)$/i.test(String(path)) })
  readonly property int targetFolderCount: entries.filter(function(entry) { return entry.isDir }).length
  readonly property int targetColorableCount: entries.filter(function(entry) { return !entry.gitDeleted }).length
  readonly property bool entriesAllFolders: targetColorableCount > 0 && targetFolderCount === targetColorableCount
  readonly property string targetFolderColor: {
    var value = null
    for (var i = 0; i < entries.length; i++) {
      if (entries[i].gitDeleted) continue
      var current = controller.folderColor(entries[i].path)
      if (value === null) value = current
      else if (value !== current) return "mixed"
    }
    return value === null ? "" : value
  }
  readonly property string targetDestination: entry && (entry.is_dir || entry.isDir)
    ? targetPath : controller.parentDirectory(targetPath)
  readonly property int menuAvailableHeight: hostWindow && hostWindow.screen
    ? Math.max(Style.space(180), hostWindow.screen.height - Style.space(28))
    : Style.space(720)
  readonly property int menuGap: Style.space(4)
  readonly property bool dockedHost: hostWindow && hostWindow.windowMode === false
  readonly property int hostOriginX: dockedHost ? Number(hostWindow.surfaceOriginX) || 0 : 0
  readonly property int hostOriginY: dockedHost ? Number(hostWindow.surfaceOriginY) || 0 : 0
  readonly property int screenWidth: hostWindow && hostWindow.screen ? hostWindow.screen.width : Style.space(1280)
  readonly property int screenHeight: hostWindow && hostWindow.screen ? hostWindow.screen.height : Style.space(720)
  readonly property int anchorX: Number(controller.actionMenuX) || 0
  readonly property int anchorY: hostOriginY + (Number(controller.actionMenuY) || 0)
  readonly property bool fitsLeft: anchorX - implicitWidth - menuGap >= 1
  readonly property bool fitsRight: anchorX + menuGap + implicitWidth <= screenWidth - 1
  readonly property bool opensLeft: controller.actionMenuOpenLeft ? (fitsLeft || !fitsRight) : !fitsRight && fitsLeft
  readonly property int menuHeight: Math.min(menuAvailableHeight, Math.max(Style.space(110), menuContent.implicitHeight + Style.space(20)))
  readonly property bool opensUp: anchorY + menuGap + menuHeight > screenHeight - 1 && anchorY - menuHeight - menuGap >= 1
  readonly property int menuX: Math.max(1, Math.min(
    opensLeft ? anchorX - implicitWidth - menuGap : anchorX + menuGap, screenWidth - implicitWidth - 1
  ))
  readonly property int menuY: Math.max(1, Math.min(
    opensUp ? anchorY - menuHeight - menuGap : anchorY + menuGap, screenHeight - menuHeight - 1
  ))
  readonly property string ownerEdge: hostWindow && hostWindow.edge ? String(hostWindow.edge) : "left"
  readonly property string requestedEdge: controller.actionMenuOpenLeft ? "right" : "left"
  readonly property bool requestedVisible: hostWindow && hostWindow.surfaceActive !== false
    && ownerEdge === requestedEdge
    && controller.actionMenuVisibleFor(hostWindow.screen)
  readonly property bool hostActive: hostWindow && hostWindow.contentItem ? hostWindow.contentItem.Window.active : true

  parent: hostWindow ? hostWindow.contentItem : null
  visible: requestedVisible
  x: menuX - hostOriginX
  y: menuY - hostOriginY
  implicitWidth: Style.space(300)
  width: implicitWidth
  height: menuHeight
  padding: 0
  margins: 1
  focus: true
  closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
  background: null

  onClosed: if (requestedVisible) controller.closeActionMenu()
  onHostActiveChanged: if (!hostActive && visible) controller.closeActionMenu()

  function returnToTree() {
    controller.closeActionMenu()
    controller.focusTree(null, true)
  }

  function collectFocusable(item, ancestorVisible, result) {
    if (!item) return
    var showing = ancestorVisible && item.visible
    if (!showing) return
    if (item.menuFocusable === true && item.enabled) result.push(item)
    var children = item.children || []
    for (var i = 0; i < children.length; i++) collectFocusable(children[i], showing, result)
  }

  function focusableItems() {
    var items = []
    collectFocusable(menuContent, true, items)
    items.sort(function(left, right) {
      var leftPoint = left.mapToItem(menuContent, 0, 0)
      var rightPoint = right.mapToItem(menuContent, 0, 0)
      var vertical = Number(leftPoint.y) - Number(rightPoint.y)
      return Math.abs(vertical) > 1 ? vertical : Number(leftPoint.x) - Number(rightPoint.x)
    })
    return items
  }

  function revealFocusedItem(item) {
    if (!item || !menuFlickable) return
    var point = item.mapToItem(menuContent, 0, 0)
    var top = Number(point.y)
    var bottom = top + Number(item.height || 0)
    var maximum = Math.max(0, menuFlickable.contentHeight - menuFlickable.height)
    if (top < menuFlickable.contentY)
      menuFlickable.contentY = Math.max(0, top)
    else if (bottom > menuFlickable.contentY + menuFlickable.height)
      menuFlickable.contentY = Math.min(maximum, bottom - menuFlickable.height)
  }

  function focusItem(item) {
    if (!item) return false
    item.forceActiveFocus()
    Qt.callLater(function() { root.revealFocusedItem(item) })
    return true
  }

  function focusFirst(reverse) {
    var items = focusableItems()
    if (items.length === 0) return false
    return focusItem(items[reverse ? items.length - 1 : 0])
  }

  function focusRelative(current, direction) {
    var items = focusableItems()
    if (items.length === 0) return false
    var index = items.indexOf(current)
    if (index < 0) index = direction < 0 ? 0 : -1
    index = (index + direction + items.length) % items.length
    return focusItem(items[index])
  }

  property string query: ""

  function matches(text) {
    var needle = String(query || "").trim().toLowerCase()
    if (needle === "") return true
    return String(text || "").replace(/^[^A-Za-z0-9]+/, "").toLowerCase().indexOf(needle) >= 0
  }

  function activateFirstMatch() {
    var items = focusableItems()
    for (var i = 0; i < items.length; i++) {
      var item = items[i]
      if (item === searchField || !item.visible || !item.enabled || item.text === undefined) continue
      item.clicked()
      return true
    }
    return false
  }

  function prepare() {
    if (!visible) return
    root.query = ""
    if (controller.actionMenuMode === "rename")
      controller.actionInput = controller.rootName(root.targetPath)
    else if (root.customColorMode)
      controller.actionInput = /^#[0-9a-fA-F]{6}$/.test(root.targetFolderColor)
        ? root.targetFolderColor : "#"
    else if (controller.actionMenuMode === "new-file" || controller.actionMenuMode === "new-folder")
      controller.actionInput = ""
    Qt.callLater(root.focusCurrent)
  }

  function submitInput() {
    var value = PathText.pathText(controller.actionInput)
    if (!value) return
    if (root.customColorMode) {
      var color = value.trim()
      if (!/^#[0-9a-fA-F]{6}$/.test(color)) {
        controller.operationError = "Use a six-digit hex color such as #7aa2f7"
        return
      }
      if (controller.setSelectionFolderColor(color, root.entries)) root.returnToTree()
    } else if (controller.actionMenuMode === "rename") controller.renameSelection(value, root.targetPath)
    else controller.createEntry(value, controller.actionMenuMode === "new-folder", root.targetDestination)
  }

  function isPaletteColor(value) {
    var target = String(value || "").toLowerCase()
    for (var i = 0; i < controller.folderColorChoices.length; i++) {
      if (String(controller.folderColorChoices[i].value || "").toLowerCase() === target) return true
    }
    return false
  }

  onVisibleChanged: {
    if (visible) prepare()
  }

  Connections {
    target: controller
    function onActionMenuModeChanged() { root.prepare() }
    function onApplicationsBusyChanged() {
      if (root.visible && root.openWithMode && !controller.applicationsBusy) root.prepare()
    }
  }

  component MoreColors: Item {
    id: more
    property bool custom: false
    property string swatch: ""
    signal chosen()

    implicitWidth: moreLabel.implicitWidth + Style.space(6)
    implicitHeight: Style.space(19)
    activeFocusOnTab: visible

    Text {
      textFormat: Text.PlainText
      id: moreLabel
      anchors.centerIn: parent
      text: "…"
      color: more.custom || more.activeFocus || morePointer.containsMouse
        ? (more.custom && more.swatch !== "" ? more.swatch : Color.menu.selectedText)
        : Color.muted
      font.family: Style.font.family
      font.pixelSize: Typography.title
      font.weight: Font.DemiBold
    }

    MouseArea {
      id: morePointer
      anchors.fill: parent
      anchors.margins: -Style.space(2)
      hoverEnabled: true
      preventStealing: true
      cursorShape: Qt.PointingHandCursor
      onEntered: more.forceActiveFocus()
      onClicked: more.chosen()
    }

    Keys.onPressed: function(event) {
      if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
        if (!root.actionKeys.isRepeat(event)) more.chosen()
        event.accepted = true
      }
    }
    Keys.onReleased: function(event) { root.actionKeys.release(event) }
  }

  component PaletteDot: Rectangle {
    id: dot
    required property string label
    required property string value
    property string glyph: ""
    property bool selected: false
    property bool menuFocusable: true
    signal chosen()

    implicitWidth: Style.space(19)
    implicitHeight: implicitWidth
    radius: width / 2
    color: value !== "" ? value : Util.alpha(Color.menu.text, 0.06)
    border.width: selected || activeFocus ? Math.max(2, Style.space(2)) : 1
    border.color: selected || activeFocus ? Color.menu.selectedText : Util.alpha(Color.menu.text, 0.42)
    activeFocusOnTab: visible

    Text {
      textFormat: Text.PlainText
      anchors.centerIn: parent
      text: dot.glyph
      color: Color.menu.text
      font.family: Style.font.family
      font.pixelSize: Typography.caption
      font.weight: Font.DemiBold
    }

    MouseArea {
      id: dotPointer
      anchors.fill: parent
      anchors.margins: -Style.space(2)
      hoverEnabled: true
      preventStealing: true
      cursorShape: Qt.PointingHandCursor
      onEntered: dot.forceActiveFocus()
      onClicked: dot.chosen()
    }

    Keys.onPressed: function(event) {
      if (event.key === Qt.Key_Left || event.key === Qt.Key_Up
          || event.key === Qt.Key_Right || event.key === Qt.Key_Down) {
        root.focusRelative(dot, event.key === Qt.Key_Left || event.key === Qt.Key_Up ? -1 : 1)
        event.accepted = true
      } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
        if (!root.actionKeys.isRepeat(event)) dot.chosen()
        event.accepted = true
      }
    }
    Keys.onReleased: function(event) { root.actionKeys.release(event) }

    Keys.onTabPressed: function(event) {
      root.focusRelative(dot, 1)
      event.accepted = true
    }

    Keys.onBacktabPressed: function(event) {
      root.focusRelative(dot, -1)
      event.accepted = true
    }

    PanelToolTip {
      visible: dotPointer.containsMouse || dot.activeFocus
      text: dot.label
    }
  }

  FocusScope {
    id: menuScope
    anchors.fill: parent
    focus: root.visible

    Keys.onEscapePressed: function(event) {
      if (!root.actionKeys.isRepeat(event)) root.returnToTree()
      event.accepted = true
    }
    Keys.onReleased: function(event) { root.actionKeys.release(event) }

    Keys.onTabPressed: function(event) {
      root.focusFirst(false)
      event.accepted = true
    }

    Keys.onBacktabPressed: function(event) {
      root.focusFirst(true)
      event.accepted = true
    }

    Rectangle {
      id: card
      anchors.fill: parent
      radius: Style.cornerRadius
      color: Color.bar.background
      border.width: 1
      border.color: Color.menu.border
      clip: true

      HoverHandler {
        enabled: root.visible
        onHoveredChanged: if (!hovered) root.hostWindow.host.actionMenuPointerExited()
      }

      Flickable {
        id: menuFlickable
        anchors.fill: parent
        anchors.margins: Style.space(8)
        contentWidth: width
        contentHeight: menuContent.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentHeight > height
        flickableDirection: Flickable.VerticalFlick
        ScrollBar.vertical: PluginUi.AccentScrollBar { }

        Column {
          id: menuContent
          width: parent.width
          spacing: Style.space(5)

          Item {
            width: parent.width
            height: Style.space(30)

            Text {
              textFormat: Text.PlainText
              anchors.left: parent.left
              anchors.right: closeGlyph.left
              anchors.rightMargin: Style.space(8)
              anchors.verticalCenter: parent.verticalCenter
              text: {
                if (root.openWithMode) return "OPEN WITH"
                if (root.cancelOperationMode) return "STOP " + (controller.operationLabel || "OPERATION").toUpperCase()
                if (root.customColorMode) return root.entriesAllFolders ? "CUSTOM FOLDER COLOR" : "CUSTOM COLOR"
                if (root.inputMode) return controller.actionMenuMode === "rename" ? "RENAME" : "CREATE"
                return root.targetCount + " ITEM" + (root.targetCount === 1 ? "" : "S")
              }
              color: Color.bar.text
              elide: Text.ElideRight
              font.family: Style.font.family
              font.pixelSize: Typography.bodySmall
              font.weight: Font.DemiBold
              font.letterSpacing: 0.5
            }

            Text {
              textFormat: Text.PlainText
              id: closeGlyph
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
                onClicked: root.returnToTree()
              }
            }
          }

          PluginUi.MenuSearchField {
            id: searchField
            actionKeys: root.actionKeys
            property bool menuFocusable: true
            width: parent.width
            height: root.actionsMode || root.openWithMode ? Style.space(32) : 0
            visible: root.actionsMode || root.openWithMode
            focus: visible
            prompt: root.openWithMode ? "Filter applications, Enter opens" : "Filter actions, Enter runs"
            textColor: Color.bar.text
            text: root.query
            onTextEdited: root.query = text
            onPicked: root.activateFirstMatch()
            onDismissed: root.returnToTree()
            onMoved: function(delta) { root.focusRelative(searchField, delta) }
            onEdgeRequested: function(last) { root.focusFirst(last) }
            Keys.onTabPressed: function(event) {
              root.focusRelative(searchField, 1)
              event.accepted = true
            }
            Keys.onBacktabPressed: function(event) {
              root.focusRelative(searchField, -1)
              event.accepted = true
            }
          }

          Text {
            textFormat: Text.PlainText
            width: parent.width
            visible: root.inputMode
            text: controller.actionMenuMode === "rename"
              ? "Choose a new name for " + controller.rootName(root.targetPath)
                + (PathText.nameNeedsEscaping(root.targetPath) ? "\nEscapes preserve bytes: \\xFF, \\n, \\\\." : "")
              : "Created inside " + root.targetDestination
            color: Color.muted
            elide: Text.ElideMiddle
            font.family: Style.font.family
            font.pixelSize: Typography.caption
          }

          Text {
            textFormat: Text.PlainText
            width: parent.width
            visible: root.customColorMode
            text: "Enter a six-digit hex color for " + root.targetColorableCount + " selected item"
              + (root.targetColorableCount === 1 ? "." : "s.")
            color: Color.muted
            wrapMode: Text.WordWrap
            font.family: Style.font.family
            font.pixelSize: Typography.caption
          }

          TextField {
            id: actionField
            property bool menuFocusable: true
            focus: root.inputMode
            width: parent.width
            height: root.inputMode ? Style.space(34) : 0
            visible: root.inputMode
            leftPadding: Style.space(9)
            rightPadding: Style.space(9)
            selectByMouse: true
            text: controller.actionInput
            placeholderText: root.customColorMode ? "#7aa2f7" : ""
            placeholderTextColor: Color.muted
            color: Color.bar.text
            selectionColor: Util.alpha(Color.accent, 0.38)
            selectedTextColor: Color.bar.text
            font.family: Style.font.family
            font.pixelSize: Typography.body
            background: Rectangle {
              radius: Math.min(Style.cornerRadius, Style.space(4))
              color: Util.alpha(Color.bar.text, actionField.activeFocus ? 0.10 : 0.06)
              border.width: 1
              border.color: actionField.activeFocus ? Color.accent : Util.alpha(Color.bar.text, 0.18)
            }
            onTextEdited: controller.actionInput = text
            Keys.onEscapePressed: function(event) {
              if (!root.actionKeys.isRepeat(event)) root.returnToTree()
              event.accepted = true
            }
            Keys.onTabPressed: function(event) {
              root.focusRelative(actionField, 1)
              event.accepted = true
            }
            Keys.onBacktabPressed: function(event) {
              root.focusRelative(actionField, -1)
              event.accepted = true
            }
            Keys.priority: Keys.BeforeItem
            Keys.onPressed: function(event) {
              if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                if (!root.actionKeys.isRepeat(event)) root.submitInput()
                event.accepted = true
              } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
                root.focusRelative(actionField, event.key === Qt.Key_Up ? -1 : 1)
                event.accepted = true
              }
            }
            Keys.onReleased: function(event) { root.actionKeys.release(event) }
          }

          Row {
            width: parent.width
            visible: root.inputMode || root.cancelOperationMode
            spacing: Style.space(6)

            MenuButton {
              menu: root
              text: "Cancel"
              width: (parent.width - parent.spacing) / 2
              onClicked: root.returnToTree()
            }

            MenuButton {
              menu: root
              text: root.cancelOperationMode ? "Stop operation"
                : (root.customColorMode ? "Apply color"
                  : (controller.actionMenuMode === "rename" ? "Rename" : "Create"))
              width: (parent.width - parent.spacing) / 2
              primary: !root.cancelOperationMode
              danger: root.cancelOperationMode
              enabled: root.cancelOperationMode ? controller.operationCancellable
                : controller.actionInput.trim() !== ""
              onClicked: {
                if (root.cancelOperationMode) {
                  controller.cancelOperation(controller.activeOperationId)
                  root.returnToTree()
                } else root.submitInput()
              }
            }
          }

          Text {
            textFormat: Text.PlainText
            width: parent.width
            visible: root.cancelOperationMode
            text: "Completed items stay completed. The item currently in progress may leave a partial destination; the tree will refresh immediately so it is visible."
            color: Color.urgent
            wrapMode: Text.WordWrap
            font.family: Style.font.family
            font.pixelSize: Typography.bodySmall
          }

          Text {
            textFormat: Text.PlainText
            width: parent.width
            visible: root.openWithMode && controller.applicationsBusy
            text: "Finding compatible applications…"
            color: Color.muted
            font.family: Style.font.family
            font.pixelSize: Typography.bodySmall
          }

          Repeater {
            id: applicationRepeater
            model: root.openWithMode ? controller.applicationModel : null

            delegate: MenuButton {
              menu: root
              required property string desktop_id
              required property string name
              required property string icon
              required property bool is_default
              appIcon: icon
              text: name + (is_default ? "  — default" : "")
              visible: root.matches(name)
              onClicked: controller.openWithApplication(desktop_id, root.targetPath)
            }
          }

          MenuButton {
            menu: root
            visible: root.matches(text) && root.openWithMode
            text: (controller.rememberApplicationDefault ? "  " : "  ") + "Remember for " + (controller.applicationsMime || "this file type")
            enabled: !controller.applicationsBusy && controller.applicationModel.count > 0
            onClicked: controller.rememberApplicationDefault = !controller.rememberApplicationDefault
          }

          PluginUi.ErrorNotice {
            width: parent.width
            visible: root.openWithMode && controller.applicationsError !== ""
            text: controller.applicationsError
            onDismissed: controller.applicationsError = ""
          }

          MenuButton {
            menu: root
            id: firstAction
            visible: root.matches(text) && root.actionsMode
            shortcut: "Enter"
            text: root.targetCount === 1 && root.entry && root.entry.is_dir ? "󰉋  Open folder" : "󰏌  Open"
            enabled: root.targetCount === 1
            onClicked: {
              var directory = root.entry ? !!root.entry.is_dir : undefined
              controller.openDefault(root.targetPath, controller.actionMenuScreen, directory)
              if (directory === true) root.returnToTree()
            }
          }

          MenuButton {
            menu: root
            visible: root.matches(text) && root.actionsMode
            shortcut: "Shift+Enter"
            text: "󰝰  Open with…"
            enabled: root.targetCount === 1 && root.entry && !root.entry.is_dir
            onClicked: controller.openActionMenu("open-with")
          }

          MenuButton {
            menu: root
            visible: root.matches(text) && root.actionsMode
            shortcut: "e"
            text: "  Open in LazyVim"
            enabled: root.targetCount === 1
            onClicked: controller.openInEditor(root.targetPath)
          }

          MenuButton {
            menu: root
            visible: root.matches(text) && root.actionsMode
            shortcut: "y  Ctrl+C"
            text: "󰆏  Copy"
            enabled: root.targetCount > 0
            onClicked: controller.copySelection(false, root.paths)
          }

          MenuButton {
            menu: root
            visible: root.matches(text) && root.actionsMode
            shortcut: "x  Ctrl+X"
            text: "  Cut"
            enabled: root.targetCount > 0
            onClicked: controller.copySelection(true, root.paths)
          }

          MenuButton {
            menu: root
            visible: root.matches(text) && root.actionsMode
            text: root.targetCount === 1 ? "󰅍  Copy path" : "󰅍  Copy paths"
            enabled: root.targetCount > 0
            onClicked: controller.copyPaths(root.paths)
          }

          MenuButton {
            menu: root
            visible: root.matches(text) && root.actionsMode && root.archivesOnly
            text: "  Extract here"
            enabled: root.targetCount > 0
            onClicked: controller.extractArchives(root.paths)
          }

          MenuButton {
            menu: root
            visible: root.matches(text) && root.actionsMode
            shortcut: "p  Ctrl+V"
            text: "  Paste here" + (controller.pasteCount > 0
              ? " (" + controller.pasteCount + ")"
              : (controller.externalClipboardBusy ? "…" : ""))
            enabled: controller.pasteReady && !controller.externalClipboardBusy
            onClicked: controller.pasteInto(root.targetDestination)
          }

          MenuButton {
            menu: root
            visible: root.matches("Undo") && root.actionsMode && controller.history.undoCount > 0
            shortcut: "u  Ctrl+Z"
            text: "󰕌  Undo " + controller.history.undoLabel
            onClicked: controller.history.undoOperation()
          }

          MenuButton {
            menu: root
            visible: root.matches("Redo") && root.actionsMode && controller.history.redoCount > 0
            shortcut: "Ctrl+Shift+Z"
            text: "󰑎  Redo " + controller.history.redoLabel
            onClicked: controller.history.redoOperation()
          }

          MenuButton {
            menu: root
            visible: root.matches(text) && root.actionsMode
            shortcut: "r  F2"
            text: "󰝒  Rename…"
            enabled: root.targetCount === 1
            onClicked: controller.openActionMenu("rename")
          }

          MenuButton {
            menu: root
            visible: root.matches(text) && root.actionsMode
            shortcut: "a  Ctrl+N"
            text: "  New file…"
            onClicked: controller.openActionMenu("new-file")
          }

          MenuButton {
            menu: root
            visible: root.matches(text) && root.actionsMode
            shortcut: "Ctrl+Shift+N"
            text: "  New folder…"
            onClicked: controller.openActionMenu("new-folder")
          }

          MenuButton {
            menu: root
            visible: root.matches(text) && root.actionsMode
            shortcut: "d  Del"
            text: "  Move to Trash…"
            danger: true
            enabled: root.targetCount > 0
            onClicked: controller.requestTrash(root.paths)
          }

          ScriptActionRows {
            width: parent.width
            visible: root.actionsMode && rows.length > 0
            controller: root.controller
            menu: root
          }

          Column {
            width: parent.width
            visible: root.actionsMode && root.targetColorableCount > 0
            spacing: Style.space(5)

            Rectangle {
              width: parent.width
              height: 1
              color: Util.alpha(Color.menu.text, 0.14)
            }

            Text {
              textFormat: Text.PlainText
              width: parent.width
              text: (root.entriesAllFolders ? "FOLDER COLOR" : "COLOR") + "   ↓ ← →  ENTER"
              color: Color.muted
              font.family: Style.font.family
              font.pixelSize: Typography.caption
              font.weight: Font.DemiBold
              font.letterSpacing: 0.4
            }

            RowLayout {
              width: parent.width
              spacing: Style.space(4)

              PaletteDot {
                label: "Default (theme)"
                value: ""
                glyph: "↺"
                selected: root.targetFolderColor === ""
                onChosen: {
                  controller.setSelectionFolderColor("", root.entries)
                  root.returnToTree()
                }
              }

              Repeater {
                model: controller.folderColorChoices.slice(1)

                delegate: PaletteDot {
                  required property var modelData
                  label: modelData.label
                  value: modelData.value
                  selected: String(root.targetFolderColor).toLowerCase() === String(modelData.value).toLowerCase()
                  onChosen: {
                    controller.setSelectionFolderColor(modelData.value, root.entries)
                    root.returnToTree()
                  }
                }
              }

              Item { Layout.fillWidth: true }

              MoreColors {
                custom: root.targetFolderColor !== "" && !root.isPaletteColor(root.targetFolderColor)
                swatch: custom ? root.targetFolderColor : ""
                onChosen: controller.openActionMenu("custom-color")
              }
            }
          }

          PluginUi.ErrorNotice {
            width: parent.width
            text: controller.operationError
            onDismissed: controller.operationError = ""
          }
        }
      }
    }
  }
}
