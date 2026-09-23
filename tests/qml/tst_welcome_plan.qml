import QtQuick
import QtTest
import "../../modules/welcome/WelcomePlan.js" as WelcomePlan

TestCase {
  name: "WelcomePlan"

  function test_core_and_offline_help_need_no_installer_or_network() {
    compare(WelcomePlan.CORE.map(function(entry) { return entry.id }), ["files", "notes", "skills", "memory", "hooks", "mcp"])
    for (var topic of WelcomePlan.HELP) verify(topic.name.length > 0 && topic.text.length > 40)
    compare(WelcomePlan.LOCAL_CATALOG.version, 1)
    compare(WelcomePlan.LOCAL_CATALOG.entries.length, 0)
    verify(WelcomePlan.installCommand === undefined)
    verify(WelcomePlan.EXTENSIONS === undefined)
  }

  function test_state_pending_while_unset_or_after_a_version_change() {
    verify(WelcomePlan.pending("", "", "0.1.2"))
    verify(WelcomePlan.pending(undefined, undefined, "0.1.2"))
    verify(WelcomePlan.firstRun(""))
    verify(!WelcomePlan.firstRun("dismissed"))
    verify(!WelcomePlan.pending("dismissed", "0.1.2", "0.1.2"))
    verify(!WelcomePlan.pending("installed", "0.1.2", "0.1.2"))
    verify(WelcomePlan.pending("dismissed", "0.1.2", "0.2.0"))
    verify(WelcomePlan.updated("dismissed", "0.1.2", "0.2.0"))
    verify(WelcomePlan.updated("installed", "0.1.2", "0.2.0"))
    verify(!WelcomePlan.updated("", "", "0.2.0"))
    verify(!WelcomePlan.updated("dismissed", "0.2.0", ""))
    verify(!WelcomePlan.updated("dismissed", "0.2.0", "0.2.0"))
    verify(!WelcomePlan.updated("dismissed", "", "0.2.0"))
    verify(!WelcomePlan.pending("dismissed", "", "0.2.0"))
    verify(WelcomePlan.needsVersionAdoption("dismissed", "", "0.2.0"))
    verify(WelcomePlan.needsVersionAdoption("installed", "", "0.2.0"))
    verify(!WelcomePlan.needsVersionAdoption("", "", "0.2.0"))
    verify(!WelcomePlan.needsVersionAdoption("dismissed", "0.1.2", "0.2.0"))
    verify(!WelcomePlan.needsVersionAdoption("dismissed", "", ""))
    compare(WelcomePlan.normalizeState("bogus"), "")
    compare(WelcomePlan.welcomeSlot().modules[0].module, "welcome")
    compare(WelcomePlan.welcomeSlot().modules[1].module, "notes")
  }

  function test_repository_links_are_derived_from_the_manifest() {
    var manifest = { repository: "https://github.com/data-goblin/fileblade" }
    compare(WelcomePlan.issuesUrl(manifest), "https://github.com/data-goblin/fileblade/issues/new")
    compare(WelcomePlan.releaseUrl(manifest, "0.2.0"), "https://github.com/data-goblin/fileblade/releases/tag/v0.2.0")
    compare(WelcomePlan.releaseNotesUrl(manifest), "https://github.com/data-goblin/fileblade/blob/main/features/release/release-notes.md")
    compare(WelcomePlan.releaseUrl(manifest, ""), "")
    compare(WelcomePlan.issuesUrl(null), "")
    compare(WelcomePlan.issuesUrl({ repository: "http://github.com/data-goblin/fileblade" }), "")
    compare(WelcomePlan.issuesUrl({ repository: "https://example.org/evil" }), "")
    compare(WelcomePlan.issuesUrl({ repository: "https://github.com/a/b/../../c" }), "")
    compare(WelcomePlan.issuesUrl({ repository: "https://github.com/../.." }), "")
  }

  function entry(id, contract) {
    return { id: id, name: "Example", source: "https://example.org/source", hostContract: contract, lifecycle: "on-demand" }
  }

  function test_catalog_describes_compatibility_without_commands() {
    var first = entry("example.one/blade", 1)
    first.command = ["sh", "-c", "exit 99"]
    var parsed = WelcomePlan.catalog(JSON.stringify({version: 1, entries: [first, entry("example.two", 4)]}), 3)
    compare(parsed.entries.length, 2)
    compare(parsed.entries[0].id, first.id)
    compare(parsed.entries[0].source, first.source)
    compare(parsed.entries[0].lifecycle, "on-demand")
    verify(parsed.entries[0].compatible)
    verify(!parsed.entries[1].compatible)
    verify(parsed.entries[0].command === undefined)
  }

  function test_malformed_catalogs_are_refused_as_a_whole() {
    for (var text of ["", "{", "null", "[]", '{"version":2,"entries":[]}',
      JSON.stringify({version: 1, entries: [entry("duplicate", 1), entry("duplicate", 1)]}),
      JSON.stringify({version: 1, entries: Array(129).fill(entry("many", 1))})]) {
      compare(WelcomePlan.catalog(text, 3), null)
    }
    for (var change of [{id: "../escape"}, {source: "javascript:alert(1)"}, {source: "file:///tmp/run"},
      {hostContract: 1.5}, {hostContract: -1}, {name: ""}, {name: "\u0001"}, {lifecycle: "installed"}]) {
      var candidate = entry("example", 1)
      for (var key of Object.keys(change)) candidate[key] = change[key]
      compare(WelcomePlan.catalog(JSON.stringify({version: 1, entries: [candidate]}), 3), null)
    }
  }
}
