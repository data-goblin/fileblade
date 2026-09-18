import QtQuick
import QtTest
import "../../modules/notes" as Notes
import "../../modules/notes/NotesState.js" as NotesState

TestCase {
  id: test
  name: "NotesModule"
  when: windowShown
  width: 380
  height: 700
  property var view
  property var editor
  QtObject {
    id: mockHost
    property bool layoutWritable: true
    property string lastWrittenLayoutText: ""
    property string layoutWriteRequestId: ""
    property string queuedLayoutDocument: ""
    function save() { layoutWriteRequestId = "pending" }
    function finish(ok) {
      layoutWriteRequestId = ""
      if (ok) lastWrittenLayoutText = JSON.stringify({blades:{right:{slots:[{id:"fixture",modules:[{module:"notes",state:mockContext.slotState}]}]}}})
    }
  }
  QtObject {
    id: mockState
    property bool accept: true
    function get(key, fallback) { return mockContext.slotState[key] === undefined ? fallback : mockContext.slotState[key] }
    function set(key, value) {
      if (!accept) return false
      var next = Object.assign({}, mockContext.slotState)
      next[key] = value
      mockContext.slotState = next
      return true
    }
  }
  QtObject {
    id: mockContext
    property var host: mockHost
    property var state: mockState
    property var slotState: ({})
    property string slotId: "fixture"
    property bool bladeOpen: true
    property bool collapsed: false
    property bool inPopout: false
    property int tabCount: 2
    property var ui: ({url: function(name) { return "" }})
    property int closes: 0
    function closeBlade() { closes++ }
  }
  Component { id: notesComponent; Notes.Module { width: test.width; height: test.height } }

  function init() {
    mockHost.layoutWritable = true
    mockHost.lastWrittenLayoutText = ""
    mockHost.layoutWriteRequestId = ""
    mockHost.queuedLayoutDocument = ""
    mockState.accept = true
    mockContext.closes = 0
    mockContext.slotState = {text: NotesState.normalizeNotebook(NotesState.emptyNotebook("First", "original text", 1)).notebook}
    mockContext.slotState.text.items[0].cursor = 3
    mockContext.slotState.text.items[0].anchor = 8
    view = createTemporaryObject(notesComponent, test, {context: mockContext})
    verify(view)
    editor = findChild(view, "notesEditor")
    verify(editor)
    editor.forceActiveFocus()
    verify(editor.activeFocus)
  }

  function test_initial_editor_restores_saved_selection() {
    wait(50)
    compare(editor.cursorPosition, 3)
    compare(editor.selectionStart, 3)
    compare(editor.selectionEnd, 8)
    verify(!view.dirty)
  }

  function test_admission_waits_for_ack_and_failure_keeps_edits_for_retry() {
    editor.text = "local edits"
    view.flush()
    verify(view.saving)
    compare(mockContext.slotState.text.items[0].text, "local edits")
    mockHost.finish(false)
    tryCompare(view, "saveFailed", true)
    verify(view.dirty)
    compare(editor.text, "local edits")
    view.closeWhenSaved()
    compare(mockContext.closes, 0)
    mockHost.finish(true)
    tryCompare(view, "saving", false)
    verify(!view.dirty && !view.saveFailed)
    compare(mockContext.closes, 1)
  }

  function test_refused_admission_and_readonly_keep_unsaved_text() {
    editor.text = "keep this"
    mockState.accept = false
    view.flush()
    verify(view.saveFailed && view.dirty && !view.saving)
    compare(editor.text, "keep this")
    mockState.accept = true
    mockHost.layoutWritable = false
    view.flush()
    verify(view.saveFailed && view.dirty)
    compare(mockHost.layoutWriteRequestId, "")
  }

  function test_typing_before_flush_survives_unchanged_saved_rebind() {
    compare(view.notebook.revision, 1)
    editor.text = "typed before flush"
    compare(view.notebook.revision, 1)
    var saved = JSON.parse(JSON.stringify(mockContext.slotState))
    mockContext.slotState = JSON.parse(JSON.stringify(saved))
    verify(!view.incomingConflict)
    compare(editor.text, "typed before flush")
    view.flush()
    verify(view.saving)
    mockHost.finish(true)
    tryCompare(view, "saving", false)
    compare(mockContext.slotState.text.items[0].text, "typed before flush")
    compare(mockContext.slotState.text.revision, 2)
    editor.text = "local second edit"
    mockContext.slotState = {text: NotesState.normalizeNotebook(NotesState.emptyNotebook("First", "external same revision", 2)).notebook}
    verify(view.incomingConflict)
    compare(editor.text, "local second edit")
  }

  function test_conflict_holds_local_until_explicit_choice() {
    editor.text = "local version"
    mockContext.slotState = {text: NotesState.normalizeNotebook(NotesState.emptyNotebook("First", "incoming version", 7)).notebook}
    verify(view.incomingConflict)
    compare(editor.text, "local version")
    view.flush()
    compare(mockHost.layoutWriteRequestId, "")
    view.resolveConflict(true)
    verify(view.saving)
    compare(mockContext.slotState.text.revision, 8)
    compare(mockContext.slotState.text.items[0].text, "local version")
    mockHost.finish(true)
    tryCompare(view, "saving", false)
    editor.text = "another local edit"
    mockContext.slotState = {text: NotesState.normalizeNotebook(NotesState.emptyNotebook("First", "incoming again", 9)).notebook}
    view.resolveConflict(false)
    compare(editor.text, "incoming again")
    verify(!view.dirty && !view.incomingConflict)
  }

  function test_queued_write_gap_does_not_report_failure() {
    editor.text = "queued edits"
    view.flush()
    mockHost.queuedLayoutDocument = "queued"
    mockHost.layoutWriteRequestId = ""
    mockHost.queuedLayoutDocument = ""
    Qt.callLater(function() { mockHost.layoutWriteRequestId = "next" })
    wait(50)
    verify(view.saving && !view.saveFailed)
    mockHost.finish(true)
    tryCompare(view, "saving", false)
    verify(!view.dirty && !view.saveFailed)
  }

  function test_footer_reports_last_edit_words_and_characters() {
    compare(view.footerText, "󰦨 2\t󰀬 13")
    var footer = findChild(view, "notesFooter")
    verify(footer)
    compare(footer.Accessible.name, "Words: 2, Characters: 13")
    var before = Date.now()
    editor.text = "three 😀 words"
    var edited = view.activeNote.edited
    verify(edited >= before && edited <= Date.now())
    compare(view.footerText, "󱦻 " + Qt.formatDateTime(new Date(edited), "yyyy-MM-dd HH:mm") + "\t󰦨 3\t󰀬 13")
    verify(footer.Accessible.name.indexOf("Last edit: ") === 0)
    view.createNote()
    compare(view.footerText, "󰦨 0\t󰀬 0")
    view.selectNote(0)
    compare(view.activeNote.edited, edited)
    var parts = footer.text.split("\t")
    compare(parts.length, 3)
    var offset = 0
    for (var field = 0; field < parts.length - 1; field++) {
      offset += parts[field].length
      var end = footer.positionToRectangle(offset)
      var next = footer.positionToRectangle(offset + 1)
      verify(next.x - end.x >= 24 || next.y > end.y)
      offset++
    }
  }

  function test_tabs_restore_selection_and_oversize_loaded_text_is_held() {
    editor.select(8, 2)
    view.createNote()
    view.selectNote(0)
    compare(editor.selectionStart, 2)
    compare(editor.selectionEnd, 8)
    compare(editor.cursorPosition, 2)
    view.flush()
    mockHost.finish(true)
    tryCompare(view, "saving", false)
    mockContext.slotState = {text: NotesState.normalizeNotebook(NotesState.emptyNotebook("Large", "x".repeat(70000), 9)).notebook}
    compare(editor.text.length, 70000)
    verify(view.overCap)
    editor.cursorPosition = 10
    view.flush()
    verify(!view.saving)
    compare(mockHost.layoutWriteRequestId, "")
  }

  function test_deleting_oversized_note_clears_capacity_and_saves() {
    var oversized = NotesState.emptyNotebook("Large", "x".repeat(NotesState.CAP_BYTES), 9)
    oversized = NotesState.addNote(oversized)
    oversized.activeId = oversized.items[0].id
    oversized.items[1].text = "small"
    mockContext.slotState = {text: oversized}
    verify(view.overCap)
    view.closeNote(0)
    compare(view.noteItems.length, 1)
    verify(!view.overCap)
    view.flush()
    verify(view.saving)
    mockHost.finish(true)
    tryCompare(view, "saving", false)
    compare(mockContext.slotState.text.items[0].text, "small")
    compare(mockContext.slotState.text.revision, 10)
    view.destroy()
    wait(0)
    view = createTemporaryObject(notesComponent, test, {context: mockContext})
    verify(view)
    editor = findChild(view, "notesEditor")
    verify(editor)
    wait(0)
    compare(view.noteItems.length, 1)
    compare(view.noteItems[0].text, "small")
  }

  function test_loading_incoming_after_escape_does_not_close_after_next_save() {
    editor.text = "local version"
    mockContext.slotState = {text: NotesState.normalizeNotebook(NotesState.emptyNotebook("First", "incoming version", 7)).notebook}
    verify(view.incomingConflict)
    view.closeWhenSaved()
    compare(mockContext.closes, 0)
    view.resolveConflict(false)
    compare(editor.text, "incoming version")
    verify(!view.incomingConflict)
    editor.text = "edited incoming"
    view.flush()
    verify(view.saving)
    mockHost.finish(true)
    tryCompare(view, "saving", false)
    compare(mockContext.closes, 0)
    compare(mockContext.slotState.text.items[0].text, "edited incoming")
    compare(mockContext.slotState.text.revision, 8)
  }
}
