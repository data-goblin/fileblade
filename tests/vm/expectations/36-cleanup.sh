#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"

if [[ ${FILEBLADE_SHAPE:-plugin} == native ]]; then
  fail harness "native fixture contract" "R65 needs integrated native routing and an isolated fixture strategy; this script only instruments staged plugin sources"
  summary
fi
require_guest
layout=$(field bladeLayoutPath)
[[ $layout == /*/blades.json ]] || { fail E-36-01 "layout namespace" "$layout"; summary; }
config=${layout%/*}
backup=$(guest 'mktemp -d /tmp/fileblade-e36.XXXXXX') || exit 1
settings=$config/settings.json
fixture_py=$(cat "$(dirname "$0")/../fixtures/36-cleanup.py")
fixture_state() {
  "$OVM" ssh "python3 -c $(printf '%q' "$fixture_py") $(printf '%q' "$1") $(printf '%q' "$config") $(printf '%q' "$backup") $(printf '%q' "$GUEST_PLUGIN")"
}

require_wait() {
  wait_for "$@" || { fail harness "wait for expected state" "$1"; [[ -z ${module:-} ]] || probe status; exit 1; }
}

probe() { "$OVM" ipc "fileblade.core-live.$module" "$@" 2>/dev/null; }
probe_bin() {
  local result
  result=$(probe bin "$@")
  [[ $result == queued ]] || { fail harness "$module $2 admitted" "$result"; probe status; exit 1; }
}
probe_ready() { probe status | jq -e '.ready and (.busy|not) and (.bin.busy|not) and (.loadError == "")' >/dev/null; }
consent_text() { "$OVM" mouse move 900 900; ocr_crop E-36-consent 380x1080+0+0 300% 6 '15%,40%'; }
right_text() { "$OVM" mouse move 900 900; ocr_crop E-36-right 360x1080+1560+0 300% 6 '15%,40%'; }

restart_checked() {
  restart_shell || return 1
  if ! status | jq -e '.bladeModules | contains(["files","properties","notes","welcome","skills","memory","hooks","mcp"])' >/dev/null; then
    fail harness "modules resolve after restart" "missing built-in module"
    return 1
  fi
  "$OVM" shot "E-36-restart-$1" >/dev/null
}
cleanup() {
  [[ -z ${focus_pid:-} ]] || guest "kill $focus_pid 2>/dev/null || true"
  "$(dirname "$0")/../stop-shell" || return 1
  if [[ -n ${locked_recovery:-} ]]; then
    guest "test ! -d $(printf '%q' "$locked_recovery") || chmod 0700 -- $(printf '%q' "$locked_recovery")" || return 1
  fi
  fixture_state restore || return 1
  restart_checked restored || return 1
  trap - EXIT
  printf 'Restored complete saved documents; backup retained at %s\n' "$backup"
}
"$(dirname "$0")/../stop-shell" || exit 1
fixture_state backup || { restart_shell; exit 1; }
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
fixture_state isolate || exit 1
fixture_state seed || exit 1
recovery_root=$(guest "jq -r ' .isolation[] | select(.label == \"recovery\") | .path' $backup/manifest.json")
artifact_root=$(guest "jq -r '.isolation[] | select(.label == \"artifact-bin\") | .path' $backup/manifest.json")
fixture_state unanswered || exit 1
restart_checked unanswered || exit 1
ctl setSidebarWidth 380
ctl setPropertiesBladeWidth 360
ctl setBladeSlots left "base64:$(printf '%s' '[{"id":"e36-files","modules":[{"module":"files"},{"module":"properties"}]}]' | base64 -w0)"
ctl focusBlade left
sleep 1
consent=$(consent_text)
expect_contains E-36-01 "unanswered preferences present the consent question" "$consent" "automatically empty"
for label in '1 day' '7 days' '30 days' '90 days'; do
  expect_contains E-36-01 "consent offers $label" "$consent" "$label"
done
"$OVM" key esc
sleep 1
expect_contains E-36-01 "Escape leaves the question present" "$(screen_text)" "automatically empty"
expect_out E-36-01 "Escape records no answer" "jq -r 'has(\"trashRetentionDays\")' $(printf '%q' "$settings")" false
expect_contains E-36-02 "consent explains permanent deletion" "$consent" "permanently deletes"
expect_contains E-36-02 "consent names shared desktop Trash" "$consent" "shared desktop Trash"
expect_contains E-36-02 "consent points to settings" "$consent" "change this later"
expect E-36-02 "cleanup defaults off before confirmation" trashRetentionDays 0
"$OVM" shot E-36-01-unanswered >/dev/null

focus_pid=$(guest "foot --app-id=e36-focus --title=E36-focus sh -c 'cat > $backup/focus-input' > $backup/focus.log 2>&1 & echo \$!")
require_wait '"$OVM" hypr clients | jq -e '\''.[] | select(.class == "e36-focus")'\''' 15
ctl releaseBladeFocus
guest "hyprctl dispatch focuswindow 'class:^e36-focus$'" >/dev/null
"$OVM" mouse click 700 250
"$OVM" type "e36-other-app"
"$OVM" key ret
require_wait "[[ \$(guest 'cat $backup/focus-input') == e36-other-app ]]" 10
expect_out E-36-02 "another app receives input while consent waits" "cat $backup/focus-input" e36-other-app
"$OVM" shot E-36-02-other-app >/dev/null
ctl focusBlade left
expect_contains E-36-02 "returning preserves the question" "$(screen_text)" "automatically empty"
"$OVM" shot E-36-02-returned >/dev/null
guest "kill $focus_pid 2>/dev/null || true"
focus_pid=

ctl undockBlade left
require_wait '[[ $(blade_mode left) == window ]]' 15
sleep 1
expect_contains E-36-02 "undocked consent remains visible" "$(screen_text)" "automatically empty"
"$OVM" shot E-36-02-undocked >/dev/null
ctl dockBlade left
require_wait '[[ $(blade_mode left) == docked ]]' 15
ctl focusBlade left
"$OVM" key ret
require_wait "[[ \$(field trashRetentionDays) == 0 ]] && ! screen_text | grep -qi 'automatically empty'" 15
expect_out E-36-01 "Confirm persists the initially selected Never" "jq -r .trashRetentionDays $(printf '%q' "$settings")" 0
expect_missing E-36-03 "confirmed consent disappears" "$(screen_text)" "automatically empty"
restart_checked never || exit 1
expect E-36-03 "Never survives restart" trashRetentionDays 0
expect_missing E-36-03 "saved answer is not asked again" "$(screen_text)" "automatically empty"
expect_out E-36-03 "preferences retain schema and release metadata" "jq -r '(.version == 1) and (.filebladeVersion | type == \"string\")' $(printf '%q' "$settings")" true
guest "$(printf '%q' "$GUEST_PLUGIN/fileblade") _backend keybindings-prepare" >/dev/null || exit 1
expect_out E-36-03 "binding preparation records schema and release metadata" "jq -r '(.version == 1) and (.filebladeVersion | type == \"string\")' $(printf '%q' "$config/keybindings.json")" true
expect_out E-36-03 "custom keybindings are preserved" "jq -c .bindings $(printf '%q' "$config/keybindings.json")" '{"next":["n"]}'

"$OVM" shot E-36-03-never >/dev/null
ctl closeBlade left
ctl openBlade right
"$(dirname "$0")/../stop-shell" || exit 1
fixture_state unanswered || exit 1
restart_checked right-only || exit 1
expect E-36-02 "unanswered consent opens a saved closed left blade" open true
expect_true E-36-02 "consent appears only once with both blades open" '[[ $(screen_text | grep -o "automatically empty" | wc -l) == 1 ]]'
"$OVM" shot E-36-02-right-only >/dev/null
for days in 1 7 30 90; do
  "$(dirname "$0")/../stop-shell" || exit 1
  fixture_state unanswered || exit 1
  restart_checked "policy-$days" || exit 1
  ctl focusBlade left
  case $days in 1) count=1;; 7) count=2;; 30) count=3;; 90) count=4;; esac
  for ((i=0; i<count; i++)); do "$OVM" key down; done
  "$OVM" shot "E-36-03-selected-$days" >/dev/null
  "$OVM" key ret
  require_wait "[[ \$(field trashRetentionDays) == $days ]]" 15
  expect E-36-03 "$days days saved after Confirm" trashRetentionDays "$days"
  restart_checked "saved-$days" || exit 1
  expect E-36-03 "$days days survives restart" trashRetentionDays "$days"
  expect_missing E-36-03 "$days days is not asked again" "$(screen_text)" "automatically empty"
  "$OVM" shot "E-36-03-saved-$days" >/dev/null
done
"$(dirname "$0")/../stop-shell" || exit 1
fixture_state fresh || exit 1
restart_checked fresh || exit 1
ctl focusBlade left
expect_contains E-36-01 "absent preferences also ask" "$(screen_text)" "automatically empty"
"$OVM" shot E-36-01-fresh >/dev/null
"$OVM" key ret
require_wait '[[ $(field trashRetentionDays) == 0 ]] && ! screen_text | grep -qi "automatically empty"' 15
"$(dirname "$0")/../stop-shell" || exit 1
fixture_state rejected || exit 1
restart_checked rejected || exit 1
ctl focusBlade left
"$OVM" key down
"$OVM" key ret
sleep 2
expect_contains E-36-03 "rejected save keeps consent visible" "$(screen_text)" "automatically empty"
expect E-36-03 "rejected save keeps cleanup off" trashRetentionDays 0
expect_out E-36-03 "refused save preserves the symlink and target" "test -L $(printf '%q' "$settings") && cat $backup/denied.json" '{"version":1,"fixture":"E36 denied"}'
"$OVM" shot E-36-03-rejected >/dev/null
"$(dirname "$0")/../stop-shell" || exit 1
fixture_state unanswered || exit 1
restart_checked management || exit 1
ctl focusBlade left
"$OVM" key ret
require_wait '! screen_text | grep -qi "automatically empty"' 15
goto_root "$backup/project"

for module in skills memory; do
  ctl setBladeSlots right "base64:$(printf '[{"id":"e36-%s","modules":[{"module":"%s"}]}]' "$module" "$module" | base64 -w0)"
  ctl openBlade right
  ctl focusBlade right
  sleep 1
  probe projectContext >/dev/null
  require_wait "[[ \$(field contextPath) == $(printf '%q' "$backup/project") ]]" 15
  expect_contains E-36-04 "$module is available without an install" "$("$OVM" ipc "$PLUGIN" blades)" "$module"
  expect_missing E-36-04 "$module resolves as a core blade" "$(screen_text)" "Unknown module"
done
"$OVM" shot E-36-04-browsing >/dev/null

ctl focusBlade left
ctl toggleBladeSettings left
sleep 1
"$OVM" mouse click 100 118
"$OVM" type "Trash retention"
sleep 1
expect_contains E-36-03 "settings exposes the retention choices" "$(consent_text)" "Trash retention"
"$OVM" mouse click 95 219
sleep 1
expect_contains E-36-03 "settings explains permanent cleanup before enabling it" "$(consent_text)" "cannot be undone"
"$OVM" shot E-36-03-settings-confirm >/dev/null
click_word cleanup || exit 1
require_wait '[[ $(field trashRetentionDays) == 1 ]]' 15
expect E-36-03 "settings changes retention after confirmation" trashRetentionDays 1
"$OVM" mouse click 54 219
require_wait '[[ $(field trashRetentionDays) == 0 ]]' 15
"$OVM" shot E-36-03-settings-never >/dev/null
ctl toggleBladeSettings left

for module in skills memory hooks mcp; do
  ctl setBladeSlots right "base64:$(printf '[{"id":"e36-%s","modules":[{"module":"%s"}]}]' "$module" "$module" | base64 -w0)"
  ctl focusBlade right
  require_wait "probe_ready && probe status | jq -e '.rows | length > 0'" 20 || { fail E-36-04 "$module fixture loads" "$(probe status)"; exit 1; }
  row=$(probe status | jq -r '.rows[] | select(.kind != "bin") | .id' | head -1)
  if [[ $module == skills || $module == memory ]]; then
    source_path="$backup/project/AGENTS.md"
    [[ $module != skills ]] || source_path="$backup/project/.claude/skills/rivet-fixture/SKILL.md"
    source_before=$(guest "sha256sum $(printf '%q' "$source_path")")
  fi
  for round in 1 2 3; do
    probe_bin "$row" bin
    require_wait "probe_ready && probe status | jq -e '.bin.rows | length == 1'" 20 || { fail E-36-05 "$module removal $round" "$(probe status)"; exit 1; }
    entry=$(probe status | jq -r '.bin.rows[0].id')
    record="$artifact_root/$module/${entry#bin:}"
    if [[ $module == skills || $module == memory ]]; then
      probe_bin "$entry" restore
    else
      recovery=$(guest "jq -r .helperRecordId $(printf '%q' "$record/manifest.json")")
      recovery_path="$recovery_root/$module-recovery/$recovery.json"
      expect_out E-36-05 "$module removal has private recovery" "test -f $(printf '%q' "$recovery_path") && echo paired" paired
      if [[ $round == 3 ]]; then
        locked_recovery="$recovery_root/$module-recovery"
        guest "chmod 0500 -- $(printf '%q' "$locked_recovery")" || exit 1
        completed_before=$(probe status | jq .completions)
        probe_bin "$entry" purge
        require_wait "probe status | jq -e '.completions > $completed_before and .response.ok == false'" 20
        expect_out E-36-05 "$module unwritable recovery store preserves both records" "test -f $(printf '%q' "$record/manifest.json") && test -f $(printf '%q' "$recovery_path") && echo retained" retained
        "$OVM" shot "E-36-05-$module-recovery-locked" >/dev/null
        guest "chmod 0700 -- $(printf '%q' "$locked_recovery")" || exit 1
        locked_recovery=
        probe refresh >/dev/null
        require_wait "probe_ready && probe status | jq -e '.bin.rows | length == 1'" 20
        probe projectContext >/dev/null
        continue
      fi
      require_wait "probe status | jq -e '.bin.rows | length == 1'" 20
      expect_contains E-36-05 "$module opens the recovery choices" "$(probe bin "$entry" ask)" opened
      sleep 1
      expect_true E-36-05 "$module purge is explicitly permanent" "probe status | jq -e '.bin.choices | any(.key == \"purge\" and .label == \"Delete forever\" and .danger == true)'"
      "$OVM" shot "E-36-05-$module-purge-$round" >/dev/null
      click_word forever right || exit 1
    fi
    require_wait "probe_ready && probe status | jq -e '.bin.rows | length == 0'" 20
    expect_true E-36-05 "$module round $round leaves no bin row" "probe status | jq -e '.bin.rows | length == 0'"
    if [[ $module == hooks || $module == mcp ]]; then
      expect_out E-36-05 "$module purge deletes paired recovery" "test ! -e $(printf '%q' "$recovery_path") && echo removed" removed
      fixture_state seed-project || exit 1
      probe refresh >/dev/null
      require_wait "probe_ready && probe status | jq -e '.rows | length > 0'" 20
    else
      expect_out E-36-04 "$module restores the original source bytes" "sha256sum $(printf '%q' "$source_path")" "$source_before"
      break
    fi
    "$OVM" shot "E-36-05-$module-empty-$round" >/dev/null
  done
done

ctl setWelcomeState dismissed
ctl setBladeSlots right "base64:$(printf '%s' '[{"id":"e36-welcome","modules":[{"module":"welcome"},{"module":"notes","state":{"text":"E36 retained note"}}]}]' | base64 -w0)"
ctl focusBlade right
sleep 1
expect_contains E-36-06 "dismissed Welcome remains reopenable" "$(right_text)" "Find your way"
expect_missing E-36-06 "Welcome offers no companion installer" "$(right_text)" "Install agent extensions"
ctl welcomeDismiss
require_wait '"$OVM" ipc "$PLUGIN" blades | jq -e '\''[.blades.right.slots[].modules[].module] == ["notes"]'\'''
"$OVM" shot E-36-06-welcome-dismissed >/dev/null
ctl addBladeModule right welcome
sleep 1
expect_contains E-36-06 "reopened Welcome renders again" "$(right_text)" "Find your way"
expect_contains E-36-06 "reopening Welcome retains adjacent Notes" "$("$OVM" ipc "$PLUGIN" blades)" 'notes'
expect E-36-06 "reopening keeps the dismissal choice" welcomeState dismissed
expect E-36-06 "Welcome starts no installer" welcomeInstalling false
"$OVM" shot E-36-06-welcome-reopened >/dev/null

fixture_state updates || exit 1
checkout="$backup/update-checkout"
remote_head=$(guest "jq -r .source_head $backup/updates.json")
objects_before=$(guest "git -C $checkout count-objects -v")
guest "$(printf '%q' "$GUEST_PLUGIN/fileblade") _backend update-check --core t.plugin --repository t.plugin=$checkout > $backup/updates-clean.json" || exit 1
expect_out E-36-06 "update comparison stays unknown without upstream objects" "jq -r '.available and (.repositories[0].comparison_known == false) and (.repositories[0].behind == null) and (.repositories[0].subjects == [])' $backup/updates-clean.json" true
expect_out E-36-06 "update check downloads no Git objects" "git -C $checkout count-objects -v" "$objects_before"
expect_out E-36-06 "upstream commit remains absent locally" "git -C $checkout cat-file -e $remote_head 2>/dev/null || echo absent" absent
guest "printf 'E36 modified checkout\n' > $checkout/Service.qml"
guest "$(printf '%q' "$GUEST_PLUGIN/fileblade") _backend update-check --core t.plugin --repository t.plugin=$checkout > $backup/updates-dirty.json" || exit 1
expect_out E-36-06 "modified checkout is skipped" "jq -r '.repositories[0].dirty and (.repositories[0].updatable == false)' $backup/updates-dirty.json" true
expect_out E-36-06 "checking preserves local checkout edits" "cat $checkout/Service.qml" 'E36 modified checkout'
"$OVM" shot E-36-06-update-check >/dev/null

pending E-36-04 "installed-native cleanup lifecycle" "R65 routing helpers and an isolated native fixture are still needed; only shared Service and ordinary CLI are qualified here."

cleanup || { fail harness "restore original documents" "backup: $backup"; summary; }
summary
