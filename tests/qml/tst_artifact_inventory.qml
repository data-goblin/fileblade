import QtQuick
import QtTest
import "../../ui"

TestCase {
  id: testCase
  name: "ArtifactInventory"
  property var requests: []
  property var subscriptions: []
  property var cancelled: []
  property var inventory: null

  Item {
    id: files
    property string contextPath: "/project/one"
    function backendRequest(name, args, generation, callback, progress, deadline, options) {
      var id = "request-" + requests.length
      requests.push({ id: id, name: name, args: args.slice(), generation: generation, callback: callback, options: options })
      return id
    }
    function backendSubscribe(paths, generation, event, ready, closed, includeWrites) {
      var id = "watch-" + subscriptions.length
      subscriptions.push({ id: id, paths: paths, generation: generation, event: event, ready: ready, closed: closed, includeWrites: includeWrites === true })
      return id
    }
    function cancelBackendRequest(id, generation, discardCallbacks) { cancelled.push({ id: id, generation: generation, discardCallbacks: !!discardCallbacks }) }
  }

  Component {
    id: component
    ArtifactInventory { files: files; providerId: "test.inventory"; providerRoot: "/plugins/inventory" }
  }

  function observer() { return { service: function() { return files } } }
  function rows(name, paths) {
    return { ok: true, schemaVersion: 1, project: files.contextPath, items: [{ name: name }], watchPaths: paths || [] }
  }
  function finish(index, response) { requests[index].callback(response) }
  function scanArguments(index) { return JSON.parse(requests[index].args[9]) }
  function scopeOf(index) { var args = scanArguments(index); return args[args.indexOf("--scope") + 1] }
  function names() { return inventory.items.map(function(row) { return row.name }) }
  function start() { inventory.startScan() }
  function init() {
    files.contextPath = "/project/one"
    requests = []; subscriptions = []; cancelled = []
    inventory = createTemporaryObject(component, this)
    verify(inventory !== null)
  }
  function cleanup() { if (inventory) inventory.destroy(); inventory = null }

  function test_one_scan_runs_a_project_lane_and_a_user_lane() {
    inventory.attach(observer()); start()
    compare(requests.length, 2)
    compare(scopeOf(0), "project")
    compare(scopeOf(1), "user")
    compare(scanArguments(0), ["--project", "/project/one", "--json", "--scope", "project", "--exact"])
    finish(0, rows("project row")); finish(1, rows("user row"))
    compare(names(), ["project row", "user row"])
    verify(!inventory.busy)
  }

  function test_two_views_share_one_scan_and_last_detach_stops_it() {
    var first = observer(), second = observer()
    inventory.attach(first); inventory.attach(second); inventory.attach(second)
    compare(inventory.observers.length, 2)
    start()
    compare(requests.length, 2)
    inventory.detach(first)
    compare(cancelled.length, 0)
    inventory.detach(second)
    compare(cancelled.length, 2)
    verify(!cancelled[0].discardCallbacks)
    finish(0, rows("late")); finish(1, rows("late user"))
    compare(inventory.items.length, 0)
    verify(!inventory.busy)
  }

  function test_root_change_clears_project_rows_keeps_user_rows_and_rejects_old_scan() {
    inventory.attach(observer()); start(); finish(0, rows("first")); finish(1, rows("stable user"))
    compare(names(), ["first", "stable user"])
    inventory.refresh(); start()
    compare(requests.length, 4)
    files.contextPath = "/project/two"
    compare(names(), ["stable user"])
    start(); compare(requests.length, 4)
    finish(2, rows("wrong project")); start()
    compare(requests.length, 5)
    compare(scopeOf(4), "project")
    finish(3, rows("user again"))
    compare(names(), ["user again"])
    compare(scanArguments(4), ["--project", "/project/two", "--json", "--scope", "project", "--exact"])
    finish(4, rows("second"))
    compare(names(), ["second", "user again"])
    compare(inventory.projectRoot, "/project/two")
    start(); compare(requests.length, 5)
  }

  function test_unchanged_user_rescan_keeps_the_same_row_array() {
    inventory.attach(observer()); start(); finish(0, rows("project row")); finish(1, rows("user row"))
    var before = inventory.items
    inventory.refresh(); start()
    finish(2, rows("project row")); finish(3, rows("user row"))
    verify(inventory.items === before)
    inventory.refresh(); start()
    finish(4, rows("project row")); finish(5, rows("changed user row"))
    verify(inventory.items !== before)
    compare(names(), ["project row", "changed user row"])
  }

  function test_accepted_write_survives_detach_and_keeps_original_arguments() {
    var view = observer()
    inventory.attach(view); start(); finish(0, rows("first")); finish(1, rows("user"))
    var args = ["--project", "/project/one", "--id", "original"]
    verify(inventory.mutate("apply", args, "private-input"))
    args[3] = "changed selection"
    inventory.detach(view)
    files.contextPath = "/project/two"
    verify(inventory.applying)
    compare(cancelled.filter(function(request) { return request.id === "request-2" }).length, 0)
    compare(JSON.parse(requests[2].args[9]), ["--project", "/project/one", "--id", "original"])
    compare(requests[2].options.input, "private-input")
    verify(requests[2].args.join(" ").indexOf("private-input") < 0)
    verify(!inventory.mutate("apply", []))
    finish(2, { ok: true, schemaVersion: 1 })
    verify(!inventory.applying)
    compare(requests.length, 3)
    inventory.attach(view); start()
    compare(requests.length, 5)
  }

  function test_write_invalidates_earlier_reads_and_waits_for_actual_completion() {
    inventory.attach(observer()); start()
    verify(inventory.mutate("apply", []))
    finish(0, rows("stale")); finish(1, rows("stale user"))
    compare(inventory.items.length, 0)
    inventory.refresh(); start(); compare(requests.length, 3)
    finish(2, { ok: false, schemaVersion: 1, message: "refused" })
    compare(inventory.applyError, "refused")
    start(); compare(requests.length, 5)
    compare(scopeOf(3), "project"); compare(scopeOf(4), "user")
  }

  function test_write_failure_from_previous_project_does_not_replace_current_status() {
    inventory.attach(observer())
    verify(inventory.mutate("apply", []))
    files.contextPath = "/project/two"
    finish(0, { ok: false, error: "old project error" })
    compare(inventory.applyError, "")
    start(); finish(1, rows("current")); finish(2, rows("user"))
    compare(names(), ["current", "user"])
  }

  function test_provider_refusal_keeps_the_agent_failure_explanation() {
    inventory.attach(observer())
    verify(inventory.mutate("apply", []))
    finish(0, { ok: false, schemaVersion: 1, results: [
      { ok: true, message: "already applied" }, { ok: false, message: "The existing directory is not a link" }
    ] })
    compare(inventory.applyError, "The existing directory is not a link")
  }

  function test_each_lane_owns_its_watch_and_a_user_event_only_rescans_the_user_lane() {
    inventory.attach(observer()); inventory.attach(observer()); start()
    finish(0, rows("first", ["/project/one"]))
    finish(1, rows("user", ["/home/me/.claude"]))
    compare(subscriptions.length, 2)
    compare(subscriptions[0].paths, ["/project/one"])
    compare(subscriptions[1].paths, ["/home/me/.claude"])
    subscriptions[0].ready({ skipped: [] }); subscriptions[1].ready({ skipped: [] }); start()
    compare(requests.length, 4)
    finish(2, rows("reconciled", ["/project/one"])); finish(3, rows("user", ["/home/me/.claude"]))
    compare(subscriptions.length, 2)
    subscriptions[1].event({ path: "/home/me/.claude/skills/new", events: ["create"] })
    start(); compare(requests.length, 5)
    compare(scopeOf(4), "user")
    finish(4, rows("external user", ["/home/me/.claude"]))
    compare(names(), ["reconciled", "external user"])
    subscriptions[0].event({ events: ["move_self"] })
    start(); compare(scopeOf(5), "project")
    finish(5, rows("replacement", ["/project/one"]))
    compare(subscriptions.length, 3)
    subscriptions[0].closed({ error: "late old close" })
    compare(inventory.watchError, "")
  }

  function test_watch_failure_is_visible_and_manual_refresh_retries() {
    inventory.attach(observer()); start()
    finish(0, rows("first", ["/project/one"])); finish(1, rows("user"))
    subscriptions[0].closed({ error: "watch unavailable" })
    verify(inventory.watchError.indexOf("Watch stopped") >= 0)
    inventory.refresh(); start()
    finish(2, rows("retry", ["/project/one"])); finish(3, rows("user"))
    compare(subscriptions.length, 2)
    subscriptions[1].ready({ skipped: ["gone"] })
    verify(inventory.watchError.indexOf("Some sources") >= 0)
    start(); finish(4, rows("reconciled", ["/project/one"]))
    compare(subscriptions.length, 2)
    inventory.refresh(true); start()
    finish(5, rows("retry skipped", ["/project/one"])); finish(6, rows("user"))
    compare(subscriptions.length, 3)
    subscriptions[2].ready({ skipped: [] })
    compare(inventory.watchError, "")
  }

  function test_watch_limit_warning_tracks_coverage_without_reinstalling_the_same_paths() {
    inventory.attach(observer()); start()
    var response = rows("limited", ["/project/one"])
    response.watchTruncated = true
    finish(0, response); finish(1, rows("user"))
    verify(inventory.watchError.indexOf("watch limit") >= 0)
    inventory.refresh(); start()
    finish(2, rows("complete", ["/project/one"])); finish(3, rows("user"))
    compare(subscriptions.length, 1)
    compare(inventory.watchError, "")
  }

  function test_payload_and_metrics_are_bounded_across_lanes() {
    inventory.attach(observer()); inventory.maximumItems = 2; start()
    finish(0, { ok: true, schemaVersion: 1, items: [
      { name: "first", metrics: { bytes: -1, words: "Infinity", fileTokens: 81, updated: "x".repeat(100) } },
      { name: "second" }
    ] })
    finish(1, { ok: true, schemaVersion: 1, items: [{ name: "third" }] })
    compare(inventory.items.length, 2); verify(inventory.truncated)
    compare(inventory.items[0].metrics.bytes, 0)
    compare(inventory.items[0].metrics.words, null)
    compare(inventory.items[0].metrics.fileTokens, 81)
    compare(inventory.items[0].metrics.updated.length, 32)
    inventory.refresh(); start()
    finish(2, { ok: true, items: [] }); finish(3, { ok: true, schemaVersion: 1, items: [] })
    verify(inventory.loadError !== ""); compare(inventory.items.length, 0)
  }

  function test_semantic_definition_shape_and_health_basis_are_preserved() {
    inventory.itemsKey = "definitions"; inventory.healthBasis = "configuration-only"; inventory.exactProject = false
    inventory.scanArguments = ["--watch"]
    inventory.attach(observer()); start()
    verify(scanArguments(0).indexOf("--exact") < 0)
    compare(scanArguments(0), ["--project", "/project/one", "--json", "--scope", "project", "--watch"])
    compare(scanArguments(1), ["--project", "/project/one", "--json", "--scope", "user", "--watch"])
    finish(0, { ok: true, schemaVersion: 1, healthBasis: "configuration-only", definitions: [{ name: "MCP" }] })
    finish(1, { ok: true, schemaVersion: 1, healthBasis: "configuration-only", definitions: [] })
    compare(names(), ["MCP"])
    inventory.refresh(); start()
    finish(2, { ok: true, schemaVersion: 1, healthBasis: "live", definitions: [{ name: "wrong" }] })
    compare(inventory.items.length, 0)
  }

  function usageRequests() { return requests.filter(function(request) { return request.args[7] === "usage" }) }
  function usage(uses) {
    return { ok: true, schemaVersion: 1, kind: "skill", coverageStart: "2026-05-06", until: "2026-09-16", ingestPending: false,
             days: uses ? [["2026-09-14", uses, uses, 0, 0, 0]] : [] }
  }

  function test_activity_stays_inert_without_a_method() {
    inventory.attach(observer()); start(); inventory.startActivity()
    compare(requests.length, 2)
    compare(usageRequests().length, 0)
    compare(inventory.activity, null)
  }

  function test_activity_follows_refreshes_one_request_at_a_time_and_drops_stale_answers() {
    inventory.activityMethod = "usage"
    inventory.observeActivity(testCase, true)
    inventory.activityArguments = function(owner) { return ["--json", "--project", owner.anchorPath].concat(owner.projectArguments) }
    inventory.attach(observer()); start(); inventory.startActivity()
    compare(usageRequests().length, 1)
    compare(usageRequests()[0].name, "helper-read")
    compare(JSON.parse(usageRequests()[0].args[9]), ["--json", "--project", "/project/one", "--exact"])
    inventory.refresh(); inventory.startActivity()
    compare(usageRequests().length, 1)
    usageRequests()[0].callback(usage(6))
    compare(inventory.activity.days[0][1], 6)
    inventory.startActivity()
    compare(usageRequests().length, 2)
    verify(inventory.mutate("apply", []))
    verify(cancelled.some(function(request) { return request.id === usageRequests()[1].id && !request.discardCallbacks }))
    usageRequests()[1].callback(usage(1))
    compare(inventory.activity.days[0][1], 6)
    inventory.startActivity()
    compare(usageRequests().length, 2)
    requests.filter(function(request) { return request.name === "helper-write" })[0].callback({ ok: true, schemaVersion: 1 })
    inventory.startActivity()
    compare(usageRequests().length, 3)
    usageRequests()[2].callback({ ok: false, error: "usage store is busy" })
    compare(inventory.activityError, "usage store is busy")
    compare(inventory.activity.days[0][1], 6)
    files.contextPath = "/project/two"
    inventory.startActivity()
    compare(JSON.parse(usageRequests()[3].args[9]), ["--json", "--project", "/project/two", "--exact"])
    usageRequests()[3].callback(usage(2))
    compare(inventory.activityError, "")
    compare(inventory.activity.days[0][1], 2)
  }

  function test_pending_activity_continues_and_hiding_all_views_stops_requests() {
    inventory.activityMethod = "usage"
    inventory.attach(observer())
    inventory.refresh(); inventory.startActivity()
    compare(usageRequests().length, 0)
    inventory.observeActivity(testCase, true)
    inventory.startActivity()
    compare(usageRequests().length, 1)
    usageRequests()[0].callback(Object.assign(usage(1), { ingestPending: true }))
    tryVerify(function() { return usageRequests().length === 2 })
    usageRequests()[1].callback(usage(3))
    compare(inventory.activity.days[0][1], 3)
    inventory.observeActivity(testCase, false)
    inventory.refresh(); inventory.startActivity()
    wait(750)
    compare(usageRequests().length, 2)
    inventory.observeActivity(testCase, true)
    inventory.startActivity()
    compare(usageRequests().length, 3)
  }

  function test_activity_request_is_discarded_with_the_provider() {
    inventory.activityMethod = "usage"
    inventory.observeActivity(testCase, true)
    inventory.attach(observer()); inventory.startActivity()
    var id = usageRequests()[0].id
    inventory.destroy(); wait(0); inventory = null
    verify(cancelled.some(function(request) { return request.id === id && request.discardCallbacks }))
  }

  function test_provider_destruction_cancels_owned_write() {
    inventory.attach(observer()); verify(inventory.mutate("apply", []))
    inventory.destroy(); wait(0); inventory = null
    compare(cancelled.length, 1)
    compare(cancelled[0].id, "request-0")
    verify(cancelled[0].discardCallbacks)
  }

  function test_provider_destruction_discards_read_and_watch_callbacks() {
    inventory.attach(observer()); start()
    finish(0, rows("first", ["/project/one"])); finish(1, rows("user", ["/home/me/.claude"]))
    inventory.refresh(); start()
    inventory.destroy(); wait(0); inventory = null
    for (var id of ["request-2", "request-3", "watch-0", "watch-1"])
      verify(cancelled.some(function(request) { return request.id === id && request.discardCallbacks }), id)
  }

  function usageSubscriptions() {
    return subscriptions.filter(function(entry) { return entry.paths[0] === "/home/me/.claude/projects" })
  }

  function test_transcript_directories_are_watched_and_a_change_refreshes_after_a_pause() {
    inventory.attach(observer()); start()
    var listing = rows("row", ["/plugins/inventory/skills"])
    listing.usageWatchPaths = ["/home/me/.claude/projects", "/home/me/.codex/sessions"]
    finish(0, listing); finish(1, listing)
    compare(usageSubscriptions().length, 1)
    var usage = usageSubscriptions()[0]
    verify(usage.includeWrites)
    verify(!subscriptions[0].includeWrites)
    compare(usage.paths, ["/home/me/.claude/projects", "/home/me/.codex/sessions"])
    compare(inventory.usageWatchPaths, usage.paths)
    var before = requests.length
    usage.event({ events: ["modify"], path: "/home/me/.claude/projects/x/s.jsonl" })
    usage.event({ events: ["modify"], path: "/home/me/.claude/projects/x/s.jsonl" })
    compare(requests.length, before)
    tryVerify(function() { return requests.length >= before + 2 }, inventory.usageChangeDelayMs + 2000)
    compare([scopeOf(before), scopeOf(before + 1)].sort(), ["project", "user"])
    var again = rows("row", ["/plugins/inventory/skills"])
    again.usageWatchPaths = usage.paths
    finish(before, again); finish(before + 1, again)
    compare(usageSubscriptions().length, 1)
    usage.closed({ cancelled: false })
    compare(inventory.usageWatch, null)
    inventory.refresh(); start()
    finish(requests.length - 2, again); finish(requests.length - 1, again)
    compare(usageSubscriptions().length, 2)
  }

  function test_the_transcript_watch_stops_with_the_last_view() {
    var view = observer()
    inventory.attach(view); start()
    var listing = rows("row", [])
    listing.usageWatchPaths = ["/home/me/.claude/projects"]
    finish(0, listing); finish(1, listing)
    compare(usageSubscriptions().length, 1)
    var usage = usageSubscriptions()[0]
    inventory.detach(view)
    compare(inventory.usageWatch, null)
    verify(cancelled.some(function(entry) { return entry.id === usage.id }))
  }

  function test_counts_update_in_place_survive_rescan_and_refresh_without_a_heatmap() {
    inventory.usageCountsMethod = "usage-counts"
    var view = observer()
    inventory.attach(view); start()
    var listing = rows("alpha", [])
    listing.items[0].id = "alpha"
    listing.usageWatchPaths = ["/home/me/.claude/projects"]
    finish(0, listing); finish(1, rows("project", []))
    compare(names(), ["alpha", "project"])
    tryCompare(requests, "length", 3)
    compare(requests[2].args[7], "usage-counts")
    finish(2, { ok: true, counts: { alpha: { uses: 3 } }, usageWatchPaths: listing.usageWatchPaths })
    compare(inventory.items[0].metrics.uses, 3)
    var before = inventory.items
    inventory.refresh(); start()
    finish(3, listing); finish(4, rows("project", []))
    verify(inventory.items === before)
    tryCompare(requests, "length", 6)
    var watch = usageSubscriptions()[0]
    watch.event({ events: ["close_write"] })
    wait(inventory.usageChangeDelayMs + 50)
    compare(requests.length, 6)
    finish(5, { ok: true, counts: { alpha: { uses: 4 } }, usageIngestPending: true })
    tryCompare(requests, "length", 7)
    finish(6, { ok: true, counts: { alpha: { uses: 5 } } })
    compare(inventory.items[0].metrics.uses, 5)
    compare(usageRequests().length, 0)
    inventory.detach(view)
    wait(inventory.usageChangeDelayMs + 50)
    compare(requests.length, 7)
    inventory.attach(view)
    compare(inventory.items[0].metrics.uses, 5)
  }

  function test_inflight_counts_are_cancelled_on_project_change_and_destruction() {
    inventory.usageCountsMethod = "usage-counts"
    inventory.attach(observer()); start()
    finish(0, rows("first")); finish(1, rows("user"))
    inventory.requestCounts()
    compare(requests.length, 3)
    files.contextPath = "/project/two"
    verify(cancelled.some(function(entry) { return entry.id === "request-2" }))
    finish(2, { ok: true, counts: { alpha: { uses: 99 } } })
    compare(Object.keys(inventory.usageCounts).length, 0)
    start(); finish(3, rows("second"))
    inventory.requestCounts()
    compare(requests.length, 5)
    inventory.destroy(); wait(0); inventory = null
    verify(cancelled.some(function(entry) { return entry.id === "request-4" && entry.discardCallbacks }))
  }
}
