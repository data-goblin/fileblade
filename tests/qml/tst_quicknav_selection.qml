import QtQuick
import QtTest
import "../../controllers" as Controllers
import "../../modules/files" as Files

TestCase {
  id: testCase
  name: "QuickNavSelection"
  when: windowShown
  width: 380
  height: 700

  ListModel { id: results }

  QtObject {
    id: service
    property bool open: false
    property bool searchDeep: false
    property bool showHidden: false
    property alias searchModel: search.model
    property alias searchQuery: search.query
    property alias searchBusy: search.busy
    property alias searchError: search.error
    property alias searchSpinner: search.spinnerGlyph
    property alias quickNavActive: search.quickNavActive
    property alias quickNavChannel: search.quickNavChannel
    property alias quickNavHome: search.quickNavHome
    property alias quickNavSelectionRevision: search.quickNavSelectionRevision
    function channelProvider(channel) { return { rows: function(query) { return [] } } }
    function folderColor(path) { return "" }
  }

  Controllers.SearchController {
    id: search
    service: service
    model: results
    bladeHost: ({})
  }

  Files.QuickNavOverlay {
    id: overlay
    anchors.fill: parent
    controller: service
    context: ({ screen: null })
  }

  function rows(names) {
    return names.map(function(name) {
      return { name: name, path: "/home/test/" + name, relative: name,
        kind: "Directory", isDir: true, isSymlink: false, isGitRepo: false,
        nameSpans: "", relativeSpans: "" }
    })
  }

  function init() {
    search.quickNavActive = false
    search.query = ""
    search.clearRows()
    search.quickNavActive = true
  }

  function test_a_changed_query_selects_the_best_result_after_reordering() {
    search.query = "s"
    search.presentRows(rows(["spare", "another", "s-sqsp-temp"]), true)
    var list = findChild(overlay, "quickNavResults")
    compare(list.currentIndex, 0)
    overlay.moveCurrent(2)
    compare(list.currentIndex, 2)
    search.query = "sqsp"
    search.presentRows(rows(["sqsp-cli", "squarespace", "s-sqsp-temp"]), true)
    compare(list.currentIndex, 0)
    compare(results.get(list.currentIndex).name, "sqsp-cli")
    overlay.moveCurrent(1)
    compare(list.currentIndex, 1)
    search.presentRows(rows(["sqsp-cli", "squarespace", "s-sqsp-temp"]), true)
    compare(list.currentIndex, 1)
  }

  function test_reopening_and_switching_channels_selects_the_first_result() {
    search.presentRows(rows(["one", "two"]), true)
    overlay.moveCurrent(1)
    search.clearRows()
    search.quickNavChannel = "recent"
    search.presentRows(rows(["three", "four"]), true)
    var list = findChild(overlay, "quickNavResults")
    compare(list.currentIndex, 0)
    overlay.moveCurrent(1)
    search.clearRows()
    search.presentRows(rows(["one", "two"]), true)
    compare(list.currentIndex, 0)
  }

  function test_recent_requests_honor_the_hidden_setting() {
    search.quickNavChannel = "recent"
    service.showHidden = false
    compare(search.searchRequest("zoxide", "sqsp").arguments.indexOf("--show-hidden"), -1)
    service.showHidden = true
    verify(search.searchRequest("zoxide", "sqsp").arguments.indexOf("--show-hidden") >= 0)
  }
}
