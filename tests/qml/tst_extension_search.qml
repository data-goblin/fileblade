import QtQuick
import QtTest

TestCase {
  id: testCase
  name: "ExtensionSearch"
  when: windowShown
  width: 380
  height: 600

  property var edits: []
  property var activations: []
  property int dismissals: 0
  property int deepRequests: 0
  property var chipToggles: []

  Loader {
    id: grammarLoader
    source: Qt.resolvedUrl("../../ui/SearchGrammar.qml")
  }

  Loader {
    id: cardLoader
    anchors.fill: parent
    source: Qt.resolvedUrl("../../ui/QuickNavCard.qml")
    onLoaded: {
      item.edited.connect(function(text) { testCase.edits.push(text) })
      item.activated.connect(function(index, alternate) { testCase.activations.push([index, alternate]) })
      item.dismissed.connect(function() { testCase.dismissals++ })
      item.deepRequested.connect(function() { testCase.deepRequests++ })
      item.chipToggled.connect(function(key, active) { testCase.chipToggles.push([key, active]) })
    }
  }

  readonly property var workspaces: [
    { name: "Sales Analytics", text: "Sales Analytics", fields: { type: ["workspace"] }, frecency: 0 },
    { name: "Customer 360", text: "Customer 360", fields: { type: ["workspace"] }, frecency: 4 },
    { name: "Supply Chain", text: "Supply Chain", fields: { type: ["workspace"] }, frecency: 1 }
  ]

  function normalize(key, value) {
    return String(value).toLowerCase().replace(/[\s_]+/g, "")
  }

  function init() {
    edits = []
    activations = []
    dismissals = 0
    deepRequests = 0
    chipToggles = []
  }

  function test_an_extension_keeps_its_own_kinds_in_type_filters() {
    var grammar = grammarLoader.item
    verify(grammar)
    var spec = grammar.parse("type:\"Semantic Model\" -type:report in:\"Customer 360\" prof", ["type", "in"], { normalize: normalize })
    compare(spec.filters.type.wanted, ["semanticmodel"])
    compare(spec.filters.type.excluded, ["report"])
    compare(spec.filters["in"].wanted, ["customer360"])
    compare(spec.terms.length, 1)
    var files = grammar.parse("type:report", ["type"], {})
    compare(files.filters.type.wanted.length, 0)
  }

  function test_frecency_orders_an_empty_query_and_fuzzy_matches_win_once_typed() {
    var grammar = grammarLoader.item
    var empty = grammar.ranked("", workspaces, { filterKeys: ["type"] })
    compare(empty.rows.map(function(row) { return row.record.name }), ["Customer 360", "Supply Chain", "Sales Analytics"])
    var typed = grammar.ranked("sa", workspaces, { filterKeys: ["type"] })
    compare(typed.rows[0].record.name, "Sales Analytics")
    compare(typed.rows[0].nameSpans, "0-2")
    var excluded = grammar.ranked("!cust", workspaces, {})
    compare(excluded.rows.map(function(row) { return row.record.name }).indexOf("Customer 360"), -1)
    var visit = grammar.frecency({ score: 2, last: 1000 }, 1000 + 7 * 86400)
    compare(Math.round(visit * 100) / 100, 1)
  }

  function test_the_quick_nav_card_takes_the_files_keys() {
    var card = cardLoader.item
    verify(card)
    card.title = "Workspaces"
    card.chips = [{ key: "case", label: "Aa", active: false, tip: "Ignoring case", activeTip: "Matching case" }]
    card.model = [
      { name: "Customer 360", relative: "Customer", nameSpans: "", relativeSpans: "" },
      { name: "Supply Chain", relative: "Operations", nameSpans: "", relativeSpans: "" }
    ]
    card.focusInput()
    keyClick(Qt.Key_S)
    compare(edits, ["s"])
    var list = findChild(card, "quickNavResults")
    compare(list.currentIndex, 0)
    keyClick(Qt.Key_Down)
    compare(list.currentIndex, 1)
    keyClick(Qt.Key_Return)
    compare(activations, [[1, false]])
    keyClick(Qt.Key_Return, Qt.ShiftModifier)
    compare(activations[1], [1, true])
    keyClick(Qt.Key_F, Qt.ControlModifier)
    compare(deepRequests, 1)
    keyClick(Qt.Key_Escape)
    compare(dismissals, 1)
  }
}
