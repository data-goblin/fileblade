import QtQuick
import Quickshell.Io
import "../lib/KeyBindings.js" as KeyBindings

Item {
  id: root
  required property var service
  readonly property string path: service.bladeHost.configDir + "/keybindings.json"
  property var plan: KeyBindings.compile({})
  property string error: ""
  property string writtenBy: ""
  property bool newerWriter: false
  property string requestId: ""
  property int generation: 0
  readonly property var treeShortcuts: ({
    title: "Tree navigation",
    items: [
      { shortcut: label("next") + " / " + label("previous"), text: "Move down / up" },
      { shortcut: label("up"), text: "Enclosing folder / group" },
      { shortcut: label("open"), text: "Open / enter" },
      { shortcut: label("activate"), text: "Activate / toggle folder or group" },
      { shortcut: label("expand") + " / " + label("collapse"), text: "Expand / collapse" },
      { shortcut: label("expand-recursive"), text: "Expand selected subtree" },
      { shortcut: label("collapse-recursive"), text: "Collapse selected subtree" },
      { shortcut: label("expand-all") + " / " + label("collapse-all"), text: "Expand / collapse whole tree" },
      { shortcut: label("first") + " / " + label("last"), text: "First / last" },
      { shortcut: label("page-next"), text: "Page down" },
      { shortcut: label("page-previous"), text: "Page up" },
      { shortcut: label("search"), text: "Search" },
      { shortcut: label("help"), text: "Shortcuts" }
    ]
  })

  function label(action) { return KeyBindings.label(plan, action) }
  function reload() { debounce.restart() }
  function read() {
    fileWatch.reload()
    if (requestId) service.cancelBackendRequest(requestId, generation, true)
    var current = ++generation
    requestId = service.backendRequest("keybindings-prepare", [], current, function(response) {
      if (current !== root.generation) return
      root.requestId = ""
      var missing = response && response.ok === false && /\(os error 2\)$/.test(String(response.error || ""))
      try {
        if (!missing && (!response || !response.ok)) throw new Error(String(response && response.error || "Unable to read keybindings"))
        var next = KeyBindings.compile(missing ? {} : JSON.parse(response.text))
        if (JSON.stringify(next) !== JSON.stringify(root.plan)) root.plan = next
        root.writtenBy = missing ? "" : String(response.writtenBy || "")
        root.newerWriter = !missing && response.newerWriter === true
        root.error = next.problems.length ? next.problems.join("; ").slice(0, 300) : ""
      } catch (failure) {
        root.error = String(failure).slice(0, 300)
        console.warn("FileBlade keybindings: " + root.error + "; keeping previous bindings")
      }
    })
  }

  // Watch only: bounded, no-follow file reads belong to the backend.
  FileView {
    id: fileWatch
    path: root.path
    preload: false
    watchChanges: true
    printErrors: false
    onFileChanged: root.reload()
  }
  Timer { id: debounce; interval: 150; onTriggered: root.read() }
  Connections {
    target: root.service
    function onBackendReadyChanged() {
      if (root.service.backendReady && root.error) root.reload()
    }
  }
  Component.onCompleted: reload()
  Component.onDestruction: {
    if (requestId) service.cancelBackendRequest(requestId, generation, true)
  }
}
