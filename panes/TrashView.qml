import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Ui
import "../theme"

FocusScope {
  id: root

  required property var controller
  required property var pane
  required property var actionKeys
  property alias list: trashList
  property string dialogMode: ""
  property string dialogEntryId: ""
  property string dialogEntryName: ""
  property string dialogOriginalParent: ""
  property int dialogFocusIndex: 1
  property string displayedNotice: ""
  property bool noticePresented: false
  readonly property bool animationsEnabled: !pane.context || !pane.context.host
    || pane.context.host.animateBlades !== false
  readonly property bool statusActive: controller.trashOperationBusy
    || (controller.trashBusy && controller.trashCount === 0)
    || controller.trashError !== "" || noticePresented

  function closeDialog() {
    dialogMode = ""
    dialogEntryId = ""
    dialogEntryName = ""
    dialogOriginalParent = ""
    restoreDestination.text = ""
    trashList.forceActiveFocus()
  }

  function confirmDelete(entry) {
    dialogMode = "delete"
    dialogEntryId = String(entry.entryId || "")
    dialogEntryName = String(entry.name || "")
    dialogOriginalParent = String(entry.originalParent || "")
    Qt.callLater(function() { if (root.dialogMode === "delete") root.focusDialog(1) })
  }

  function confirmEmpty() {
    dialogMode = "empty"
    dialogEntryId = ""
    dialogEntryName = ""
    dialogOriginalParent = ""
    Qt.callLater(function() { if (root.dialogMode === "empty") root.focusDialog(1) })
  }

  function restoreTo(entry) {
    dialogMode = "restore-to"
    dialogEntryId = String(entry.entryId || "")
    dialogEntryName = String(entry.name || "")
    dialogOriginalParent = String(entry.originalParent || "")
    restoreDestination.text = dialogOriginalParent
    Qt.callLater(function() {
      if (root.dialogMode !== "restore-to") return
      root.focusDialog(-1)
      restoreDestination.selectAll()
    })
  }

  function focusDialog(index) {
    dialogFocusIndex = index
    if (index === -1) restoreDestination.forceActiveFocus()
    else dialogOverlay.forceActiveFocus()
  }

  function focusList() {
    if (dialogMode !== "") return focusDialog(dialogFocusIndex)
    if (trashList.count > 0 && trashList.currentIndex < 0) {
      trashList.currentIndex = 0
      controller.trashSelectedId = String(trashList.model.get(0).entryId)
    }
    trashList.forceActiveFocus()
  }

  function acceptDialog() {
    if (dialogFocusIndex === 0) closeDialog()
    else if (dialogFocusIndex === 2) {
      controller.restoreTrashEntry(dialogEntryId, "", true)
      closeDialog()
    } else runDialogAction()
  }

  function handleDialogKey(event) {
    event.accepted = true
    var key = event.key
    if (key === Qt.Key_Escape) {
      if (!actionKeys.isRepeat(event)) closeDialog()
    } else if (key === Qt.Key_Return || key === Qt.Key_Enter || key === Qt.Key_Space) {
      if (!actionKeys.isRepeat(event)) acceptDialog()
    } else if (key === Qt.Key_Tab || key === Qt.Key_Backtab || key === Qt.Key_Left || key === Qt.Key_Right) {
      var order = dialogMode === "restore-to" ? [-1, 2, 0, 1] : [0, 1]
      var backwards = key === Qt.Key_Backtab || key === Qt.Key_Left || (event.modifiers & Qt.ShiftModifier)
      var next = (order.indexOf(dialogFocusIndex) + (backwards ? -1 : 1) + order.length) % order.length
      focusDialog(order[next])
    }
  }

  function runDialogAction() {
    if (dialogMode === "delete") controller.trashDeletePermanently(dialogEntryId)
    else if (dialogMode === "empty") controller.emptyTrash()
    else if (dialogMode === "restore-to") {
      var destination = restoreDestination.text.trim()
      if (!destination) return
      controller.restoreTrashEntry(dialogEntryId, destination, false)
    }
    closeDialog()
  }

  function operationStatus() {
    var label = controller.trashOperationLabel || "Trash operation"
    var value = controller.trashProgress
    if (!value || typeof value !== "object") return label + "…"
    var completed = Math.max(0, Number(value.completed) || 0)
    var total = Math.max(0, Number(value.total) || 0)
    if (total > 0) return label + "  " + completed + "/" + total
    return label + "…"
  }

  function syncNotice() {
    var next = String(controller.trashNotice || "")
    if (next !== "") {
      noticeCleanup.stop()
      displayedNotice = next
      noticePresented = true
    } else {
      noticePresented = false
      if (displayedNotice !== "" && animationsEnabled) noticeCleanup.restart()
      else displayedNotice = ""
    }
  }

  Component.onCompleted: syncNotice()

  Connections {
    target: controller
    function onTrashNoticeChanged() { root.syncNotice() }
  }

  Timer {
    id: noticeCleanup
    interval: 160
    onTriggered: if (!root.noticePresented && controller.trashNotice === "") root.displayedNotice = ""
  }

  ListView {
    id: trashList
    anchors.fill: parent
    clip: true
    reuseItems: true
    boundsBehavior: Flickable.StopAtBounds
    model: controller.trashModel
    currentIndex: -1

    onCountChanged: {
      if (count === 0) {
        currentIndex = -1
        controller.trashSelectedId = ""
      } else if (currentIndex >= count) {
        currentIndex = count - 1
        controller.trashSelectedId = String(model.get(currentIndex).entryId)
      } else if (currentIndex >= 0 && controller.trashSelectedId === "") {
        controller.trashSelectedId = String(model.get(currentIndex).entryId)
      }
    }

    delegate: BrowserRow {
      id: trashRow
      required property string entryId
      required property string originalPath
      required property string originalParent
      required property string deletedAt
      required property bool isLink
      required property bool canRestore
      required property bool canRestoreTo
      required property bool canDelete
      required property bool emergency

      readonly property string displayTimestamp: String(deletedAt || "").replace("T", " ")
      readonly property var actionSpecs: [
        { glyph: "󰁯", action: "restore", enabled: canRestore && !controller.trashOperationBusy, danger: false },
        { glyph: "", action: "restore-to", enabled: canRestoreTo && !controller.trashOperationBusy, danger: false },
        { glyph: "󰆴", action: "delete", enabled: canDelete && !controller.trashOperationBusy, danger: true }
      ]

      controller: root.controller
      pane: root.pane
      ownerView: trashList
      treeMode: true
      customInteraction: true
      customSelected: controller.trashSelectedId === entryId
      urgent: emergency
      path: originalPath
      isSymlink: isLink
      isGitRepo: false
      gitDeleted: false
      gitIgnored: false
      gitRepoRoot: ""
      gitStatus: ""
      gitStatusLabel: ""
      gitIndexStatus: ""
      gitWorktreeStatus: ""
      gitOriginalPath: ""
      gitModifiedCount: 0
      gitDeletedCount: 0
      gitNewCount: 0
      gitUntrackedCount: 0
      gitSummary: ""
      gitRepoName: ""
      gitBranch: ""
      gitWorktree: ""
      modified: displayTimestamp
      created: displayTimestamp
      relative: originalParent
      nameSpans: ""
      relativeSpans: ""
      depth: 0
      expanded: false
      loading: false
      error: ""
      metadataDimmed: trashRow.hovered

      onCustomClicked: function(mouse) {
        trashList.currentIndex = trashRow.index
        controller.trashSelectedId = trashRow.entryId
        trashList.forceActiveFocus()
      }

      onCustomDoubleClicked: function(mouse) {
        if (trashRow.canRestore) controller.restoreTrashEntry(trashRow.entryId, "", false)
      }

      Rectangle {
        id: actionCover
        anchors.right: parent.right
        anchors.rightMargin: Style.space(7)
        anchors.verticalCenter: parent.verticalCenter
        width: trashRow.actionSpecs.length * Style.space(25)
          + Math.max(0, trashRow.actionSpecs.length - 1) * Style.space(3)
        height: Style.space(25)
        z: 2
        opacity: trashRow.hovered ? 1 : 0
        visible: opacity > 0
        color: Color.bar.background

        Behavior on opacity {
          NumberAnimation { duration: root.animationsEnabled ? 90 : 0 }
        }

        Rectangle { anchors.fill: parent; color: trashRow.color }

        Row {
          id: actionRow
          anchors.fill: parent
          spacing: Style.space(3)

          Repeater {
            model: trashRow.actionSpecs.length

            delegate: Rectangle {
              id: actionButton
              required property int index
              readonly property var spec: index < trashRow.actionSpecs.length
                ? trashRow.actionSpecs[index]
                : ({})

              width: Style.space(25)
              height: Style.space(25)
              radius: Math.min(Style.cornerRadius, Style.space(4))
              opacity: spec.enabled ? 1 : 0.35
              color: actionPointer.containsMouse && spec.enabled
                ? Util.alpha(spec.danger ? Color.urgent : Color.accent, 0.20)
                : Util.alpha(Color.bar.text, 0.05)

              Text {
                textFormat: Text.PlainText
                anchors.centerIn: parent
                text: String(actionButton.spec.glyph || "")
                color: actionButton.spec.danger && actionPointer.containsMouse ? Color.urgent : Color.muted
                font.family: Style.font.family
                font.pixelSize: Typography.caption
              }

              MouseArea {
                id: actionPointer
                anchors.fill: parent
                enabled: trashRow.hovered && actionButton.spec.enabled === true
                hoverEnabled: true
                cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                onClicked: {
                  if (actionButton.spec.action === "restore") controller.restoreTrashEntry(trashRow.entryId, "", false)
                  else if (actionButton.spec.action === "restore-to") root.restoreTo(trashRow)
                  else root.confirmDelete(trashRow)
                }
              }
            }
          }
        }
      }
    }

    Keys.onUpPressed: function(event) {
      if (count > 0) currentIndex = Math.max(0, currentIndex < 0 ? 0 : currentIndex - 1)
      if (currentIndex >= 0) controller.trashSelectedId = String(model.get(currentIndex).entryId)
      event.accepted = true
    }
    Keys.onDownPressed: function(event) {
      if (count > 0) currentIndex = Math.min(count - 1, currentIndex < 0 ? 0 : currentIndex + 1)
      if (currentIndex >= 0) controller.trashSelectedId = String(model.get(currentIndex).entryId)
      event.accepted = true
    }
    function restoreCurrent(event) {
      if (root.actionKeys.isRepeat(event)) { event.accepted = true; return }
      var entry = currentIndex >= 0 ? model.get(currentIndex) : null
      if (entry && entry.canRestore) controller.restoreTrashEntry(entry.entryId, "", false)
      event.accepted = true
    }
    Keys.onReturnPressed: function(event) { restoreCurrent(event) }
    Keys.onEnterPressed: function(event) { restoreCurrent(event) }
    Keys.onDeletePressed: function(event) {
      if (root.actionKeys.isRepeat(event)) { event.accepted = true; return }
      var entry = currentIndex >= 0 ? model.get(currentIndex) : null
      if (entry && entry.canDelete) root.confirmDelete(entry)
      event.accepted = true
    }
    Keys.onEscapePressed: function(event) {
      if (!root.actionKeys.isRepeat(event)) controller.goBack()
      event.accepted = true
    }
    Keys.onPressed: function(event) {
      if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier)) return
      var target = -1
      if (event.key === Qt.Key_J) target = Math.min(count - 1, currentIndex < 0 ? 0 : currentIndex + 1)
      else if (event.key === Qt.Key_K) target = Math.max(0, currentIndex < 0 ? 0 : currentIndex - 1)
      else if (event.key === Qt.Key_G) target = (event.modifiers & Qt.ShiftModifier) ? count - 1 : 0
      else if (event.key === Qt.Key_R) controller.refreshTrash()
      else if (event.key === Qt.Key_E && (event.modifiers & Qt.ShiftModifier) && count > 0) root.confirmEmpty()
      else return
      if (target >= 0 && count > 0) {
        currentIndex = target
        controller.trashSelectedId = String(model.get(currentIndex).entryId)
      }
      event.accepted = true
    }
  }

  Text {
    textFormat: Text.PlainText
    anchors.centerIn: trashList
    width: Math.max(0, parent.width - Style.space(40))
    visible: !controller.trashBusy && controller.trashCount === 0 && !controller.trashError
    text: "Trash is empty"
    color: Color.muted
    horizontalAlignment: Text.AlignHCenter
    font.family: Style.font.family
    font.pixelSize: Typography.body
  }

  Rectangle {
    id: statusToast
    anchors.top: parent.top
    anchors.left: parent.left
    anchors.right: parent.right
    height: Style.space(25)
    z: 5
    opacity: root.statusActive ? 1 : 0
    visible: opacity > 0
    color: Qt.lighter(Color.bar.background, 1.035)

    Behavior on opacity {
      NumberAnimation { duration: root.animationsEnabled ? 140 : 0 }
    }

    Text {
      textFormat: Text.PlainText
      anchors.fill: parent
      anchors.leftMargin: Style.space(9)
      anchors.rightMargin: Style.space(9)
      verticalAlignment: Text.AlignVCenter
      text: controller.trashOperationBusy
        ? root.operationStatus()
        : (controller.trashError || root.displayedNotice || "Reading Trash…")
      color: controller.trashError ? Color.urgent : (root.displayedNotice !== "" ? Color.accent : Color.muted)
      elide: Text.ElideRight
      font.family: Style.font.family
      font.pixelSize: Typography.caption
    }
  }

  Rectangle {
    id: dialogOverlay
    anchors.fill: parent
    visible: root.dialogMode !== ""
    focus: visible
    z: 80
    color: Util.alpha(Color.background, 0.70)

    Keys.priority: Keys.BeforeItem
    Keys.onPressed: function(event) { root.handleDialogKey(event) }

    MouseArea { anchors.fill: parent }

    Rectangle {
      anchors.centerIn: parent
      width: Math.min(parent.width - Style.space(28), Style.space(390))
      implicitHeight: dialogContent.implicitHeight + Style.space(28)
      color: Color.popups.background
      radius: Style.cornerRadius
      border.width: 1
      border.color: Color.popups.border

      Column {
        id: dialogContent
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: Style.space(14)
        spacing: Style.space(10)

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: root.dialogMode === "empty"
            ? "Empty Trash?"
            : (root.dialogMode === "delete" ? "Delete permanently?" : "Restore to another folder")
          color: Color.popups.text
          wrapMode: Text.WordWrap
          font.family: Style.font.family
          font.pixelSize: Typography.body
          font.weight: Font.DemiBold
        }

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: root.dialogMode === "empty"
            ? "Permanently delete all " + controller.trashCount + (controller.trashCount === 1 ? " item" : " items") + ". This cannot be undone."
            : (root.dialogMode === "delete"
              ? "Permanently delete “" + root.dialogEntryName + "”. This cannot be undone."
              : "Choose the destination folder for “" + root.dialogEntryName + "”. An existing item is never overwritten.")
          color: root.dialogMode === "restore-to" ? Color.muted : Color.urgent
          wrapMode: Text.WordWrap
          font.family: Style.font.family
          font.pixelSize: Typography.bodySmall
        }

        TextField {
          id: restoreDestination
          width: parent.width
          height: visible ? Style.space(30) : 0
          visible: root.dialogMode === "restore-to"
          onActiveFocusChanged: if (activeFocus) root.dialogFocusIndex = -1
          placeholderText: "/destination/folder"
          color: Color.popups.text
          selectedTextColor: Color.popups.text
          selectionColor: Util.alpha(Color.accent, 0.38)
          font.family: Style.font.family
          font.pixelSize: Typography.bodySmall
          background: Rectangle {
            color: Util.alpha(Color.popups.text, restoreDestination.activeFocus ? 0.10 : 0.06)
            radius: Math.min(Style.cornerRadius, Style.space(4))
            border.width: 1
            border.color: restoreDestination.activeFocus ? Color.accent : Util.alpha(Color.popups.text, 0.18)
          }
          Keys.priority: Keys.BeforeItem
          Keys.onPressed: function(event) {
            if ([Qt.Key_Tab, Qt.Key_Backtab, Qt.Key_Escape, Qt.Key_Return, Qt.Key_Enter].indexOf(event.key) >= 0)
              root.handleDialogKey(event)
          }
        }

        Rectangle {
          width: parent.width
          height: visible ? Style.space(28) : 0
          visible: root.dialogMode === "restore-to"
          radius: Math.min(Style.cornerRadius, Style.space(4))
          color: recreatePointer.containsMouse ? Util.alpha(Color.accent, 0.18) : Util.alpha(Color.popups.text, 0.05)
          border.width: root.dialogFocusIndex === 2 && dialogOverlay.activeFocus ? 1 : 0
          border.color: Color.accent

          Text {
            textFormat: Text.PlainText
            anchors.centerIn: parent
            text: "Recreate original parent and restore"
            color: recreatePointer.containsMouse ? Color.accent : Color.muted
            font.family: Style.font.family
            font.pixelSize: Typography.caption
          }

          MouseArea {
            id: recreatePointer
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: {
              controller.restoreTrashEntry(root.dialogEntryId, "", true)
              root.closeDialog()
            }
          }
        }

        Row {
          anchors.right: parent.right
          spacing: Style.space(7)

          Repeater {
            model: [
              { text: "Cancel", confirm: false },
              { text: root.dialogMode === "empty" ? "Empty Trash" : (root.dialogMode === "delete" ? "Delete" : "Restore"), confirm: true }
            ]

            delegate: Rectangle {
              required property var modelData
              width: Style.space(modelData.confirm ? 94 : 68)
              height: Style.space(28)
              radius: Math.min(Style.cornerRadius, Style.space(4))
              border.width: root.dialogFocusIndex === (modelData.confirm ? 1 : 0) && dialogOverlay.activeFocus ? 1 : 0
              border.color: modelData.confirm && root.dialogMode !== "restore-to" ? Color.urgent : Color.accent
              color: dialogPointer.containsMouse
                ? Util.alpha(modelData.confirm && root.dialogMode !== "restore-to" ? Color.urgent : Color.accent, 0.22)
                : Util.alpha(Color.popups.text, 0.07)

              Text {
                textFormat: Text.PlainText
                anchors.centerIn: parent
                text: modelData.text
                color: modelData.confirm && root.dialogMode !== "restore-to" ? Color.urgent : Color.popups.text
                font.family: Style.font.family
                font.pixelSize: Typography.caption
                font.weight: modelData.confirm ? Font.DemiBold : Font.Normal
              }

              MouseArea {
                id: dialogPointer
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: modelData.confirm ? root.runDialogAction() : root.closeDialog()
              }
            }
          }
        }
      }
    }
  }
}
