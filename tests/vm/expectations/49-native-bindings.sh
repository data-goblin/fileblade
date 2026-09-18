#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"

[[ $FILEBLADE_SHAPE == native ]] || { printf '%s\n' 'FILEBLADE_SHAPE=native is required' >&2; exit 1; }
require_guest
launcher=/home/omarchy/.local/bin/fileblade
role=$(guest "$launcher native roles status --json" | jq -r .roles.bindings.enabled)
[[ $role == false ]] || { printf '%s\n' 'Use a disposable guest with the bindings role disabled' >&2; exit 1; }
result=$(guest "$launcher preferences --trash-retention-days 0 --agent-management false -o json")
expect_true E-40-21 "native preferences save through the authority" "[[ \$(jq -r .settings.trashRetentionDays <<<\"\$result\") == 0 ]]"
result=$(guest "$launcher preferences -o json")
expect_true E-40-21 "native preferences read the saved value" "[[ \$(jq -r .settings.agentManagement <<<\"\$result\") == false ]]"
"$OVM" restart >/dev/null
guest "$launcher blade set left files" >/dev/null
guest "$launcher blade set right notes" >/dev/null
fixture=$(guest 'mktemp -d /tmp/fileblade-bindings.XXXXXXXX')
[[ $fixture == /tmp/fileblade-bindings.* ]] || exit 1
probe=$(guest "setsid foot --title=FileBladeBindingsProbe sleep 120 >/dev/null 2>&1 </dev/null & echo \$!")
cleanup() {
  [[ $probe =~ ^[0-9]+$ ]] && guest "kill -TERM $probe 2>/dev/null || true"
  ctl closeBlade left
  ctl closeBlade right
  guest "$launcher native roles disable --role bindings --json" >/dev/null
  guest "rm -rf -- $(printf '%q' "$fixture")"
}
trap cleanup EXIT
expect_true E-40-19 "the legacy plugin endpoint is absent" "! guest 'omarchy-shell data-goblin.fileblade status'"
for attempt in 1 2; do
  result=$(guest "$launcher native roles enable --role bindings --json")
  expect_true E-40-19 "binding installation succeeds on attempt $attempt" "[[ \$(jq -r .status <<<\"\$result\") == complete || \$(jq -r .status <<<\"\$result\") == already_on ]]"
done
expect_true E-40-19 "Super+B has one registered action" "[[ \$(guest 'hyprctl -j binds' | jq '[.[] | select(.key == \"B\" and .modmask == 64 and .release == false)] | length') == 1 ]]"
expect_true E-40-19 "Hyprland accepts the bindings" "[[ -z \$(guest 'hyprctl configerrors') ]]"
ctl closeBlade left
ctl closeBlade right
sleep 1
"$OVM" key meta_l-b
sleep 1
expect E-40-19 "Super+B opens and focuses the left blade" focusedBlade left
"$OVM" key shift-meta_l-b
sleep 1
expect E-40-19 "Super+Shift+B opens and focuses the right blade" focusedBlade right
"$OVM" key meta_l-z
sleep 1
expect E-40-19 "Super+Z opens quick navigation" quickNavActive true
"$OVM" key esc
sleep 1
expect E-40-19 "Escape leaves quick navigation" quickNavActive false
guest "$launcher extension template acme.fileblade-audit $(printf '%q' "$fixture/extension") --author Audit" >/dev/null
result=$(guest "python3 $(printf '%q' "$fixture/extension/bin/fileblade-host-status")")
expect_true E-40-20 "a generated extension recognizes the native host" "[[ \$(jq -r .state <<<\"\$result\") == ready ]]"
summary
