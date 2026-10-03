import QtQuick
import QtTest
import "../HostGuard.js" as HostGuard

TestCase {
  name: "HostGuard"
  readonly property var self: ({ id: "{{PLUGIN_ID}}", name: "{{PLUGIN_NAME}}", enabled: true })

  function snapshot(state, plugins) {
    return { schemaVersion: 1, state: state, plugins: plugins === undefined ? [self] : plugins }
  }

  function test_a_ready_host_shows_nothing() {
    compare(HostGuard.plan(snapshot("ready"), self.id).show, false)
    compare(HostGuard.plan(null, self.id).show, false)
    compare(HostGuard.plan({ schemaVersion: 2, state: "missing" }, self.id).show, false)
  }

  function test_a_missing_host_never_offers_to_install_it() {
    var plan = HostGuard.plan(snapshot("missing"), self.id)
    verify(plan.show)
    compare(plan.command, [])
    compare(plan.action, "")
    verify(plan.message.indexOf("never installs it for you") > 0)
  }

  function test_only_a_missing_host_names_the_command_that_removes_the_extension() {
    compare(HostGuard.plan(snapshot("missing"), self.id).removal, "omarchy plugin remove " + self.id)
    for (var state of ["disabled", "starting", "unknown", "ready"])
      compare(HostGuard.plan(snapshot(state), self.id).removal, "")
  }

  function test_a_disabled_host_offers_to_enable_it() {
    var plan = HostGuard.plan(snapshot("disabled"), self.id)
    verify(plan.show)
    compare(plan.action, "Enable")
    compare(plan.command.slice(0, 3), ["timeout", "--kill-after=1s", "20s"])
    verify(plan.command.join(" ").indexOf("$(omarchy") === -1)
    verify(plan.command.join(" ").indexOf("omarchy plugin enable data-goblin.fileblade") >= 0)
  }

  function test_starting_or_unknown_waits_rather_than_claiming_it_is_absent() {
    for (var state of ["starting", "unknown"]) {
      var plan = HostGuard.plan(snapshot(state), self.id)
      verify(plan.show)
      compare(plan.command, [])
      compare(plan.action, "")
      verify(plan.message.indexOf("not installed") === -1)
    }
  }

  function test_no_plan_ever_carries_a_remote_fetch() {
    for (var state of ["missing", "disabled", "starting", "unknown", "ready"]) {
      var plan = HostGuard.plan(snapshot(state), self.id)
      var command = plan.command.join(" ")
      verify(command.indexOf("plugin add") < 0)
      verify(command.indexOf("git") < 0)
      verify(command.indexOf("://") < 0)
    }
  }
}
