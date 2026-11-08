import QtQuick
import QtTest
import "../../controllers"

TestCase {
  name: "SearchContinuationRegression"

  property var requests: []

  ListModel { id: rows }
  ListModel { id: treeRows }

  QtObject {
    id: fakeHost
    property string focusedMonitorName: ""
  }

  QtObject {
    id: fakeService
    property bool open: true
    property string rootPath: "/home/test"
    property string home: "/home/test"
    property bool showHidden: false
    property bool searchDeep: true
    property bool searchCaseSensitive: false
    property bool searchRegex: false
    property bool searchTreeLayout: true
    property bool gitEnabled: false
    property var gitRepoDirectories: ({})
    property var searchHistory: []
    property string treeSort: "name"
    property string treeFilter: ""
    property var treeModel: treeRows
    property var referenceScreen: null
    property var pendingCallback: null

    function backendRequest(name, arguments, generation, callback) {
      requests.push({ name: name, arguments: arguments })
      pendingCallback = callback
      return "search-" + requests.length
    }
    function reply(response) {
      var callback = pendingCallback
      pendingCallback = null
      callback(response)
    }
    function cancelBackendRequest() { return true }
    function channelProvider() { return null }
    function entrySnapshot() { return null }
    function makeRow(entry, depth) { return { path: String(entry.path || ""), name: String(entry.name || ""), depth: depth } }
    function normalizeRoot(path) { return String(path || "") }
    function pathWithin() { return true }
    function rememberSearch() {}
    function setOpen(value) { open = value }
  }

  SearchController {
    id: search
    service: fakeService
    model: rows
    bladeHost: fakeHost
  }

  function capped() {
    return { ok: true, partial: true, walked: 0, indexed: 0, entries: [{ path: "/home/test/a", name: "a" }] }
  }

  function init() {
    fakeService.open = true
    requests = []
    search.query = ""
    search.continuedFrom = ""
  }

  function test_a_capped_search_is_not_rerun_without_progress() {
    search.query = "scope:everywhere a"
    tryCompare(requests, "length", 1, 2000)
    fakeService.reply(capped())
    tryCompare(requests, "length", 2, 2000)
    fakeService.reply(capped())
    compare(search.spinning, false)
    wait(900)
    compare(requests.length, 2)
  }

  function test_a_partial_search_waits_while_the_blade_is_hidden() {
    search.query = "scope:everywhere b"
    tryCompare(requests, "length", 1, 2000)
    fakeService.open = false
    fakeService.reply(capped())
    wait(900)
    compare(requests.length, 1)
    fakeService.open = true
    tryCompare(requests, "length", 2, 2000)
  }
}
