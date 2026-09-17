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
    if (cursorPosition !== undefined)
      editor.cursorPosition = Math.max(0, Math.min(Number(cursorPosition), editor.length))
    module.syncingEditor = false
  }

  function changed(next) {
    module.notebook = next
    module.bytes = NotesState.textBytes(next)
    module.dirty = true
    module.saveFailed = false
    saveTimer.restart()
  }

  function createNote() {
    var next = NotesState.addNote(module.notebook)
    if (!next) return false
    changed(next)
    syncEditor(0)
    editor.forceActiveFocus()
    noteTabs.ensureVisible(module.activeNoteIndex)
    return true
  }

  function selectNote(index) {
    if (index === module.activeNoteIndex) {
      editor.forceActiveFocus()
      return
    }
    changed(NotesState.selectNote(module.notebook, index))
    module.clamped = false
    syncEditor(0)
    editor.forceActiveFocus()
  }

  function renameNote(index, label) {
    changed(NotesState.renameNote(module.notebook, index, label))
  }

  function closeNote(index) {
    if (module.noteItems.length <= 1) return
    var activeBefore = module.activeNote.id
    changed(NotesState.removeNote(module.notebook, index))
    if (module.activeNote.id !== activeBefore) syncEditor(0)
    editor.forceActiveFocus()
  }

  function editorChanged(value) {
    if (module.syncingEditor || !editor.activeFocus) return
    var cursor = editor.cursorPosition
    var plan = NotesState.editPlan(module.notebook, value, module.overCap)
    module.notebook = plan.notebook
    module.bytes = plan.bytes
    if (plan.action !== "write") {
      module.overCap = true
      module.dirty = false
      saveTimer.stop()
      return
    }
    module.overCap = false
    module.clamped = plan.clamped
    module.saveFailed = false
    module.dirty = true
    if (plan.clamped) syncEditor(cursor)
    saveTimer.restart()
  }

  function hydrate() {
    if (!context) return
    var incoming = NotesState.normalizeNotebook(
      context.state.get("text", NotesState.FIRST_NOTE_TEXT), context.state.get("rev", 0), context.state.get("label", "Note 1"))
    if (NotesState.notebookHydration(incoming, localState()).action !== "apply") return
    module.notebook = incoming.notebook
    module.bytes = incoming.bytes
    module.overCap = incoming.overCap
    module.clamped = false
    syncEditor()
    if (incoming.migrated) {
      module.dirty = true
      saveTimer.restart()
    }
  }

  function flush() {
    saveTimer.stop()
    if (!module.dirty || !context) return
    var next = NotesState.persistedNotebook(module.notebook)
    module.saving = true
    var saved = context.state.set("text", next)
    module.saving = false
    if (saved === false) {
      module.saveFailed = true
      module.dirty = true
      return
    }
    module.notebook = next
    module.dirty = false
    module.saveFailed = false
  }

  onActiveChanged: {
    if (!active) flush()
    else hydrate()
  }

  Component.onCompleted: hydrate()
  Component.onDestruction: flush()

  Connections {
    target: module.context
    ignoreUnknownSignals: true
    function onSlotStateChanged() { module.hydrate() }
  }

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
    visible: module.overCap || module.clamped || module.saveFailed
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
      text: module.saveFailed
        ? "The host rejected these notes. Shorten them, then edit again to retry."
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
    anchors.bottom: parent.bottom
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

      onTextChanged: module.editorChanged(text)
      onActiveFocusChanged: if (!activeFocus) module.flush()

      Keys.onPressed: function(event) {
        if (KeyPlan.isTabCycle(event.key, event.modifiers)) return
        var action = KeyPlan.editorAction(event.key, event.modifiers)
        if (action === "") return
        if (action === "focus-next") module.context.focusNext()
        else if (action === "focus-previous") module.context.focusPrevious()
        else {
          module.flush()
          module.context.closeBlade()
        }
        event.accepted = true
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
