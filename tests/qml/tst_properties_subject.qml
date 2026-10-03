import QtQuick
import QtTest
import "../../controllers"
import "../../modules/properties" as Properties
import "../../lib/KeyBindings.js" as KeyBindings

TestCase {
  id: test
  name: "PropertiesSubject"
  width: 420
  height: 900
  visible: true
  when: windowShown

  property var triggered: []
  property var opened: []
  property var copied: []
  property int trashRequests: 0

  QtObject {
    id: selection
    property var selectedPaths: ["/work/report.pbip"]
    signal chosen()
  }

  QtObject {
    id: files
    property var keybindings: ({ plan: KeyBindings.compile({}) })
    property var selectedMetadata: ({ path: "/work/report.pbip", name: "report.pbip", kind: "File", mime: "text/plain", size: 12, size_text: "12 B", is_dir: false })
    property int selectedCount: 1
    property int selectedTotalSize: 12
    property bool metadataBusy: false
    property string metadataError: ""
    property string operationNotice: ""
    property string operationError: ""
    property string operationLabel: ""
    property bool operationCancellable: false
    property string launchError: ""
    property bool gitEnabled: false
    property bool propertyIcons: true
    property var themeFolderPalette: ({})
    function backendRequest() { return "" }
    function cancelBackendRequest() {}
    function folderColor() { return "" }
    function humanSize(value) { return String(value) + " B" }
    function openUrl(url, screen) { test.opened = test.opened.concat([url]); return true }
    function copyText(text) { test.copied = test.copied.concat([text]); return true }
    function requestTrash() { test.trashRequests++ }
    function openDefault() { fail("file activation must not run while a module item is shown") }
    function openInEditor() { fail("file editing must not run while a module item is shown") }
    function focusTree() {}
  }

  PropertiesController {
    id: properties
    selection: selection
    onActionTriggered: function(ownerModuleId, actionId) { test.triggered = test.triggered.concat([ownerModuleId + ":" + actionId]) }
  }

  QtObject {
    id: owner
    property string moduleId: "data-goblin.fileblade-fabric/fabric"
    property var definition: ({ name: "Fabric" })
    property bool retired: false
  }

  QtObject {
    id: other
    property string moduleId: "data-goblin.fileblade-databricks/databricks"
    property var definition: ({ name: "Databricks" })
    property bool retired: false
  }

  QtObject {
    id: context
    property bool bladeOpen: true
    property var hostWindow: null
    property int tabCount: 1
    property int cornerReserveLeft: 0
    property int cornerReserveRight: 0
    property int surfaceOriginX: 0
    property string edge: "right"
    function service(id) { return id === "files" ? files : (id === "properties" ? properties : null) }
    function requestFocus() { view.takeFocus("") }
    function focusNext() {}
    function focusPrevious() {}
    function closeBlade() {}
  }

  Properties.Module { id: view; width: test.width; height: test.height; context: context }

  Component {
    id: ownerComponent
    QtObject {
      property string moduleId: "data-goblin.fileblade-fabric/fabric"
      property bool retired: false
    }
  }

  function item() {
    return {
      title: "Sales report",
      subtitle: "Report in Finance workspace",
      glyph: "\u{F2810}",
      glyphFamily: "FabricSymbols NF",
      color: "#E8A33D",
      fields: [
        { label: "Workspace", value: "Finance", kind: "text" },
        { label: "Description", value: "Line one\nLine two", kind: "multiline" },
        { label: "Tags", value: ["certified", "finance", "certified"], kind: "tags" },
        { label: "Portal", value: "https://app.fabric.microsoft.com/groups/abc/reports/def", kind: "link" },
        { label: "Path", value: "Finance.Workspace/Sales.Report", kind: "code" }
      ],
      actions: [{ id: "rename", text: "Rename" }, { id: "refresh", text: "Refresh" }]
    }
  }

  function subjectView() {
    var found = null
    function walk(node) {
      if (found || !node) return
      if (node.runAction !== undefined && node.copyCurrent !== undefined) { found = node; return }
      for (var i = 0; i < node.children.length; i++) walk(node.children[i])
    }
    walk(view)
    return found
  }

  function textItems(root, value) {
    var hits = []
    function walk(node) {
      if (!node) return
      if (node.text !== undefined && String(node.text) === value && node.visible) hits.push(node)
      for (var i = 0; i < node.children.length; i++) walk(node.children[i])
    }
    walk(root)
    return hits
  }

  function press(key, modifiers) {
    keyClick(key, modifiers || Qt.NoModifier)
    wait(30)
  }

  function init() {
    properties.clear()
    selection.selectedPaths = ["/work/report.pbip"]
    owner.retired = false
    test.triggered = []
    test.opened = []
    test.copied = []
    test.trashRequests = 0
  }

  function test_inspect_shows_the_module_item_instead_of_the_file() {
    verify(textItems(view, "report.pbip").length > 0)
    verify(properties.inspect(owner, item()))
    compare(properties.ownerModuleId, "data-goblin.fileblade-fabric/fabric")
    compare(properties.ownerName, "Fabric")
    var subject = subjectView()
    verify(subject.visible)
    compare(textItems(view, "report.pbip").length, 0)
    compare(textItems(subject, "Sales report").length, 1)
    compare(textItems(subject, "Report in Finance workspace").length, 1)
    var glyph = textItems(subject, "\u{F2810}")[0]
    compare(glyph.font.family, "FabricSymbols NF")
    compare(String(glyph.color), "#e8a33d")
    verify(textItems(subject, "Line one\nLine two").length === 1)
    compare(textItems(subject, "certified").length, 1)
    compare(textItems(subject, "finance").length, 1)
    compare(textItems(subject, "https://app.fabric.microsoft.com/groups/abc/reports/def").length, 1)
    var code = textItems(subject, "Finance.Workspace/Sales.Report")[0]
    compare(code.font.family, "monospace")
    compare(textItems(subject, "Rename").length, 1)
    compare(textItems(subject, "Refresh").length, 1)
  }

  function test_mouse_opens_links_copies_code_and_runs_actions() {
    verify(properties.inspect(owner, item()))
    var subject = subjectView()
    var link = textItems(subject, "https://app.fabric.microsoft.com/groups/abc/reports/def")[0]
    waitForRendering(view)
    mouseClick(link)
    compare(test.opened, ["https://app.fabric.microsoft.com/groups/abc/reports/def"])
    mouseClick(textItems(subject, "Finance.Workspace/Sales.Report")[0])
    compare(test.copied, ["Finance.Workspace/Sales.Report"])
    tryCompare(textItems(subject, "Copied")[0] || ({ visible: false }), "visible", true)
    mouseClick(textItems(subject, "Refresh")[0])
    compare(test.triggered, ["data-goblin.fileblade-fabric/fabric:refresh"])
  }

  function test_keyboard_walks_fields_and_actions_without_touching_files() {
    view.takeFocus("")
    var subject = subjectView()
    tryVerify(function() { return view.activeFocus && !subject.activeFocus })
    verify(properties.inspect(owner, item()))
    tryVerify(function() { return subject.activeFocus })
    compare(subject.cursor, 0)
    press(Qt.Key_Y)
    compare(test.copied, ["Finance"])
    press(Qt.Key_J); press(Qt.Key_J); press(Qt.Key_J)
    compare(subject.cursor, 3)
    press(Qt.Key_Return)
    compare(test.opened, ["https://app.fabric.microsoft.com/groups/abc/reports/def"])
    press(Qt.Key_J)
    press(Qt.Key_Return)
    compare(test.copied, ["Finance", "Finance.Workspace/Sales.Report"])
    press(Qt.Key_G, Qt.ShiftModifier)
    compare(subject.cursor, 6)
    press(Qt.Key_Return)
    compare(test.triggered, ["data-goblin.fileblade-fabric/fabric:refresh"])
    press(Qt.Key_K)
    press(Qt.Key_C, Qt.ControlModifier)
    compare(test.copied.length, 2)
    press(Qt.Key_Delete)
    press(Qt.Key_E)
    compare(test.trashRequests, 0)
    press(Qt.Key_G)
    compare(subject.cursor, 0)
    compare(view.shortcuts[0].items[0].text, "Move between fields and actions")
    selection.chosen()
    tryVerify(function() { return view.activeFocus && !subject.activeFocus })
    compare(view.shortcuts[0].items[0].text, "Scroll")
  }

  function test_newest_wins_between_files_and_the_module() {
    verify(properties.inspect(owner, item()))
    selection.selectedPaths = ["/work/report.pbip"]
    verify(properties.active)
    selection.selectedPaths = ["/work/model.bim"]
    verify(!properties.active)
    verify(subjectView().visible === false)
    verify(properties.inspect(owner, item()))
    selection.chosen()
    verify(!properties.active)
    verify(properties.inspect(owner, item()))
    verify(properties.inspect(other, { title: "main.default.sales", fields: [] }))
    compare(properties.ownerModuleId, "data-goblin.fileblade-databricks/databricks")
    verify(!properties.release(owner))
    verify(properties.active)
    verify(properties.release(other))
    verify(!properties.active)
    verify(textItems(view, "report.pbip").length > 0)
    verify(!properties.trigger("refresh"))
  }

  function test_unloaded_or_destroyed_owners_are_cleared() {
    verify(properties.inspect(owner, item()))
    owner.retired = true
    verify(!properties.active)
    compare(properties.subject, null)
    verify(!properties.inspect(owner, item()))
    var transient = ownerComponent.createObject(test)
    verify(properties.inspect(transient, item()))
    transient.destroy()
    tryVerify(function() { return properties.subject === null })
    compare(properties.ownerModuleId, "")
  }

  function test_subjects_are_bounded() {
    verify(!properties.inspect(owner, { title: "   " }))
    verify(!properties.inspect(null, item()))
    verify(!properties.inspect({ moduleId: "" }, item()))
    var fields = []
    for (var i = 0; i < 80; i++) fields.push({ label: "Field " + i, value: "value " + i })
    var tags = []
    for (var t = 0; t < 50; t++) tags.push("tag-" + t)
    var actions = []
    for (var a = 0; a < 12; a++) actions.push({ id: "action-" + a, text: "Action " + a })
    actions.push({ id: "../escape", text: "Bad" }, { id: "action-1", text: "Duplicate" })
    verify(properties.inspect(owner, {
      title: "x".repeat(500) + "\u0007",
      subtitle: "y".repeat(500),
      glyph: "too long for a glyph",
      glyphFamily: "Bad;Family{}",
      color: "red; background: url(x)",
      fields: [
        { label: "Tags", value: tags, kind: "tags" },
        { label: "Unsafe link", value: "javascript:alert(1)", kind: "link" },
        { label: "Unknown kind", value: "plain", kind: "html" },
        { label: "Huge", value: "z".repeat(20000), kind: "code" }
      ].concat(fields),
      actions: actions
    }))
    var subject = properties.subject
    compare(subject.title.length, properties.limits.title)
    verify(subject.title.indexOf("\u0007") < 0)
    compare(subject.subtitle.length, properties.limits.subtitle)
    compare(subject.glyph, "")
    compare(subject.glyphFamily, "")
    compare(subject.color, "")
    compare(subject.fields.length, properties.limits.fields)
    compare(subject.fields[0].value.length, properties.limits.tags)
    compare(subject.fields[1].kind, "text")
    compare(subject.fields[2].kind, "text")
    compare(subject.fields[3].value.length, properties.limits.code)
    compare(subject.actions.length, properties.limits.actions)
    verify(subject.truncated)
    verify(textItems(subjectView(), "Some properties were left out because the module sent more than FileBlade shows.").length === 1)
    verify(!properties.trigger("../escape"))
    var budgetFields = []
    for (var b = 0; b < 10; b++) budgetFields.push({ label: "Block " + b, value: "w".repeat(8000), kind: "multiline" })
    verify(properties.inspect(owner, { title: "Budget", fields: budgetFields }))
    compare(properties.subject.fields.length, 4)
    verify(properties.subject.truncated)
  }
}
