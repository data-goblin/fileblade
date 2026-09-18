import QtQuick
import QtQuick.Controls
import qs.Commons
import "../../ui" as PluginUi
import "NotesState.js" as NotesState
import "KeyPlan.js" as KeyPlan
import "../../theme"

FocusScope {
  id: module

  property var context: null

  readonly property string title: "Notes"
  readonly property var shortcuts: [
    {
      title: "Notes",
      items: [
        { shortcut: "Double-click note", text: "Rename note" },
        { shortcut: "Middle-click note", text: "Close note" },
        { shortcut: "Tab/Shift+Tab", text: "Next / previous field" },
        { shortcut: "Esc", text: "Save and close the blade" }
      ]
    }
  ]
  readonly property color paneBackground: Qt.lighter(Color.background, 1.035)
  readonly property bool collapsed: !!context && context.collapsed === true
  readonly property bool active: !!context && context.bladeOpen !== false && !collapsed
  readonly property int maximumNotes: NotesState.MAX_NOTES
  readonly property var noteItems: notebook.items
  readonly property int activeNoteIndex: NotesState.activeIndex(notebook)
  readonly property var activeNote: NotesState.activeNote(notebook)

  property var notebook: NotesState.emptyNotebook("Note 1", "", 0)
  property int bytes: 0
  property bool dirty: false
  property bool saving: false
  property bool clamped: false
  property bool overCap: false
  property bool saveFailed: false
  property bool syncingEditor: false
  property var pendingNotebook: null
  property var incomingConflict: null
  property string observedNotebookText: ""
  property bool closeAfterSave: false
  readonly property var host: context ? context.host : null
  readonly property bool temporary: !!context && !!context.inPopout
  readonly property var footerFields: [
    { glyph: "󱦻", label: "Last edit", value: activeNote.edited ? Qt.formatDateTime(new Date(activeNote.edited), "yyyy-MM-dd HH:mm") : "" },
    { glyph: "󰦨", label: "Words", value: String((activeNote.text.match(/\S+/g) || []).length) },
    { glyph: "󰀬", label: "Characters", value: String([...activeNote.text].length) }
  ].filter(function(field) { return field.value !== "" })
  readonly property string footerText: footerFields.map(function(field) { return field.glyph + " " + field.value }).join("\t")

  function takeFocus(part) {
    editor.forceActiveFocus()
  }

  function localState() {
    return { notebook: module.notebook, revision: module.notebook.revision, dirty: module.dirty, saving: module.saving }
  }

  function syncEditor(cursorPosition) {
    module.syncingEditor = true
    var source = String(module.activeNote.text || "")
    if (editor.text !== source) editor.text = source
    if (cursorPosition !== undefined) editor.cursorPosition = NotesState.position(cursorPosition, source)
    else editor.select(NotesState.position(module.activeNote.anchor, source), NotesState.position(module.activeNote.cursor, source))
    module.syncingEditor = false
  }

  function rememberEditor() {
    if (syncingEditor) return
    var next = NotesState.remember(notebook, editor.cursorPosition, editor.selectionStart, editor.selectionEnd)
    if (next !== notebook) changed(next)
  }

  function changed(next) {
    module.notebook = next
    module.bytes = NotesState.textBytes(next)
    module.overCap = module.bytes > NotesState.CAP_BYTES
    module.dirty = true
    module.saveFailed = false
    if (!incomingConflict) saveTimer.restart()
  }

  function createNote() {
    rememberEditor()
    var next = NotesState.addNote(module.notebook)
    if (!next) return false
    changed(next)
    syncEditor()
    editor.forceActiveFocus()
    noteTabs.ensureVisible(module.activeNoteIndex)
    return true
  }

  function selectNote(index) {
    rememberEditor()
    if (index === module.activeNoteIndex) {
      editor.forceActiveFocus()
      return
    }
    changed(NotesState.selectNote(module.notebook, index))
    module.clamped = false
    syncEditor()
    editor.forceActiveFocus()
  }

  function renameNote(index, label) {
    changed(NotesState.renameNote(module.notebook, index, label))
  }

  function closeNote(index) {
    rememberEditor()
    if (module.noteItems.length <= 1) return
    var activeBefore = module.activeNote.id
    changed(NotesState.removeNote(module.notebook, index))
    if (module.activeNote.id !== activeBefore) syncEditor()
    editor.forceActiveFocus()
  }

  function editorChanged(value) {
    if (module.syncingEditor || !editor.activeFocus) return
    var cursor = editor.cursorPosition
    var plan = NotesState.editPlan(module.notebook, value, module.overCap, Date.now())
    module.notebook = plan.notebook
    module.bytes = plan.bytes
    if (plan.action !== "write") {
      module.overCap = true
      module.dirty = true
      saveTimer.stop()
      return
    }
    module.overCap = false
    module.clamped = plan.clamped
    module.saveFailed = false
    module.dirty = true
    if (plan.clamped) syncEditor(cursor)
    if (!incomingConflict) saveTimer.restart()
  }

  function hydrate() {
    if (!context) return
    var incoming = NotesState.normalizeNotebook(
      context.state.get("text", NotesState.FIRST_NOTE_TEXT), context.state.get("rev", 0), context.state.get("label", "Note 1"))
    var text = JSON.stringify(incoming.notebook)
    if (text === observedNotebookText) return
    observedNotebookText = text
    if (pendingNotebook && text === JSON.stringify(pendingNotebook)) return
    if (text !== JSON.stringify(notebook) && incoming.notebook.revision >= notebook.revision && (dirty || saving)) {
      incomingConflict = incoming.notebook
      saveTimer.stop()
      closeAfterSave = false
      return
    }
    if (NotesState.notebookHydration(incoming, localState()).action !== "apply") return
    module.notebook = incoming.notebook
    module.bytes = incoming.bytes
    module.overCap = incoming.overCap
    module.clamped = false
    syncEditor()
    if (incoming.migrated) {
      module.dirty = true
      if (!incomingConflict) saveTimer.restart()
    }
  }

  function flush() {
    rememberEditor()
    saveTimer.stop()
    if (overCap || incomingConflict || saving || !dirty || !context) return
    if (!temporary && (!host || !host.layoutWritable)) { saveFailed = true; return }
    var next = NotesState.persistedNotebook(notebook)
    pendingNotebook = next
    saving = true
    saveFailed = false
    notebook = next
    dirty = false
    if (context.state.set("text", next) === false) {
      saving = false
      dirty = true
      saveFailed = true
      pendingNotebook = null
      closeAfterSave = false
      return
    }
    if (temporary) {
      saving = false
      pendingNotebook = null
      return
    }
    host.save()
    settleTimer.restart()
  }

  function settleSave() {
    if (!saving || !host || !pendingNotebook) return
    var written = NotesState.writtenNotebook(host.lastWrittenLayoutText, context.slotId)
    if (JSON.stringify(written) === JSON.stringify(pendingNotebook)) {
      saving = false
      pendingNotebook = null
      saveFailed = false
      if (dirty) saveTimer.restart()
      else if (closeAfterSave && !incomingConflict) { closeAfterSave = false; context.closeBlade() }
    } else if (!host.layoutWriteRequestId && !host.queuedLayoutDocument) {
      saving = false
      pendingNotebook = null
      dirty = true
      saveFailed = true
      closeAfterSave = false
    }
  }

  function resolveConflict(keepLocal) {
    if (!incomingConflict || saving) return
    if (keepLocal) {
      notebook.revision = Math.max(notebook.revision, incomingConflict.revision)
      dirty = true
    } else {
      notebook = incomingConflict
      closeAfterSave = false
      bytes = NotesState.textBytes(notebook)
      overCap = bytes > NotesState.CAP_BYTES
      dirty = false
      saveFailed = false
      syncEditor()
    }
    incomingConflict = null
    if (keepLocal) flush()
  }

  function closeWhenSaved() {
    closeAfterSave = true
    flush()
    if (!dirty && !saving && !incomingConflict && !saveFailed) {
      closeAfterSave = false
      context.closeBlade()
    }
  }

  onActiveChanged: {
    if (!active) flush()
    else hydrate()
  }

  Component.onCompleted: { hydrate(); Qt.callLater(syncEditor) }
  Component.onDestruction: flush()

  Connections {
    target: module.context
    ignoreUnknownSignals: true
    function onSlotStateChanged() { module.hydrate() }
  }

  Connections {
    target: module.host
    ignoreUnknownSignals: true
    function onLastWrittenLayoutTextChanged() { settleTimer.restart() }
    function onLayoutWriteRequestIdChanged() { settleTimer.restart() }
    function onQueuedLayoutDocumentChanged() { settleTimer.restart() }
  }

  Timer { id: cursorTimer; interval: 0; onTriggered: module.rememberEditor() }
  Timer { id: settleTimer; interval: 0; onTriggered: Qt.callLater(module.settleSave) }

  Timer {
    id: saveTimer
    interval: 400
    repeat: false
    onTriggered: module.flush()
  }

  Rectangle {
    anchors.fill: parent
    color: module.paneBackground
  }

  Loader {
    id: header
    anchors.top: parent.top
    anchors.left: parent.left
    anchors.right: parent.right
    height: module.context && module.context.tabCount > 1 ? 0 : Style.space(32)
    source: module.context ? module.context.ui.url("PaneHeader") : ""
    onLoaded: {
      item.context = module.context
      item.title = "Notes"
      item.tabIndex = Qt.binding(function() { return module.context.tabIndex })
      item.reservedLeft = Qt.binding(function() { return module.context.cornerReserveLeft })
      item.reservedRight = Qt.binding(function() { return module.context.cornerReserveRight })
      item.highlighted = Qt.binding(function() { return module.activeFocus })
      item.preferredHeight = Qt.binding(function() { return header.height })
      item.showNavigation = false
    }
  }

  NoteTabs {
    id: noteTabs
    anchors.top: header.bottom
    anchors.left: parent.left
    anchors.right: parent.right
    moduleRoot: module
    z: 30
  }

  Rectangle {
    id: notice
    anchors.top: noteTabs.bottom
    anchors.left: parent.left
    anchors.right: parent.right
    height: visible ? warning.implicitHeight + Style.space(8) : 0
    visible: module.overCap || module.clamped || module.saveFailed || !!module.incomingConflict || module.temporary
    color: Qt.rgba(Color.urgent.r, Color.urgent.g, Color.urgent.b, 0.12)

    Text {
      id: warning
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      anchors.leftMargin: Style.space(8)
      anchors.rightMargin: Style.space(8)
      textFormat: Text.PlainText
      wrapMode: Text.WordWrap
      text: module.incomingConflict
        ? "Another version arrived. Your local edits are held here until you choose which version to keep."
        : module.temporary
        ? "This popout is temporary. Copy your notes before closing it."
        : module.saveFailed
        ? "Saving was not confirmed. Keep this blade open and retry; your edits are still here."
        : module.overCap
        ? "These notes are larger than 64 KiB in total. Saving is paused until they are trimmed."
        : "64 KiB total limit reached. Text beyond the limit was not kept."
      color: Color.urgent
      font.family: Style.font.family
      font.pixelSize: Typography.caption
    }
  }

  Flickable {
    id: scroller
    anchors.top: notice.bottom
    anchors.bottom: footer.top
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.margins: Style.space(6)
    contentWidth: width
    contentHeight: editor.implicitHeight
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: PluginUi.AccentScrollBar { }

    TapHandler {
      onTapped: editor.forceActiveFocus()
    }

    TextArea.flickable: TextArea {
      id: editor
      textFormat: TextEdit.PlainText
      wrapMode: TextEdit.Wrap
      selectByMouse: true
      persistentSelection: true
      placeholderText: "Type a note"
      placeholderTextColor: Color.muted
      color: Color.foreground
      selectionColor: Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.38)
      selectedTextColor: Color.foreground
      font.family: Style.font.family
      font.pixelSize: Typography.body
      background: null

      objectName: "notesEditor"
      onTextChanged: module.editorChanged(text)
      onCursorPositionChanged: if (!module.syncingEditor) cursorTimer.restart()
      onSelectionStartChanged: if (!module.syncingEditor) cursorTimer.restart()
      onSelectionEndChanged: if (!module.syncingEditor) cursorTimer.restart()
      onActiveFocusChanged: if (!activeFocus) module.flush()

      Keys.onPressed: function(event) {
        if (KeyPlan.isTabCycle(event.key, event.modifiers)) return
        var action = KeyPlan.editorAction(event.key, event.modifiers)
        if (action === "") return
        if (action === "focus-next") module.context.focusNext()
        else if (action === "focus-previous") module.context.focusPrevious()
        else {
          module.closeWhenSaved()
        }
        event.accepted = true
      }
    }
  }

  Column {
    id: footer
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.bottom: parent.bottom
    padding: Style.space(6)
    spacing: Style.space(3)
    TextEdit {
      objectName: "notesFooter"
      Accessible.name: module.footerFields.map(function(field) { return field.label + ": " + field.value }).join(", ")
      width: parent.width - parent.leftPadding - parent.rightPadding
      text: module.footerText.replace(/ /g, "\u00A0")
      tabStopDistance: NotesState.tabStop(text.split("\t").map(function(field) { return footerFont.advanceWidth(field) }), Style.space(24))
      readOnly: true
      selectByMouse: false
      activeFocusOnPress: false
      textFormat: TextEdit.PlainText
      wrapMode: TextEdit.WordWrap
      color: Color.muted
      font: footerFont.font
      FontMetrics {
        id: footerFont
        font.family: Style.font.family
        font.pixelSize: Typography.caption
      }
    }
    Row {
      spacing: Style.space(4)
      Button {
        text: "Retry save"
        font.family: Style.font.family
        font.pixelSize: Typography.caption
        visible: module.saveFailed && !module.incomingConflict
        enabled: !module.saving
        onClicked: module.flush()
        background: Rectangle { color: Color.bar.background; border.color: Color.accent; border.width: 1 }
      }
      Button {
        text: "Keep local edits"
        font.family: Style.font.family
        font.pixelSize: Typography.caption
        visible: !!module.incomingConflict
        enabled: !module.saving
        onClicked: module.resolveConflict(true)
        background: Rectangle { color: Color.bar.background; border.color: Color.accent; border.width: 1 }
      }
      Button {
        text: "Load incoming"
        font.family: Style.font.family
        font.pixelSize: Typography.caption
        visible: !!module.incomingConflict
        enabled: !module.saving
        onClicked: module.resolveConflict(false)
        background: Rectangle { color: Color.bar.background; border.color: Color.accent; border.width: 1 }
      }
    }
  }

  Loader {
    anchors.fill: scroller
    source: module.context ? module.context.ui.url("ScrollEdgeFade") : ""
    z: 2

    onLoaded: {
      item.flickable = scroller
      item.surfaceColor = Qt.binding(function() { return module.paneBackground })
    }
  }
}
